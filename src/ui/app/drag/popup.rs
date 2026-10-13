//! Menus can update their scroll drag without hit testing the source scene.
use super::{DragTarget, OxydeApp};
use gpui::*;

fn is_popup_scroll(target: Option<&DragTarget>) -> bool {
    matches!(target, Some(DragTarget::ContextMenuV { .. } | DragTarget::BreadcrumbV { .. }))
}

impl OxydeApp {
    pub(crate) fn move_popup_scroll(&mut self, event: &MouseMoveEvent, window: &mut Window, cx: &mut Context<Self>) {
        if is_popup_scroll(self.drag_target.as_ref()) {
            self.handle_pointer_move(event, window, cx);
        }
    }

    pub(crate) fn release_popup_scroll(&mut self, event: &MouseUpEvent, cx: &mut Context<Self>) {
        if event.button == MouseButton::Left && is_popup_scroll(self.drag_target.as_ref()) {
            self.finish_pointer_drag(cx);
        }
    }
}

#[cfg(test)]
#[path = "tests/popup_scroll.rs"]
mod tests;
