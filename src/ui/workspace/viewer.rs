use super::{TabLoadStatus, TabState, WorkspaceState};

pub(crate) const VIEWER_PATH: &str = "viewer://";

impl TabState {
    pub(crate) fn is_viewer(&self) -> bool {
        self.path == VIEWER_PATH
    }
}

impl WorkspaceState {
    pub(crate) fn add_viewer(&mut self) -> Option<String> {
        let pane_id = self.add_panel_right()?;
        let tab = self.find_pane_mut(&pane_id)?.active_tab_mut();
        tab.path = VIEWER_PATH.into();
        tab.load_status = TabLoadStatus::Ready;
        Some(tab.id.clone())
    }

    /// A Viewer always owns its pane; enforce this below every UI drop handler.
    pub(crate) fn can_merge_tab(&self, source: &str, index: usize, target: &str) -> bool {
        let Some(tab) = self.find_pane(source).and_then(|pane| pane.tabs.get(index)) else {
            return false;
        };
        let Some(pane) = self.find_pane(target) else {
            return false;
        };
        !tab.is_viewer() && !pane.tabs.iter().any(TabState::is_viewer)
    }
}

#[cfg(test)]
#[path = "tests/viewer.rs"]
mod tests;
