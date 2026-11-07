//! The file chooser's entries as the main window arranges a folder: its name filter,
//! its sort (`workspace::sort_files`, folders first), its type-to-select, and the
//! choices kept between windows: view, sort, the sidebar's width and the folder the
//! last one ended in.
use crate::ui::workspace::{SortColumn, SortDirection};
use oxyde_lib::models::FileEntry;
use std::collections::HashSet;

/// Kept in the preferences as `file_chooser_options`.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub(super) struct Options {
    pub(super) grid: bool,
    pub(super) sort: SortColumn,
    pub(super) direction: SortDirection,
    pub(super) folder: Option<String>,
    /// The sidebar's width at the baseline scale, once resized here.
    pub(super) sidebar: Option<f32>,
}

impl Options {
    pub(super) fn from_value(value: Option<&serde_json::Value>) -> Self {
        value.and_then(|value| serde_json::from_value(value.clone()).ok()).unwrap_or_default()
    }

    pub(super) fn to_value(&self) -> Option<serde_json::Value> {
        serde_json::to_value(self).ok()
    }
}

/// The main sidebar's bounds, at the baseline scale.
pub(super) const SIDEBAR_MIN: f32 = 150.0;
const SIDEBAR_MAX: f32 = 600.0;
/// What the pane keeps beside the sidebar: the name's minimum and the metadata columns.
pub(super) const PANE_MIN: f32 = 700.0;

/// The sidebar's width for a drag to `requested`, as the main window bounds it, while
/// the pane keeps its minimum in a window `window` wide; both at the baseline scale.
/// Without the main window's rail, a narrower drag stops at the minimum.
pub(super) fn sidebar_width(requested: f32, window: f32) -> f32 {
    requested.min(window - PANE_MIN).clamp(SIDEBAR_MIN, SIDEBAR_MAX)
}

/// What the list shows, and what its metadata columns are measured from.
#[derive(Default)]
pub(super) struct Arranged {
    pub(super) shown: Vec<FileEntry>,
    pub(super) folders: usize,
    pub(super) types: HashSet<String>,
    pub(super) sizes: HashSet<(u64, bool, bool)>,
}

/// The entries whose name holds `filter`, without regard to case, in the main
/// window's order for `sort`.
pub(super) fn arrange(entries: &[FileEntry], filter: &str, sort: SortColumn, direction: SortDirection) -> Arranged {
    let filter = filter.to_lowercase();
    let mut shown: Vec<FileEntry> = entries.iter()
        .filter(|entry| filter.is_empty() || entry.name.to_lowercase().contains(&filter))
        .cloned()
        .collect();
    crate::ui::workspace::sort_files(&mut shown, sort, direction);
    let mut arranged = Arranged { folders: shown.iter().filter(|entry| entry.is_dir).count(), ..Default::default() };
    for entry in entries {
        arranged.types.insert(crate::ui::components::file_list::file_type_label(entry));
        arranged.sizes.insert((entry.size, entry.is_dir, entry.is_calculated));
    }
    arranged.shown = shown;
    arranged
}

/// The main window's type-to-select: the name to select for a lowercase `prefix`, in
/// display order. A repeated single letter cycles through the names starting with it;
/// otherwise the current item stays selected while it still matches.
pub(super) fn find_match(names: &[&str], current: Option<usize>, prefix: &str) -> Option<usize> {
    let first = prefix.chars().next()?;
    let count = names.len();
    let starts = |i: usize, text: &str| names[i].to_lowercase().starts_with(text);
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

/// The main window's arrow keys over rows of positions: no selection starts at the
/// first entry, and a short last grid row keeps the nearest column.
pub(super) fn arrow_target(rows: &[&[usize]], current: Option<usize>, key: &str) -> Option<usize> {
    let Some((row, column)) = current.and_then(|index| rows.iter().enumerate()
        .find_map(|(row, indices)| indices.iter().position(|i| *i == index).map(|column| (row, column)))) else {
        return rows.first().and_then(|row| row.first()).copied();
    };
    let target_row = match key {
        "up" => row.saturating_sub(1),
        "down" => (row + 1).min(rows.len() - 1),
        _ => row,
    };
    let target_column = match key {
        "left" => column.saturating_sub(1),
        "right" => column + 1,
        _ => column,
    }.min(rows[target_row].len() - 1);
    Some(rows[target_row][target_column])
}

#[cfg(test)]
#[path = "tests/arrange.rs"]
mod tests;
