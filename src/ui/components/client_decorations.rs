//! Window moves, frame and edge resizes for Linux compositors that leave decorations to the
//! application. Windows keeps native caption, borders and hit-testing: these are no-ops there.
use gpui::*;
#[cfg(target_os = "linux")]
use gpui::prelude::FluentBuilder;

pub(crate) trait ClientDecorationsExt: InteractiveElement + Sized {
    /// Title area: drag moves, double-click maximizes, right-click opens the window menu.
    fn client_drag_area(self) -> Self {
        #[cfg(target_os = "linux")]
        {
            self.on_mouse_down(MouseButton::Left, |event, window, _| {
                let maximize = event.click_count == 2;
                PENDING_MOVE.set(!maximize);
                if maximize && window.is_resizable() {
                    window.zoom_window();
                }
            })
            .on_mouse_up(MouseButton::Left, |_, _, _| PENDING_MOVE.set(false))
            // Start on the first motion so a double-click still reaches the window.
            .on_mouse_move(|event, window, _| {
                if event.pressed_button == Some(MouseButton::Left) && PENDING_MOVE.replace(false) {
                    window.start_window_move();
                }
            })
            .on_mouse_down(MouseButton::Right, |event, window, _| {
                window.show_window_menu(event.position);
            })
        }
        #[cfg(not(target_os = "linux"))]
        {
            self
        }
    }
}
impl<T: InteractiveElement> ClientDecorationsExt for T {}

/// Wrap a window root. On Linux with client decorations, a transparent margin carries the
/// shadow and the resize zones; tiled, maximized and fullscreen sides have none.
pub(crate) fn frame(content: impl IntoElement, border: Hsla, window: &mut Window) -> AnyElement {
    #[cfg(target_os = "linux")]
    {
        let Decorations::Client { tiling } = window.window_decorations() else {
            return content.into_any_element();
        };
        window.set_client_inset(MARGIN);
        let margin = margin(tiling);
        let side = |tiled: bool| if tiled { px(0.0) } else { px(1.0) };
        div()
            .size_full()
            .pt(margin.top)
            .pr(margin.right)
            .pb(margin.bottom)
            .pl(margin.left)
            .when(window.is_resizable(), |frame| {
                frame
                    .child(canvas(
                        move |_, window, _| resize_zones(window.viewport_size(), tiling)
                            .map(|(edge, zone)| (edge, window.insert_hitbox(zone, HitboxBehavior::Normal)))
                            .collect::<Vec<_>>(),
                        |_, zones, window, _| {
                            for (edge, hitbox) in &zones {
                                window.set_cursor_style(resize_cursor(*edge), hitbox);
                            }
                        },
                    ).absolute().top_0().left_0().size_full())
                    .on_mouse_down(MouseButton::Left, move |event, window, cx| {
                        if let Some(edge) = resize_edge(event.position, window.viewport_size(), tiling) {
                            window.start_window_resize(edge);
                            cx.stop_propagation();
                        }
                    })
            })
            .child(
                div()
                    .relative()
                    .size_full()
                    .when(!tiling.is_tiled(), |window_area| window_area.shadow(vec![BoxShadow {
                        color: hsla(0.0, 0.0, 0.0, 0.4),
                        offset: point(px(0.0), px(0.0)),
                        blur_radius: MARGIN / 2.0,
                        spread_radius: px(0.0),
                        inset: false,
                    }]))
                    .child(content)
                    // Drawn over the content so the border takes no layout space.
                    .child(div().absolute().inset_0().border_color(border)
                        .border_t(side(tiling.top))
                        .border_r(side(tiling.right))
                        .border_b(side(tiling.bottom))
                        .border_l(side(tiling.left))),
            )
            .into_any_element()
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (border, window);
        content.into_any_element()
    }
}

/// Window area showing content: the viewport less the Linux shadow margin.
pub(crate) fn content_bounds(window: &Window) -> Bounds<Pixels> {
    let viewport = window.viewport_size();
    #[cfg(target_os = "linux")]
    if let Decorations::Client { tiling } = window.window_decorations() {
        let margin = margin(tiling);
        return Bounds::new(point(margin.left, margin.top), size(
            (viewport.width - margin.left - margin.right).max(px(0.0)),
            (viewport.height - margin.top - margin.bottom).max(px(0.0)),
        ));
    }
    Bounds::new(Point::default(), viewport)
}

/// Window size giving `content` its size once the Linux shadow margin is added.
pub(crate) fn window_size_for(content: Size<Pixels>, window: &Window) -> Size<Pixels> {
    let shown = content_bounds(window).size;
    let viewport = window.viewport_size();
    size(content.width + viewport.width - shown.width, content.height + viewport.height - shown.height)
}

#[cfg(target_os = "linux")]
thread_local! {
    static PENDING_MOVE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Transparent margin holding the shadow and the resize zones.
#[cfg(target_os = "linux")]
const MARGIN: Pixels = px(10.0);

#[cfg(target_os = "linux")]
fn margin(tiling: Tiling) -> Edges<Pixels> {
    let side = |tiled: bool| if tiled { px(0.0) } else { MARGIN };
    Edges {
        top: side(tiling.top),
        right: side(tiling.right),
        bottom: side(tiling.bottom),
        left: side(tiling.left),
    }
}

/// Non-overlapping zones of the margin, corners first; tiled sides have none.
#[cfg(target_os = "linux")]
fn resize_zones(size: Size<Pixels>, tiling: Tiling) -> impl Iterator<Item = (ResizeEdge, Bounds<Pixels>)> {
    let Edges { top, right, bottom, left } = margin(tiling);
    let (width, height) = (size.width, size.height);
    let (inner_width, inner_height) = (width - left - right, height - top - bottom);
    let zone = |x, y, w, h| Bounds::new(point(x, y), gpui::size(w, h));
    [
        (ResizeEdge::TopLeft, zone(px(0.0), px(0.0), left, top)),
        (ResizeEdge::TopRight, zone(width - right, px(0.0), right, top)),
        (ResizeEdge::BottomLeft, zone(px(0.0), height - bottom, left, bottom)),
        (ResizeEdge::BottomRight, zone(width - right, height - bottom, right, bottom)),
        (ResizeEdge::Top, zone(left, px(0.0), inner_width, top)),
        (ResizeEdge::Bottom, zone(left, height - bottom, inner_width, bottom)),
        (ResizeEdge::Left, zone(px(0.0), top, left, inner_height)),
        (ResizeEdge::Right, zone(width - right, top, right, inner_height)),
    ]
    .into_iter()
    .filter(|(_, zone)| !zone.is_empty())
}

#[cfg(target_os = "linux")]
fn resize_edge(position: Point<Pixels>, size: Size<Pixels>, tiling: Tiling) -> Option<ResizeEdge> {
    // Half-open, unlike `Bounds::contains`, so adjacent zones never share a pixel.
    resize_zones(size, tiling)
        .find(|(_, zone)| {
            let end = zone.bottom_right();
            (zone.origin.x..end.x).contains(&position.x) && (zone.origin.y..end.y).contains(&position.y)
        })
        .map(|(edge, _)| edge)
}

#[cfg(target_os = "linux")]
fn resize_cursor(edge: ResizeEdge) -> CursorStyle {
    match edge {
        ResizeEdge::Top | ResizeEdge::Bottom => CursorStyle::ResizeUpDown,
        ResizeEdge::Left | ResizeEdge::Right => CursorStyle::ResizeLeftRight,
        ResizeEdge::TopLeft | ResizeEdge::BottomRight => CursorStyle::ResizeUpLeftDownRight,
        ResizeEdge::TopRight | ResizeEdge::BottomLeft => CursorStyle::ResizeUpRightDownLeft,
    }
}

#[cfg(all(test, target_os = "linux"))]
#[path = "client_decorations/tests/edges.rs"]
mod tests;
