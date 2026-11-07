//! The order and filter of the file chooser's list, and the choices it remembers
//! between windows: view, sort and the folder the last one ended in.
use oxyde_lib::models::FileEntry;
use std::cmp::Ordering;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub(super) enum SortKey {
    #[default]
    Name,
    Type,
    Size,
    Modified,
}

/// Kept in the preferences as `file_chooser_options`.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub(super) struct Options {
    pub(super) grid: bool,
    pub(super) sort: SortKey,
    pub(super) descending: bool,
    pub(super) folder: Option<String>,
}

impl Options {
    pub(super) fn from_value(value: Option<&serde_json::Value>) -> Self {
        value.and_then(|value| serde_json::from_value(value.clone()).ok()).unwrap_or_default()
    }

    pub(super) fn to_value(&self) -> Option<serde_json::Value> {
        serde_json::to_value(self).ok()
    }
}

fn extension(entry: &FileEntry) -> String {
    if entry.is_dir {
        return String::new();
    }
    entry.name.rsplit_once('.').filter(|(stem, _)| !stem.is_empty())
        .map(|(_, extension)| extension.to_lowercase()).unwrap_or_default()
}

fn compare(a: &FileEntry, b: &FileEntry, key: SortKey) -> Ordering {
    let name = || oxyde_lib::utils::compare_natural(&a.name, &b.name);
    match key {
        SortKey::Name => name(),
        SortKey::Type => extension(a).cmp(&extension(b)).then_with(name),
        // Folder sizes are not counted here: folders keep their names' order.
        SortKey::Size if a.is_dir => name(),
        SortKey::Size => a.size.cmp(&b.size).then_with(name),
        SortKey::Modified => a.modified.cmp(&b.modified).then_with(name),
    }
}

/// Whether `name` holds `filter`, without regard to case.
pub(super) fn matches(name: &str, filter: &str) -> bool {
    filter.is_empty() || name.to_lowercase().contains(filter)
}

/// Positions in `entries` the list shows, folders first, then in `key` order.
pub(super) fn arrange(entries: &[FileEntry], filter: &str, key: SortKey, descending: bool) -> Vec<usize> {
    let filter = filter.trim().to_lowercase();
    let mut shown: Vec<usize> = (0..entries.len()).filter(|&index| matches(&entries[index].name, &filter)).collect();
    shown.sort_by(|&a, &b| {
        let (a, b) = (&entries[a], &entries[b]);
        let order = compare(a, b, key);
        b.is_dir.cmp(&a.is_dir).then(if descending { order.reverse() } else { order })
    });
    shown
}

/// The first of `shown` whose name starts with `prefix`, as typing in the list finds it.
pub(super) fn find_prefix(entries: &[FileEntry], shown: &[usize], prefix: &str) -> Option<usize> {
    let prefix = prefix.to_lowercase();
    shown.iter().position(|&index| entries[index].name.to_lowercase().starts_with(&prefix))
}

#[cfg(test)]
#[path = "tests/arrange.rs"]
mod tests;
