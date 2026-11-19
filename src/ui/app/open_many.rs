//! Opening many selected items at once starts as many applications: past
//! `CONFIRM_ABOVE`, the user confirms first, as in Explorer.
use super::*;

/// Explorer asks from 16 items on.
pub(super) const CONFIRM_ABOVE: usize = 15;

pub(crate) struct OpenManyConfirmation {
    pane_id: String,
    paths: Vec<(String, bool)>,
    pub focus: FocusHandle,
    pub confirm_focus: FocusHandle,
}

impl OpenManyConfirmation {
    pub(crate) fn count(&self) -> usize {
        self.paths.len()
    }
}

impl OxydeApp {
    pub(super) fn ask_open_many(&mut self, pane_id: String, paths: Vec<(String, bool)>, cx: &mut Context<Self>) {
        if self.open_many.is_some() {
            return;
        }
        self.open_many = Some(OpenManyConfirmation {
            pane_id,
            paths,
            focus: cx.focus_handle(),
            confirm_focus: cx.focus_handle(),
        });
        cx.notify();
    }

    /// Opens the items, or leaves them as they are.
    pub(crate) fn resolve_open_many(&mut self, open: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some(confirmation) = self.open_many.take() else { return };
        window.focus(&self.focus_handle, cx);
        if open {
            for (path, is_dir) in confirmation.paths {
                self.open_file_item(&confirmation.pane_id, path, is_dir, cx);
            }
        }
        cx.notify();
    }
}
