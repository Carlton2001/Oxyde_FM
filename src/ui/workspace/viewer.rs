use super::{TabLoadStatus, TabState, WorkspaceState};

pub(crate) const VIEWER_PATH: &str = "viewer://";

impl TabState {
    pub(crate) fn is_viewer(&self) -> bool {
        self.path == VIEWER_PATH
    }
}

impl WorkspaceState {
    /// At most one Viewer exists; a second request is ignored.
    pub(crate) fn add_viewer(&mut self) -> Option<String> {
        if self.viewer_tab_id().is_some() {
            return None;
        }
        let previous = self.active_pane_id.clone();
        let pane_id = self.add_panel_right()?;
        let tab = self.find_pane_mut(&pane_id)?.active_tab_mut();
        tab.path = VIEWER_PATH.into();
        tab.load_status = TabLoadStatus::Ready;
        let id = tab.id.clone();
        self.keep_explorer_active(&previous);
        Some(id)
    }

    pub(crate) fn viewer_tab_id(&self) -> Option<String> {
        let mut found = None;
        self.layout.visit_panes(&mut |pane| {
            if let Some(tab) = pane.tabs.iter().find(|tab| tab.is_viewer()) {
                found = Some(tab.id.clone());
            }
        });
        found
    }

    pub(crate) fn is_viewer_pane(&self, pane_id: &str) -> bool {
        self.find_pane(pane_id).is_some_and(|pane| pane.active_tab().is_viewer())
    }

    pub(crate) fn viewer_pane_id(&self, tab_id: &str) -> Option<String> {
        let mut found = None;
        self.layout.visit_panes(&mut |pane| {
            if pane.tabs.iter().any(|tab| tab.id == tab_id) {
                found = Some(pane.id.clone());
            }
        });
        found
    }

    /// A Viewer pane is never the active pane: its commands act on itself while
    /// the explorer pane keeps the selection, keyboard and toolbar actions.
    pub(crate) fn keep_explorer_active(&mut self, previous: &str) {
        if !self.is_viewer_pane(&self.active_pane_id) {
            return;
        }
        if self.find_pane(previous).is_some() && !self.is_viewer_pane(previous) {
            self.active_pane_id = previous.to_string();
            return;
        }
        let mut fallback = None;
        self.layout.visit_panes(&mut |pane| {
            if fallback.is_none() && !pane.active_tab().is_viewer() {
                fallback = Some(pane.id.clone());
            }
        });
        if let Some(pane) = fallback {
            self.active_pane_id = pane;
        }
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
