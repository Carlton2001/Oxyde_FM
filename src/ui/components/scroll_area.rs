//! Locally owned scrolling for forms and menus, using the shared scrollbar skin.
use super::scrollbar::{render_scrollbar_with_pointer, ScrollbarAxis, ScrollbarState, ZED_TRACK_WIDTH};
use crate::ui::{scale::rem_pixels, theme::ThemeContext};
use gpui::{prelude::*, *};
use std::collections::HashMap;

#[derive(Default)]
struct ScrollAreaState {
    scroll: ScrollHandle,
    viewport: Size<Pixels>,
    maximum: Point<Pixels>,
    offset: Point<Pixels>,
    origins: HashMap<String, Point<f32>>,
    grab: f32,
    drag: Option<(ScrollbarAxis, String, f32, f32)>,
}

impl ScrollbarState for ScrollAreaState {
    fn scrollbar_origins(&mut self) -> &mut HashMap<String, Point<f32>> { &mut self.origins }
    fn set_grab_offset(&mut self, offset: f32) { self.grab = offset; }
}

fn drag_offset(viewport: f32, content: f32, position: f32, grab: f32) -> f32 {
    let thumb = (viewport * viewport / content.max(1.0))
        .max(rem_pixels(rems(2.5)).as_f32()).min(viewport);
    let travel = (viewport - thumb).max(1.0);
    -((position - grab) / travel).clamp(0.0, 1.0) * (content - viewport).max(0.0)
}

impl ScrollAreaState {
    fn move_drag(&mut self, position: Point<Pixels>) {
        let Some((axis, track, viewport, content)) = &self.drag else { return; };
        let Some(origin) = self.origins.get(track) else { return; };
        let mut offset = self.scroll.offset();
        match axis {
            ScrollbarAxis::Vertical => offset.y = px(drag_offset(*viewport, *content, position.y.as_f32() - origin.y, self.grab)),
            ScrollbarAxis::Horizontal => offset.x = px(drag_offset(*viewport, *content, position.x.as_f32() - origin.x, self.grab)),
        }
        self.scroll.set_offset(offset);
    }
}

/// The caller retains the viewport ID, content, padding and size constraints.
/// The outer flex frame must constrain the viewport whenever scrolling is needed.
pub(crate) fn scroll_area(
    id: impl Into<ElementId>,
    frame: Div,
    viewport: Stateful<Div>,
    horizontal: bool,
    window: &mut Window,
    cx: &mut App,
) -> Div {
    let state = window.use_keyed_state(id.into(), cx, |_, _| ScrollAreaState::default());
    let data = state.read(cx);
    let scroll = data.scroll.clone();
    let metrics = data.viewport;
    let maximum = data.maximum;
    let offset = data.offset;
    let dragging = data.drag.as_ref().map(|drag| drag.0);
    let overflow_v = maximum.y > px(0.0);
    let overflow_h = horizontal && maximum.x > px(0.0);
    let suffix = format!("form-scroll-{:?}", state.entity_id());
    let theme = *cx.theme();
    let mut frame = frame.relative().flex().flex_col().min_h_0().min_w_0()
        .child(viewport.min_h_0().min_w_0().flex_shrink(1.0).overflow_y_scroll().track_scroll(&scroll)
            .when(horizontal, |viewport| viewport.overflow_x_scroll())
            .when(overflow_v, |viewport| viewport.mr(ZED_TRACK_WIDTH))
            .when(overflow_h, |viewport| viewport.mb(ZED_TRACK_WIDTH))
            .on_scroll_wheel(|_, window, _| window.refresh()));
    for axis in [ScrollbarAxis::Vertical, ScrollbarAxis::Horizontal] {
        if axis == ScrollbarAxis::Horizontal && !horizontal { continue; }
        let (viewport, maximum, offset) = if axis == ScrollbarAxis::Vertical {
            (metrics.height, maximum.y, offset.y)
        } else { (metrics.width, maximum.x, offset.x) };
        let owner = window.window_handle();
        let rail = render_scrollbar_with_pointer(theme, state.clone(), axis,
            viewport.as_f32(), (viewport + maximum).as_f32(), -offset.as_f32(),
            dragging == Some(axis), suffix.clone(), None,
            move |state, track, viewport, content, position, cx| {
                state.drag = Some((axis, track, viewport, content));
                state.move_drag(position);
                cx.defer(move |cx| { let _ = owner.update(cx, |_, window, _| window.refresh()); });
            });
        frame = frame.child(div().absolute().inset_0()
            .when(axis == ScrollbarAxis::Vertical && overflow_h, |rail| rail.bottom(ZED_TRACK_WIDTH))
            .when(axis == ScrollbarAxis::Horizontal && overflow_v, |rail| rail.right(ZED_TRACK_WIDTH))
            .child(rail));
    }
    let measure = state.clone();
    frame.child(canvas(|_, _, _| (), move |_, _, window, cx| {
        let changed = measure.update(cx, |state, _| {
            let metrics = (state.scroll.bounds().size, state.scroll.max_offset(), state.scroll.offset());
            let changed = metrics != (state.viewport, state.maximum, state.offset);
            (state.viewport, state.maximum, state.offset) = metrics;
            changed
        });
        if changed { window.refresh(); }
        let movement = measure.clone();
        window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, cx| {
            if phase != DispatchPhase::Capture || movement.read(cx).drag.is_none() { return; }
            let position = super::popup::owner_point(window, cx, event.position);
            movement.update(cx, |state, _| {
                if event.dragging() { state.move_drag(position); } else { state.drag = None; }
            });
            window.refresh();
        });
        let release = measure.clone();
        window.on_mouse_event(move |event: &MouseUpEvent, phase, window, cx| {
            if phase == DispatchPhase::Capture && event.button == MouseButton::Left {
                if release.update(cx, |state, _| state.drag.take().is_some()) { window.refresh(); }
            }
        });
    }).absolute().size_0())
}

#[cfg(test)]
#[path = "scroll_area/tests/drag.rs"]
mod tests;
