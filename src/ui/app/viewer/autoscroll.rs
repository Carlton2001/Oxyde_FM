//! Scrolling a text or Markdown view while a selection is dragged past its top or
//! bottom edge, faster the farther the pointer goes, as in editors.
use super::{content::Content, Viewer};
use gpui::*;
use std::time::Duration;

const TICK: Duration = Duration::from_millis(16);

#[derive(Default)]
pub(super) struct AutoScroll {
    pointer: Option<Point<Pixels>>,
    task: Option<Task<()>>,
}

/// Pixels to scroll per tick for a pointer `beyond` the edge (negative above).
pub(super) fn speed(beyond: Pixels) -> Pixels {
    let distance = beyond.abs().min(px(160.0));
    let step = px(2.0) + distance * 0.25;
    if beyond < px(0.0) { -step } else { step }
}

impl Viewer {
    fn selection_viewport(&self) -> Option<(ListState, bool)> {
        match &self.content {
            Content::Text(text) if text.selecting => Some((text.list.clone(), true)),
            Content::Markdown(document) if document.selecting => Some((document.list.clone(), false)),
            _ => None,
        }
    }

    /// A window-wide listener, registered while painting: rows only hear the
    /// pointer while it is over them.
    pub(super) fn selection_drag_listener(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let active = self.selection_viewport().is_some();
        let handle = cx.entity().downgrade();
        canvas(|_, _, _| (), move |_, _, window, _| {
            if !active {
                return;
            }
            let handle = handle.clone();
            window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, cx| {
                if phase != DispatchPhase::Bubble {
                    return;
                }
                let pressed = event.pressed_button == Some(MouseButton::Left);
                let _ = handle.update(cx, |this, cx| this.selection_dragged(pressed.then_some(event.position), window, cx));
            });
        })
        .absolute()
        .size_0()
    }

    fn selection_dragged(&mut self, pointer: Option<Point<Pixels>>, window: &Window, cx: &mut Context<Self>) {
        let Some((list, _)) = self.selection_viewport() else {
            self.autoscroll = AutoScroll::default();
            return;
        };
        let viewport = list.viewport_bounds();
        let outside = pointer.filter(|pointer| pointer.y < viewport.top() || pointer.y > viewport.bottom());
        self.autoscroll.pointer = outside;
        if outside.is_none() {
            self.autoscroll.task = None;
            if pointer.is_none() {
                self.text_pointer_released();
                self.markdown_pointer_released();
            }
            return;
        }
        if self.autoscroll.task.is_some() {
            return;
        }
        let window_handle = window.window_handle();
        self.autoscroll.task = Some(cx.spawn(async move |this, cx| loop {
            cx.background_executor().timer(TICK).await;
            let keep = window_handle.update(cx, |_, window, cx| {
                this.update(cx, |this, cx| this.autoscroll_tick(window, cx)).unwrap_or(false)
            });
            if !matches!(keep, Ok(true)) {
                break;
            }
        }));
    }

    /// Scrolls one step, then extends the selection to the row at the edge.
    fn autoscroll_tick(&mut self, window: &Window, cx: &mut Context<Self>) -> bool {
        let (Some(pointer), Some((list, text))) = (self.autoscroll.pointer, self.selection_viewport()) else {
            self.autoscroll = AutoScroll::default();
            return false;
        };
        let viewport = list.viewport_bounds();
        let beyond = if pointer.y < viewport.top() { pointer.y - viewport.top() } else { pointer.y - viewport.bottom() };
        list.scroll_by(speed(beyond));
        let edge = point(pointer.x, pointer.y.clamp(viewport.top() + px(1.0), viewport.bottom() - px(1.0)));
        let top = list.logical_scroll_top();
        if text {
            let Content::Text(file) = &self.content else { return false };
            let height = file.row_height.max(px(1.0));
            let index = top.item_ix + ((edge.y - viewport.top() + top.offset_in_item) / height).floor() as usize;
            self.text_pointer(index, edge, false, false, 1, window, cx);
        } else {
            let Content::Markdown(document) = &self.content else { return false };
            let count = document.block_count();
            let index = (top.item_ix..count)
                .find(|index| list.bounds_for_item(*index).is_some_and(|bounds| bounds.bottom() >= edge.y))
                .unwrap_or(count.saturating_sub(1));
            let length = document.block_len(index);
            let offset = if beyond < px(0.0) { 0 } else { length };
            self.markdown_pointer(index, offset, false, false, 1, cx);
        }
        cx.notify();
        true
    }
}

#[cfg(test)]
#[path = "tests/autoscroll.rs"]
mod tests;
