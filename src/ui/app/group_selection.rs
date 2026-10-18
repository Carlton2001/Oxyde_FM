use super::OxydeApp;
use crate::ui::date_groups::{group_for_file, GroupKey, GroupMode};
use crate::ui::workspace::TabState;
use gpui::Context;
use oxyde_lib::models::FileEntry;
use std::collections::HashSet;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GroupSelection {
    None,
    Partial,
    All,
}
impl GroupSelection {
    pub(crate) fn for_members(
        files: &[FileEntry],
        indices: &[usize],
        selected: &HashSet<String>,
    ) -> Self {
        let count = indices
            .iter()
            .filter(|index| selected.contains(&files[**index].path))
            .count();
        if count == 0 {
            Self::None
        } else if count == indices.len() {
            Self::All
        } else {
            Self::Partial
        }
    }
}

impl OxydeApp {
    pub(crate) fn toggle_group_selection(
        &mut self,
        pane_id: &str,
        group: &GroupKey,
        cx: &mut Context<Self>,
    ) {
        self.finish_pointer_drag(cx);
        self.activate_pane(pane_id, cx);
        if let Some(pane) = self.state.find_pane_mut(pane_id) {
            toggle_group(pane.active_tab_mut(), group);
        }
        cx.notify();
    }
}

fn toggle_group(tab: &mut TabState, group: &GroupKey) {
    let mode = tab.grouping();
    if mode == GroupMode::None {
        return;
    }
    let today = chrono::Local::now().date_naive();
    // Use the current filtered snapshot, including members of collapsed groups.
    // Resolve on click so navigation, filters, and manual size changes cannot use stale paths.
    let paths: Vec<_> = tab
        .filtered_files()
        .iter()
        .filter(|file| &group_for_file(file, mode, today) == group)
        .map(|file| file.path.clone())
        .collect();
    let Some(first) = paths.first().cloned() else {
        return;
    };
    if paths.iter().all(|path| tab.selected_paths.contains(path)) {
        for path in &paths {
            tab.selected_paths.remove(path);
        }
        if tab
            .selection_anchor
            .as_ref()
            .is_some_and(|path| !tab.selected_paths.contains(path))
        {
            tab.selection_anchor = None;
        }
        if tab
            .keyboard_cursor
            .as_ref()
            .is_some_and(|path| !tab.selected_paths.contains(path))
        {
            tab.keyboard_cursor = None;
        }
    } else {
        tab.selected_paths.extend(paths);
        tab.selection_anchor = Some(first.clone());
        tab.keyboard_cursor = Some(first);
    }
}

#[cfg(test)]
#[path = "group_selection/tests/selection.rs"]
mod tests;
