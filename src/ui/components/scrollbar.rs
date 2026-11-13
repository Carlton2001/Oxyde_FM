use crate::ui::app::OxydeApp;
use crate::ui::elements::motion;
use crate::ui::scale::rem_pixels;
use crate::ui::theme::OxydeTheme;
use gpui::prelude::*;
use gpui::*;
use std::cell::RefCell;
pub(crate) mod scroll_top;

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

/// What the wheel scrolls while the pointer is over a rail, which lies beside
/// the content rather than inside its scrolling hitbox.
#[derive(Clone)]
pub(crate) enum WheelTarget {
    Handle(ScrollHandle),
    List(ListState),
}
impl From<ScrollHandle> for WheelTarget {
    fn from(handle: ScrollHandle) -> Self { Self::Handle(handle) }
}
impl From<ListState> for WheelTarget {
    fn from(list: ListState) -> Self { Self::List(list) }
}
impl WheelTarget {
    /// Moves by a wheel step as GPUI's scroll containers do, bounded by the rail's overflow.
    fn scroll(&self, axis: ScrollbarAxis, step: Pixels, maximum: f32) {
        match self {
            Self::Handle(handle) => {
                let mut offset = handle.offset();
                let along = if axis == ScrollbarAxis::Vertical { &mut offset.y } else { &mut offset.x };
                *along = (*along + step).clamp(px(-maximum), px(0.0));
                handle.set_offset(offset);
            }
            Self::List(list) => list.scroll_by(-step),
        }
    }

    /// Puts the content `scrolled` from its start, as a press beside the thumb moves
    /// it there. Lists measure their items as they go: their owners place them.
    fn jump(&self, axis: ScrollbarAxis, scrolled: f32) {
        if let Self::Handle(handle) = self {
            let mut offset = handle.offset();
            *(if axis == ScrollbarAxis::Vertical { &mut offset.y } else { &mut offset.x }) = px(-scrolled);
            handle.set_offset(offset);
        }
    }
}

pub const TRACK_WIDTH: Rems = rems(0.75);
pub fn track_width() -> f32 {
    rem_pixels(TRACK_WIDTH).as_f32()
}
const THUMB_MIN_SIZE: Rems = rems(2.5);
/// Gap between the painted thumb and the rail's ends.
const THUMB_END_INSET: Rems = rems(0.1875);
/// Thumb thickness and its gap to the rail's outer edge, both centered in the rail.
const THUMB_REST: (Rems, Rems) = (rems(0.375), rems(0.1875));
const THUMB_ACTIVE: (Rems, Rems) = (rems(0.625), rems(0.0625));

/// Wheel deltas reach the handle before GPUI clamps them at prepaint, so bound
/// the raw offset by the last measured overflow.
pub(crate) fn scrolled_from_top(handle: &ScrollHandle) -> bool {
    (-handle.offset().y).min(handle.max_offset().y) >= px(0.5)
}

/// Thumb length and travel along a rail, shared by painting, hit testing and dragging.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Thumb {
    pub(crate) size: f32,
    pub(crate) travel: f32,
    range: f32,
}

impl Thumb {
    /// `track` is the rail's length, which select menus extend past the viewport.
    pub(crate) fn new(viewport: f32, content: f32, track: f32, minimum: Option<f32>) -> Self {
        let track = track.max(0.0);
        let ratio = (viewport / content.max(1.0)).min(1.0);
        let minimum = minimum.unwrap_or_else(|| rem_pixels(THUMB_MIN_SIZE).as_f32());
        let size = (track * ratio).max(minimum).min(track);
        Self { size, travel: track - size, range: (content - viewport).max(0.0) }
    }

    /// Start of the thumb along the rail for a distance scrolled from the content's start.
    pub(crate) fn position(&self, scrolled: f32) -> f32 {
        (scrolled.max(0.0) / self.range.max(1.0)).clamp(0.0, 1.0) * self.travel
    }

    /// Distance scrolled from the content's start that puts the thumb's start at `position`.
    pub(crate) fn scrolled_at(&self, position: f32) -> f32 {
        (position / self.travel.max(1.0)).clamp(0.0, 1.0) * self.range
    }
}

/// Distance scrolled while a thumb grabbed `grab` from its start is held `along`
/// the rail from its origin. Every drag owner goes through this one conversion.
pub(crate) fn drag_scrolled(viewport: f32, content: f32, track: f32, along: f32, grab: f32) -> f32 {
    Thumb::new(viewport, content, track, None).scrolled_at(along - grab)
}

thread_local! {
    /// One pointer hovers at most one rail at a time, in any window.
    static HOVERED_TRACK: RefCell<Option<String>> = const { RefCell::new(None) };
}

fn hovered(track_id: &str) -> bool {
    HOVERED_TRACK.with(|hovered| hovered.borrow().as_deref() == Some(track_id))
}

/// Shared scrollbar for `OxydeApp`, whose drags carry no pointer position.
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
    wheel: impl Into<WheelTarget>,
    on_drag: impl Fn(&mut OxydeApp, String, f32, f32, &mut App) + 'static,
) -> AnyElement {
    let owner = handle.entity_id();
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
        wheel,
        move |view, track, viewport, content, position, cx| {
            on_drag(view, track, viewport, content, cx);
            // A press beside the thumb moves it there at once, before any pointer move.
            if let Some(target) = view.drag_target.clone() {
                if view.move_scrollbar_thumb(&target, position) {
                    cx.notify(owner);
                }
            }
        },
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
    wheel: impl Into<WheelTarget>,
    on_drag: impl Fn(&mut T, String, f32, f32, Point<Pixels>, &mut App) + 'static,
) -> AnyElement {
    render_scrollbar_on_track(theme, handle, axis, viewport_size, content_size, current_scroll, is_dragging, id_suffix, min_thumb_size, viewport_size, TrackShape::Square, wheel, on_drag)
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
    wheel: impl Into<WheelTarget>,
    on_drag: impl Fn(&mut T, String, f32, f32, Point<Pixels>, &mut App) + 'static,
) -> AnyElement {
    let is_v = axis == ScrollbarAxis::Vertical;

    // Don't render scrollbar if content fits
    if viewport_size >= content_size.max(1.0) {
        return div().into_any_element();
    }

    let usable_track = track_size.max(0.0);
    let geometry = Thumb::new(viewport_size, content_size, usable_track, min_thumb_size);
    let thumb_pos = geometry.position(current_scroll);
    // Inset only the painted grip. Keep its logical bounds for hit testing and
    // dragging so existing viewport/offset calculations retain their behavior.
    let grip_inset = rem_pixels(THUMB_END_INSET).as_f32().min(geometry.size / 2.0);
    let grip_size = (geometry.size - grip_inset * 2.0).max(0.0);

    let track_id = format!(
        "scrollbar-track-{}-{}",
        if is_v { "v" } else { "h" },
        id_suffix
    );
    let active = is_dragging || hovered(&track_id);
    let (rest_width, rest_edge) = (rem_pixels(THUMB_REST.0).as_f32(), rem_pixels(THUMB_REST.1).as_f32());
    let (active_width, active_edge) = (rem_pixels(THUMB_ACTIVE.0).as_f32(), rem_pixels(THUMB_ACTIVE.1).as_f32());
    let rest_color = theme.scrollbar_thumb;
    let active_color = if is_dragging { theme.scrollbar_thumb_drag } else { theme.scrollbar_thumb_hover };
    // Widening follows the pointer at the margin; the drag itself stays immediate.
    let thumb = div()
        .absolute()
        .rounded_full()
        .map(|thumb| if is_v {
            thumb.top(px(thumb_pos + grip_inset)).h(px(grip_size))
        } else {
            thumb.left(px(thumb_pos + grip_inset)).w(px(grip_size))
        })
        .with_spring(SharedString::from(format!("{track_id}-active")), motion::presence(active), move |thumb, progress| {
            let width = px(rest_width + (active_width - rest_width) * progress);
            let edge = px(rest_edge + (active_edge - rest_edge) * progress);
            let thumb = thumb.bg(rest_color.blend(active_color.opacity(progress)));
            if is_v { thumb.w(width).right(edge) } else { thumb.h(width).bottom(edge) }
        });

    let track_id_cap = track_id.clone();
    let handle_cap = handle.clone();
    let owner = handle.entity_id();
    let wheel: WheelTarget = wheel.into();
    let jump = wheel.clone();
    let overflow = (content_size - viewport_size).max(0.0);

    let track_container = div()
        .id(SharedString::from(track_id.clone()))
        .absolute()
        .bg(theme.scrollbar_track)
        .map(|track| match shape {
            TrackShape::Square => track,
            TrackShape::RoundedRight(radius) => track.rounded_r(radius),
        })
        .when(is_v, |s: Stateful<Div>| {
            s.top_0().right_0().bottom_0().w(px(track_width()))
        })
        .when(!is_v, |s: Stateful<Div>| {
            s.left_0().bottom_0().right_0().h(px(track_width()))
        })
        .on_hover({
            let track_id = track_id.clone();
            move |entered, window, _| {
                HOVERED_TRACK.with(|hovered| {
                    let mut hovered = hovered.borrow_mut();
                    if *entered {
                        *hovered = Some(track_id.clone());
                    } else if hovered.as_deref() == Some(track_id.as_str()) {
                        *hovered = None;
                    }
                });
                window.refresh();
            }
        })
        .on_scroll_wheel(move |event, window, cx| {
            // The content's own scrolling never sees this pointer position.
            cx.stop_propagation();
            let delta = event.delta.pixel_delta(window.line_height());
            // A vertical wheel also moves a horizontal rail, as GTK and Windows do.
            let step = if is_v { delta.y } else if delta.x.is_zero() { delta.y } else { delta.x };
            if step.is_zero() {
                return;
            }
            crate::ui::components::tooltip::hover::dismiss(window, cx);
            wheel.scroll(axis, step, overflow);
            cx.notify(owner);
            window.refresh();
        })
        .child(
            canvas(
                |bounds, _, _| bounds,
                move |bounds, _, window, cx| {
                    // A rail removed while hovered, such as a closed menu's, gets no leave event.
                    if hovered(&track_id_cap) && !bounds.contains(&window.mouse_position()) {
                        HOVERED_TRACK.with(|hovered| hovered.borrow_mut().take());
                        window.refresh();
                    }
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
                        let t_pos = geometry.position(current_scroll);

                        // A press on the thumb keeps its grip; elsewhere the thumb
                        // centers under the pointer, as GTK warps it.
                        let on_thumb = mouse_in_track >= t_pos && mouse_in_track <= t_pos + geometry.size;
                        let grab_offset = if on_thumb { mouse_in_track - t_pos } else { geometry.size / 2.0 };
                        // Every owner sees the thumb under the pointer at once, before
                        // any move; owners that place it themselves then do the same.
                        if !on_thumb {
                            jump.jump(axis, geometry.scrolled_at(mouse_in_track - grab_offset));
                            app.notify();
                            window.refresh();
                        }

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

    // While the thumb is dragged, nothing else in the window reacts to the pointer passing over it.
    // Every owner tracks the drag with window-level handlers, which this hitbox does not intercept.
    track_container
        .when(is_dragging, |track| track.child(deferred(canvas(
            |_, window, _| window.insert_hitbox(Bounds::new(Point::default(), window.viewport_size()), HitboxBehavior::BlockMouse),
            |_, _, _, _| {},
        ).absolute().size_0()).with_priority(100)))
        .into_any_element()
}

#[cfg(test)]
#[path = "scrollbar/tests/thumb.rs"]
mod tests;
