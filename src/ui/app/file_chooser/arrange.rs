//! The file chooser's entries as the main window arranges a folder: its name filter,
//! its sort (`workspace::sort_files`, folders first), its type-to-select, and the
//! choices kept between windows: view, sort, the sidebar's width and the folder the
//! last one ended in. The request decides beside them which files appear: none when
//! choosing folders, else those of the type filter shown, as GTK's filters match them.
use crate::ui::workspace::{SortColumn, SortDirection};
use oxyde_lib::models::FileEntry;
use oxyde_lib::platform::{FileTypeFilter, FileTypePattern};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

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

/// Which files the request lets the list show; folders always appear, to browse.
#[derive(Clone, Default)]
pub(super) struct Admission {
    pub(super) folders_only: bool,
    pub(super) types: Option<FileTypeFilter>,
    /// The files' MIME types by path, read when a filter names MIME types.
    pub(super) content_types: Arc<HashMap<String, &'static str>>,
}

/// Whether a filter needs the files' MIME types.
pub(super) fn names_mime_types(filter: &FileTypeFilter) -> bool {
    filter.patterns.iter().any(|pattern| matches!(pattern, FileTypePattern::MimeType(_)))
}

/// A type filter ready to test names: GTK's shell patterns keep case, and a filter
/// without rules lets no file through.
struct Types<'a> {
    globs: Vec<glob::Pattern>,
    mime_types: Vec<&'a str>,
}

impl<'a> Types<'a> {
    fn of(filter: &'a FileTypeFilter) -> Self {
        let mut types = Types { globs: Vec::new(), mime_types: Vec::new() };
        for pattern in &filter.patterns {
            match pattern {
                FileTypePattern::Glob(glob) => types.globs.extend(glob::Pattern::new(glob).ok()),
                FileTypePattern::MimeType(mime) => types.mime_types.push(mime),
            }
        }
        types
    }

    fn admit(&self, entry: &FileEntry, admission: &Admission, mime_matches: &dyn Fn(&str, &str) -> bool) -> bool {
        self.globs.iter().any(|glob| glob.matches(&entry.name))
            || admission.content_types.get(&entry.path).is_some_and(|mime| self.mime_types.iter().any(|pattern| mime_matches(mime, pattern)))
    }
}

/// What the list shows, and what its metadata columns are measured from.
#[derive(Default)]
pub(super) struct Arranged {
    pub(super) shown: Vec<FileEntry>,
    pub(super) folders: usize,
    pub(super) types: HashSet<String>,
    pub(super) sizes: HashSet<(u64, bool, bool)>,
}

/// The entries the request admits whose name holds `filter`, without regard to case,
/// in the main window's order for `sort`. `mime_matches` tells whether a MIME type
/// answers a filter's, as the system's database relates them.
pub(super) fn arrange(
    entries: &[FileEntry],
    filter: &str,
    admission: &Admission,
    mime_matches: &dyn Fn(&str, &str) -> bool,
    sort: SortColumn,
    direction: SortDirection,
) -> Arranged {
    let filter = filter.to_lowercase();
    let types = admission.types.as_ref().map(Types::of);
    let admitted = |entry: &FileEntry| entry.is_dir || (!admission.folders_only
        && types.as_ref().is_none_or(|types| types.admit(entry, admission, mime_matches)));
    let mut shown: Vec<FileEntry> = entries.iter()
        .filter(|entry| admitted(entry))
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

#[cfg(test)]
#[path = "tests/arrange.rs"]
mod tests;
