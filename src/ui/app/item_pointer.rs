use super::{DragTarget, OxydeApp};
use gpui::*;
use std::collections::HashSet;

pub(super) struct ItemPress {
    pane_id: String,
    position: Point<Pixels>,
    modifiers: Modifiers,
    initial_selection: HashSet<String>,
    was_selected: bool,
}

fn gesture(was_selected: bool, distance: f64, threshold: f64) -> Option<bool> {
    if was_selected {
        // Let GPUI apply its ordinary drag threshold without adding a delay.
        Some(true)
    } else {
        (distance > threshold).then_some(false)
    }
}

impl OxydeApp {
    /// Record the selection before mouse-down changes it, for additive rectangles.
    pub(crate) fn begin_item_press(&mut self, pane_id: &str, path: &str, event: &MouseDownEvent) {
        self.item_selection_drag = false;
        self.item_press = (event.click_count == 1).then(|| ItemPress {
            pane_id: pane_id.to_owned(),
            position: event.position,
            modifiers: event.modifiers,
            initial_selection: self
                .state
                .find_pane(pane_id)
                .map(|pane| pane.active_tab().selected_paths.clone())
                .unwrap_or_default(),
            was_selected: self
                .state
                .find_pane(pane_id)
                .is_some_and(|pane| pane.active_tab().selected_paths.contains(path)),
        });
    }

    /// Capture movements until the gesture is decided, before GPUI starts a file drag.
    pub(super) fn move_item_press(&mut self, event: &MouseMoveEvent, cx: &mut Context<Self>) {
        if let Some(press) = &self.item_press {
            let decision = gesture(
                press.was_selected,
                (event.position - press.position).magnitude(),
                crate::ui::scale::logical(4.0) as f64,
            );
            match decision {
                None => {
                    cx.stop_propagation();
                    return;
                }
                Some(true) => {
                    self.item_press = None;
                }
                Some(false) => {
                    let press = self.item_press.take().expect("pending item press");
                    self.item_selection_drag = true;
                    self.drag_target = Some(DragTarget::SelectionRectangle {
                        pane_id: press.pane_id,
                        start_pos: press.position,
                        current_pos: event.position,
                        modifiers: press.modifiers,
                        initial_selection: press.initial_selection,
                    });
                    cx.notify();
                }
            }
        }
        if self.item_selection_drag {
            cx.stop_propagation();
        }
    }
}

#[cfg(test)]
#[path = "item_pointer/tests/gestures.rs"]
mod tests;
