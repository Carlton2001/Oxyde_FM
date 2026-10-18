use crate::ui::app::OxydeApp;
use crate::ui::scale::rem_pixels;
use crate::ui::theme::OxydeTheme;
use gpui::prelude::*;
use gpui::*;

/// Pointer state owned by the view using a shared scrollbar.
pub trait ScrollbarState {
    fn scrollbar_origins(&mut self) -> &mut std::collections::HashMap<String, Point<f32>>;
    fn set_grab_offset(&mut self, offset: f32);
}
impl ScrollbarState for OxydeApp {
    fn scrollbar_origins(&mut self) -> &mut std::collections::HashMap<String, Point<f32>> { &mut self.scrollbar_origins }
    fn set_grab_offset(&mut self, offset: f32) { self.grab_offset = offset; }
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum ScrollbarAxis {
    Vertical,
    Horizontal,
}

/// Only the rail's outer contour depends on its parent. Colors, spacing and
/// pointer behavior are shared by every scrollbar.
#[derive(Clone, Copy)]
pub(crate) enum TrackShape {
    Square,
    RoundedRight(Rems),
}

const ZED_THUMB_MIN_SIZE: Rems = rems(2.5);
const THUMB_END_INSET: Rems = rems(0.1875);
fn zed_thumb_min_size() -> f32 {
    rem_pixels(ZED_THUMB_MIN_SIZE).as_f32()
}
pub const ZED_TRACK_WIDTH: Rems = rems(0.75);
pub fn zed_track_width() -> f32 {
    rem_pixels(ZED_TRACK_WIDTH).as_f32()
}
const ZED_THUMB_WIDTH_NORMAL: Rems = rems(0.3125);
fn zed_thumb_width_normal() -> f32 {
    rem_pixels(ZED_THUMB_WIDTH_NORMAL).as_f32()
}
const ZED_THUMB_WIDTH_HOVER: Rems = rems(0.625);
fn zed_thumb_width_hover() -> f32 {
    rem_pixels(ZED_THUMB_WIDTH_HOVER).as_f32()
}

/// Composant Scrollbar Visuel Réutilisable "Zed-like"
pub fn render_scrollbar(
    theme: OxydeTheme,
    handle: Entity<OxydeApp>,
    axis: ScrollbarAxis,
    viewport_size: f32,
    content_size: f32,
    current_scroll: f32,
    is_dragging: bool,
    id_suffix: String,
    min_thumb_size: Option<f32>,
    on_drag: impl Fn(&mut OxydeApp, String, f32, f32, &mut App) + 'static,
) -> AnyElement {
    render_scrollbar_with_pointer(
        theme,
        handle,
        axis,
        viewport_size,
        content_size,
        current_scroll,
        is_dragging,
        id_suffix,
        min_thumb_size,
        move |view, track, viewport, content, _, cx| on_drag(view, track, viewport, content, cx),
    )
}
pub fn render_scrollbar_with_pointer<T: ScrollbarState + 'static>(
    theme: OxydeTheme,
    handle: Entity<T>,
    axis: ScrollbarAxis,
    viewport_size: f32,
    content_size: f32,
    current_scroll: f32,
    is_dragging: bool,
    id_suffix: String,
    min_thumb_size: Option<f32>,
    on_drag: impl Fn(&mut T, String, f32, f32, Point<Pixels>, &mut App) + 'static,
) -> AnyElement {
    render_scrollbar_on_track(theme, handle, axis, viewport_size, content_size, current_scroll, is_dragging, id_suffix, min_thumb_size, viewport_size, TrackShape::Square, on_drag)
}

/// Separate the physical rail length from the scrollable content viewport.
pub(crate) fn render_scrollbar_on_track<T: ScrollbarState + 'static>(
    theme: OxydeTheme,
    handle: Entity<T>,
    axis: ScrollbarAxis,
    viewport_size: f32,
    content_size: f32,
    current_scroll: f32,
    is_dragging: bool,
    id_suffix: String,
    min_thumb_size: Option<f32>,
    track_size: f32,
    shape: TrackShape,
    on_drag: impl Fn(&mut T, String, f32, f32, Point<Pixels>, &mut App) + 'static,
) -> AnyElement {
    let is_v = axis == ScrollbarAxis::Vertical;

    // Proportions
    let ratio = (viewport_size / content_size.max(1.0)).min(1.0);

    // Don't render scrollbar if content fits
    if ratio >= 1.0 {
        return div().into_any_element();
    }

    let m_thumb_size = min_thumb_size.unwrap_or(zed_thumb_min_size());
    let usable_track = track_size.max(0.0);
    let thumb_size = (usable_track * ratio).max(m_thumb_size).min(usable_track);
    let max_offset = (content_size - viewport_size).max(1.0);
    let scroll_ratio = (current_scroll.max(0.0) / max_offset).clamp(0.0, 1.0);
    let thumb_pos = scroll_ratio * (usable_track - thumb_size);
    // Inset only the painted grip. Keep its logical bounds for hit testing and
    // dragging so existing viewport/offset calculations retain their behavior.
    let grip_inset = rem_pixels(THUMB_END_INSET).as_f32().min(thumb_size / 2.0);
    let grip_size = (thumb_size - grip_inset * 2.0).max(0.0);

    let mut thumb = div()
        .absolute()
        .bg(theme.scrollbar_thumb) // Plus subtil par défaut
        .rounded_md();

    if is_v {
        thumb = thumb
            .right(rems(0.1875))
            .top(px(thumb_pos + grip_inset))
            .h(px(grip_size))
            .w(px(zed_thumb_width_normal()))
            .group_hover("scroll-track", |s| {
                s.w(px(zed_thumb_width_hover()))
                    .right(rems(0.0625))
                    .bg(theme.scrollbar_thumb_hover)
            });
        if is_dragging {
            thumb = thumb
                .w(px(zed_thumb_width_hover()))
                .right(rems(0.0625))
                .bg(theme.scrollbar_thumb_drag);
        }
    } else {
        thumb = thumb
            .bottom(rems(0.1875))
            .left(px(thumb_pos + grip_inset))
            .w(px(grip_size))
            .h(px(zed_thumb_width_normal()))
            .group_hover("scroll-track", |s| {
                s.h(px(zed_thumb_width_hover()))
                    .bottom(rems(0.0625))
                    .bg(theme.scrollbar_thumb_hover)
            });
        if is_dragging {
            thumb = thumb
                .h(px(zed_thumb_width_hover()))
                .bottom(rems(0.0625))
                .bg(theme.scrollbar_thumb_drag);
        }
    }

    let is_v = axis == ScrollbarAxis::Vertical;
    let track_id = format!(
        "scrollbar-track-{}-{}",
        if is_v { "v" } else { "h" },
        id_suffix
    );
    let track_id_cap = track_id.clone();
    let handle_cap = handle.clone();

    let track_container = div()
        .id(SharedString::from(track_id.clone()))
        .group("scroll-track")
        .absolute()
        .bg(theme.scrollbar_track)
        .map(|track| match shape {
            TrackShape::Square => track,
            TrackShape::RoundedRight(radius) => track.rounded_r(radius),
        })
        .when(is_v, |s: Stateful<Div>| {
            s.top_0().right_0().bottom_0().w(px(zed_track_width()))
        })
        .when(!is_v, |s: Stateful<Div>| {
            s.left_0().bottom_0().right_0().h(px(zed_track_width()))
        })
        .child(
            canvas(
                |bounds, _, _| bounds,
                move |bounds, _, window, cx| {
                    let origin = crate::ui::components::popup::owner_point(window, cx, bounds.origin);
                    handle_cap.update(cx, |this, _| {
                        this.scrollbar_origins().insert(
                            track_id_cap.clone(),
                            origin.map(|p| p.to_f64() as f32),
                        );
                    });
                },
            )
            .absolute()
            .size_full(),
        )
        .child(thumb)
        .on_mouse_down(MouseButton::Left, {
            let h = handle.clone();
            let track_id_for_drag = track_id.clone();
            move |e, window, cx| {
                cx.stop_propagation();
                let position = crate::ui::components::popup::owner_point(window, cx, e.position);
                let mouse_pos = if is_v {
                    position.y.to_f64() as f32
                } else {
                    position.x.to_f64() as f32
                };

                h.update(cx, |this, app| {
                    if let Some(origin) = this.scrollbar_origins().get(&track_id_for_drag) {
                        let track_origin_coord = if is_v { origin.y } else { origin.x };
                        let mouse_in_track = mouse_pos - track_origin_coord;

                        let max_offset = (content_size - viewport_size).max(1.0);
                        let ratio = (viewport_size / content_size.max(1.0)).min(1.0);
                        let m_thumb_size = min_thumb_size.unwrap_or(zed_thumb_min_size());
                        let t_size = (usable_track * ratio).max(m_thumb_size).min(usable_track);
                        let scroll_ratio = (current_scroll.max(0.0) / max_offset).clamp(0.0, 1.0);
                        let t_pos = scroll_ratio * (usable_track - t_size);

                        let grab_offset =
                            if mouse_in_track >= t_pos && mouse_in_track <= t_pos + t_size {
                                mouse_in_track - t_pos
                            } else {
                                t_size / 2.0
                            };

                        this.set_grab_offset(grab_offset);
                        on_drag(
                            this,
                            track_id_for_drag.clone(),
                            viewport_size,
                            content_size,
                            position,
                            app,
                        );
                    }
                });
            }
        });

    track_container.into_any_element()
}
