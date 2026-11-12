//! Type-to-select: typed letters move the selection to the next matching name.

use super::OxydeApp;
use super::selection::scroll_row_into_view;
use gpui::*;
use std::time::{Duration, Instant};

/// Pause after which typing starts a new prefix, as in native file managers.
pub(super) const RESET_DELAY: Duration = Duration::from_millis(1000);

#[derive(Default)]
pub(super) struct TypeSelect {
    pub(super) prefix: String,
    pub(super) last: Option<Instant>,
}

impl TypeSelect {
    /// Appends the typed text and returns the prefix to look for.
    pub(super) fn push(&mut self, text: &str, now: Instant) -> &str {
        if self.last.is_none_or(|last| now.duration_since(last) > RESET_DELAY) {
            self.prefix.clear();
        }
        self.last = Some(now);
        self.prefix.extend(text.chars().flat_map(char::to_lowercase));
        &self.prefix
    }
}

impl OxydeApp {
    /// Handles a printable key without Control or Alt. Returns whether it was used.
    pub(super) fn type_select(&mut self, text: &str, cx: &mut Context<Self>) -> bool {
        use crate::ui::date_groups::{display_rows, DisplayRow};
        if text.is_empty() || text.chars().any(char::is_control) { return false; }
        let pane_id = self.state.active_pane_id.clone();
        let Some(pane) = self.state.find_pane(&pane_id) else { return false; };
        let tab = pane.active_tab();
        // A leading space is not a name search.
        if self.type_select.prefix.is_empty() && text == " "
            && self.type_select.last.is_none_or(|last| last.elapsed() > RESET_DELAY) { return false; }
        let prefix = self.type_select.push(text, Instant::now()).to_owned();
        let rows = display_rows(tab.filtered_files(), tab.grouping(), &tab.collapsed_groups, 1);
        let order: Vec<usize> = rows.iter().filter_map(|row| match row {
            DisplayRow::Files(indices) => Some(indices.iter().copied()), _ => None,
        }).flatten().collect();
        let files = tab.filtered_files();
        let names: Vec<&str> = order.iter().map(|index| files[*index].name.as_str()).collect();
        let current = tab.keyboard_cursor.as_ref().filter(|path| tab.selected_paths.contains(*path))
            .and_then(|path| order.iter().position(|index| &files[*index].path == path));
        let Some(position) = find_match(&names, current, &prefix) else { return true; };
        let index = order[position];
        let path = files[index].path.clone();
        self.toggle_selection(&pane_id, path, false, false, cx);
        if let Some(pane) = self.state.find_pane(&pane_id) {
            let tab = pane.active_tab();
            let columns = if tab.view_mode == crate::ui::workspace::ViewMode::Grid {
                let width = self.list_bounds.get(&pane_id).map_or(self.list_width, |bounds| bounds.size.width);
                crate::ui::style::GridGeometry::for_viewport(width, crate::ui::components::scrollbar::track_width()).columns
            } else { 1 };
            let rows = display_rows(tab.filtered_files(), tab.grouping(), &tab.collapsed_groups, columns);
            scroll_row_into_view(tab, &rows, index);
        }
        true
    }
}

/// Finds the name to select for a lowercase `prefix`, in display order.
///
/// A repeated single letter ("aaa") cycles through names starting with that
/// letter. Otherwise the current item stays selected while it still matches,
/// and the search continues after it.
pub(super) fn find_match(names: &[&str], current: Option<usize>, prefix: &str) -> Option<usize> {
    let first = prefix.chars().next()?;
    let count = names.len();
    let starts = |i: usize, text: &str| names[i].to_lowercase().starts_with(text);
    // Searches in display order, from the current item (included or not).
    let search = |text: &str, include_current: bool| {
        let (start, skip) = match current {
            Some(current) => (current, usize::from(!include_current)),
            None => (0, 0),
        };
        (skip..count + skip).map(|offset| (start + offset) % count).find(|&i| starts(i, text))
    };
    if prefix.chars().count() == 1 { return search(prefix, false); }
    if let Some(found) = search(prefix, true) { return Some(found); }
    if prefix.chars().all(|c| c == first) { return search(&first.to_string(), false); }
    None
}

#[cfg(test)]
#[path = "type_select/tests/find_match.rs"]
mod tests;
