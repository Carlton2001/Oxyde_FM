//! Extending the selection, as native fields do: Shift with the arrows, Home or End
//! moves its free end from the fixed one; a drag follows the pointer even outside the
//! field and, past its edge, keeps selecting while the text scrolls, faster the farther
//! the pointer goes. The caret stays in view through `horizontal_scroll_offset`.
use super::TextInput;
use crate::ui::scale::rem_pixels;
use gpui::*;
use std::ops::Range;
use std::time::Duration;
use unicode_segmentation::UnicodeSegmentation;

actions!(input, [SelectLeft, SelectRight, SelectHome, SelectEnd]);

pub(super) fn bindings() -> [KeyBinding; 4] {
    [
        KeyBinding::new("shift-left", SelectLeft, Some("TextInput")),
        KeyBinding::new("shift-right", SelectRight, Some("TextInput")),
        KeyBinding::new("shift-home", SelectHome, Some("TextInput")),
        KeyBinding::new("shift-end", SelectEnd, Some("TextInput")),
    ]
}

/// The pointer of a selection drag, and the timer scrolling while it is past an edge.
#[derive(Default)]
pub(super) struct Drag {
    pointer: Option<Point<Pixels>>,
    autoscroll: Option<Task<()>>,
}

/// Between two steps of a drag past an edge.
const AUTOSCROLL_TICK: Duration = Duration::from_millis(40);

/// The selection from `anchor` to `caret`, and whether the caret is its start.
fn span(anchor: usize, caret: usize) -> (Range<usize>, bool) {
    (anchor.min(caret)..anchor.max(caret), caret < anchor)
}

fn previous_boundary(text: &str, index: usize) -> usize {
    text.grapheme_indices(true).rev().map(|(i, _)| i).find(|i| *i < index).unwrap_or(0)
}

fn next_boundary(text: &str, index: usize) -> usize {
    text.grapheme_indices(true).map(|(i, _)| i).find(|i| *i > index).unwrap_or(text.len())
}

/// Characters selected per tick past an edge: one at the edge, one more per rem beyond.
fn autoscroll_steps(beyond: f32, rem: f32) -> usize {
    1 + (beyond.abs() / rem.max(1.0)) as usize
}

/// While a drag selects, follows the pointer over the whole window, not only the field.
pub(super) fn follow_drag(input: &Entity<TextInput>, window: &mut Window, cx: &App) {
    if !input.read(cx).is_selecting {
        return;
    }
    let input = input.clone();
    window.on_mouse_event(move |event: &MouseMoveEvent, phase, _, cx| {
        if phase == DispatchPhase::Bubble {
            input.update(cx, |input, cx| input.drag(event, cx));
        }
    });
}

impl TextInput {
    fn caret(&self) -> usize {
        if self.selection_reversed { self.selected_range.start } else { self.selected_range.end }
    }

    fn select_span(&mut self, anchor: usize, caret: usize) {
        let (range, reversed) = span(anchor, caret);
        self.selected_range = range;
        self.selection_reversed = reversed;
        self.selection_anchor = anchor;
    }

    /// Moves the free end of the selection, the other one staying.
    fn extend_to(&mut self, caret: usize, cx: &mut Context<Self>) {
        let anchor = if self.selection_reversed { self.selected_range.end } else { self.selected_range.start };
        self.select_span(anchor, caret);
        self.reset_blinking(cx);
    }

    pub(super) fn select_left(&mut self, _: &SelectLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.extend_to(previous_boundary(&self.content, self.caret()), cx);
    }

    pub(super) fn select_right(&mut self, _: &SelectRight, _: &mut Window, cx: &mut Context<Self>) {
        self.extend_to(next_boundary(&self.content, self.caret()), cx);
    }

    pub(super) fn select_home(&mut self, _: &SelectHome, _: &mut Window, cx: &mut Context<Self>) {
        self.extend_to(0, cx);
    }

    pub(super) fn select_end(&mut self, _: &SelectEnd, _: &mut Window, cx: &mut Context<Self>) {
        self.extend_to(self.content.len(), cx);
    }

    pub(super) fn end_drag(&mut self) {
        self.is_selecting = false;
        self.drag = Drag::default();
    }

    fn drag(&mut self, event: &MouseMoveEvent, cx: &mut Context<Self>) {
        if !event.dragging() {
            self.end_drag();
            return;
        }
        if !self.is_selecting {
            return;
        }
        self.drag.pointer = Some(event.position);
        let Some(bounds) = self.last_bounds else { return };
        let outside = self.beyond_edge().is_some();
        // Past an edge, the timer moves the caret; the pointer only sets it on the way out.
        if !outside || self.drag.autoscroll.is_none() {
            let x = event.position.x.clamp(bounds.left(), bounds.right());
            let index = self.byte_index_for_position(point(x, event.position.y));
            self.select_span(self.selection_anchor, index);
            self.cursor_visible = self.selected_range.is_empty();
        }
        if outside && self.drag.autoscroll.is_none() {
            self.drag.autoscroll = Some(cx.spawn(async move |this, cx| loop {
                cx.background_executor().timer(AUTOSCROLL_TICK).await;
                if !this.update(cx, |this, cx| this.autoscroll_step(cx)).unwrap_or(false) {
                    break;
                }
            }));
        }
        cx.notify();
    }

    /// How far left (negative) or right of the field the drag pointer is.
    fn beyond_edge(&self) -> Option<f32> {
        let (pointer, bounds) = (self.drag.pointer?, self.last_bounds?);
        if pointer.x < bounds.left() {
            Some((pointer.x - bounds.left()).as_f32())
        } else if pointer.x > bounds.right() {
            Some((pointer.x - bounds.right()).as_f32())
        } else {
            None
        }
    }

    fn autoscroll_step(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(beyond) = self.beyond_edge().filter(|_| self.is_selecting) else {
            self.drag.autoscroll = None;
            return false;
        };
        let mut caret = self.caret();
        for _ in 0..autoscroll_steps(beyond, rem_pixels(rems(1.0)).as_f32()) {
            caret = if beyond < 0.0 { previous_boundary(&self.content, caret) } else { next_boundary(&self.content, caret) };
        }
        self.select_span(self.selection_anchor, caret);
        self.cursor_visible = self.selected_range.is_empty();
        cx.notify();
        true
    }
}

#[cfg(test)]
#[path = "tests/selection.rs"]
mod tests;
