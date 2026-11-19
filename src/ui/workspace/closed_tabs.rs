//! Recently closed tabs, reopened most recent first.

use super::{PaneState, TabState, WorkspaceState};
use std::collections::VecDeque;

/// Tabs remembered for reopening, like a browser's recently closed list.
pub(crate) const CLOSED_TAB_HISTORY: usize = 25;

/// What a closed tab needs to come back where it was.
#[derive(Clone)]
pub(crate) struct ClosedTab {
    pane_id: String,
    index: usize,
    path: String,
    view_mode: super::ViewMode,
    group_mode: crate::ui::date_groups::GroupMode,
    sort_column: super::SortColumn,
    sort_direction: super::SortDirection,
    back_stack: Vec<String>,
    forward_stack: Vec<String>,
}

#[derive(Clone, Default)]
pub(crate) struct ClosedTabs(VecDeque<ClosedTab>);

impl ClosedTabs {
    pub(crate) fn is_empty(&self) -> bool { self.0.is_empty() }
}

impl WorkspaceState {
    /// Remember a tab of `pane_id` at `index` before it closes. The viewer is not a location.
    pub(super) fn remember_closed_tab(&mut self, pane_id: &str, index: usize) {
        let Some(tab) = self.find_pane(pane_id).and_then(|pane| pane.tabs.get(index)) else { return };
        if tab.is_viewer() { return; }
        let closed = ClosedTab {
            pane_id: pane_id.to_string(),
            index,
            path: tab.path.clone(),
            view_mode: tab.view_mode,
            group_mode: tab.group_mode,
            sort_column: tab.sort_column,
            sort_direction: tab.sort_direction,
            back_stack: tab.back_stack.clone(),
            forward_stack: tab.forward_stack.clone(),
        };
        let history = &mut self.closed_tabs.0;
        history.push_front(closed);
        history.truncate(CLOSED_TAB_HISTORY);
    }

    /// Reopen the last closed tab in the pane it left, or else in the active pane,
    /// and return that pane with the tab's index.
    pub fn reopen_closed_tab(&mut self) -> Option<(String, usize)> {
        let closed = self.closed_tabs.0.pop_front()?;
        let usable = |pane: &PaneState| !pane.active_tab().is_viewer();
        let pane_id = if self.find_pane(&closed.pane_id).is_some_and(usable) {
            closed.pane_id.clone()
        } else if self.find_pane(&self.active_pane_id).is_some_and(usable) {
            self.active_pane_id.clone()
        } else {
            let mut first = None;
            self.layout.visit_panes(&mut |pane| if first.is_none() && usable(pane) { first = Some(pane.id.clone()) });
            first?
        };
        let id = format!("tab-{}", self.next_tab_id);
        self.next_tab_id = self.next_tab_id.wrapping_add(1);
        let mut tab = TabState::new(id, closed.path);
        tab.opened_at = Some(std::time::Instant::now());
        tab.view_mode = closed.view_mode;
        tab.group_mode = closed.group_mode;
        tab.sort_column = closed.sort_column;
        tab.sort_direction = closed.sort_direction;
        tab.back_stack = closed.back_stack;
        tab.forward_stack = closed.forward_stack;
        let pane = self.find_pane_mut(&pane_id)?;
        let index = if pane_id == closed.pane_id { closed.index.min(pane.tabs.len()) } else { pane.active_tab_index + 1 };
        pane.tabs.insert(index, tab);
        pane.active_tab_index = index;
        self.active_pane_id = pane_id.clone();
        Some((pane_id, index))
    }
}

#[cfg(test)]
#[path = "tests/closed_tabs.rs"]
mod tests;
