use super::TextInput;
use crate::ui::components::scrollbar::{
    render_scrollbar_on_track, ScrollbarAxis, ScrollbarState,
};
use crate::ui::theme::ThemeContext;
use gpui::{prelude::*, *};
use std::collections::HashMap;

#[derive(Default)]
pub(in crate::ui::components::input) struct PointerState {
    origins: HashMap<String, Point<f32>>,
    grab_offset: f32,
    drag: Option<(String, f32, f32)>,
}

impl ScrollbarState for TextInput {
    fn scrollbar_origins(&mut self) -> &mut HashMap<String, Point<f32>> {
        &mut self.completion_pointer.origins
    }

    fn set_grab_offset(&mut self, offset: f32) {
        self.completion_pointer.grab_offset = offset;
    }
}

fn update_drag(input: &mut TextInput, position: Point<Pixels>) {
    let state = &input.completion_pointer;
    let Some((track, viewport, content)) = &state.drag else {
        return;
    };
    let Some(origin) = state.origins.get(track) else {
        return;
    };
    let minimum = crate::ui::scale::rem_pixels(rems(2.5))
        .as_f32()
        .min(*viewport);
    let track_size = crate::ui::components::select_menu::scrollbar_track_size(*viewport);
    let usable_track = track_size.max(0.0);
    let thumb = (usable_track * viewport / content).max(minimum).min(usable_track);
    let travel = (usable_track - thumb).max(1.0);
    let ratio = ((position.y.as_f32() - origin.y - state.grab_offset) / travel).clamp(0.0, 1.0);
    input
        .completion_scroll
        .0
        .borrow()
        .base_handle
        .set_offset(point(px(0.0), px(-ratio * (content - viewport))));
}

pub(super) fn render(
    input: &TextInput,
    viewport: Pixels,
    content: Pixels,
    cx: &Context<TextInput>,
) -> AnyElement {
    let handle = cx.entity();
    let mouse = handle.clone();
    let drag_handle = handle.clone();
    let scroll = input.completion_scroll.0.borrow().base_handle.clone();
    let rendered_offset = scroll.offset();
    let viewport = viewport.as_f32();
    crate::ui::components::select_menu::scrollbar_overlay()
        .child(render_scrollbar_on_track(
            *cx.theme(),
            handle,
            ScrollbarAxis::Vertical,
            viewport,
            content.as_f32(),
            scroll.offset().y.as_f32().abs(),
            input.completion_pointer.drag.is_some(),
            "path-completion".into(),
            Some(
                crate::ui::scale::rem_pixels(rems(2.5))
                    .as_f32()
                    .min(viewport),
            ),
            crate::ui::components::select_menu::scrollbar_track_size(viewport),
            crate::ui::components::select_menu::scrollbar_track_shape(),
            move |input, track, viewport, content, position, cx| {
                input.completion_pointer.drag = Some((track, viewport, content));
                update_drag(input, position);
                cx.notify(drag_handle.entity_id());
            },
        ))
        .child(
            canvas(
                |_, _, _| (),
                move |_, _, window, cx| {
                    // Keyboard scroll requests resolve during layout, after the
                    // thumb was built. Refresh once with the resolved offset.
                    if scroll.offset() != rendered_offset {
                        let refresh = mouse.clone();
                        cx.defer(move |cx| refresh.update(cx, |_, cx| cx.notify()));
                    }
                    let moving = mouse.clone();
                    window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, cx| {
                        if phase != DispatchPhase::Capture {
                            return;
                        }
                        let position =
                            crate::ui::components::popup::owner_point(window, cx, event.position);
                        moving.update(cx, |input, cx| {
                            if input.completion_pointer.drag.is_some() {
                                if event.dragging() {
                                    update_drag(input, position);
                                } else {
                                    input.completion_pointer.drag = None;
                                }
                                cx.notify();
                            }
                        });
                    });
                    let released = mouse.clone();
                    window.on_mouse_event(move |event: &MouseUpEvent, phase, _, cx| {
                        if phase == DispatchPhase::Capture && event.button == MouseButton::Left {
                            released.update(cx, |input, cx| {
                                if input.completion_pointer.drag.take().is_some() {
                                    cx.notify();
                                }
                            });
                        }
                    });
                },
            )
            .absolute()
            .size_0(),
        )
        .into_any_element()
}
