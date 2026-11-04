//! Read-only overview of a folder or an archive in the Viewer. Folders open by
//! their left chevron only, as in the sidebar tree; the right one only says that
//! a folder holds subfolders, as in the pane. A double-click hands the item to the
//! source pane. Local
//! folders are read level by level in the background; an archive is indexed once.
mod loading;
mod render;
pub(super) use loading::has_tree;
pub(super) use render::LIST_PADDING;

use crate::ui::workspace::{SortColumn, SortDirection};
use gpui::{ListAlignment, ListOffset, ListState, Pixels};
use oxyde_lib::models::FileEntry;
use std::{
    collections::{HashMap, HashSet},
    path::Path,
    sync::{atomic::AtomicBool, Arc},
};

/// Entries shown per folder; a last row counts the others.
pub(super) const SHOWN_PER_FOLDER: usize = 1_000;
/// Entries read from a local folder to sort it; past this, reading order is kept.
pub(super) const READ_PER_FOLDER: usize = 10_000;
/// Rows of the whole tree; once reached, closed folders no longer open.
pub(super) const MAX_ROWS: usize = 20_000;

/// The source pane's listing choices, applied when a level is read.
#[derive(Clone, Copy, Default, PartialEq)]
pub(super) struct Listing {
    pub(super) show_hidden: bool,
    pub(super) show_system: bool,
    pub(super) sort: (SortColumn, SortDirection),
}

/// One level, sorted when complete. Every entry read is kept so that a new sort
/// of the pane can reorder it; the first `SHOWN_PER_FOLDER` are shown.
pub(super) struct Folder {
    pub(super) entries: Vec<Arc<FileEntry>>,
    /// Visible entries read, shown or not.
    pub(super) total: usize,
    pub(super) folders: usize,
    pub(super) files: usize,
    /// Sum of the sizes of the files read at this level.
    pub(super) file_bytes: u64,
    /// The level was read to its end.
    pub(super) complete: bool,
}

impl Folder {
    /// A complete level follows the pane's order; a truncated one keeps reading
    /// order, since sorting part of a folder would misrepresent it.
    pub(super) fn new(mut entries: Vec<FileEntry>, complete: bool, sort: (SortColumn, SortDirection)) -> Self {
        let folders = entries.iter().filter(|entry| entry.is_dir).count();
        let file_bytes = entries.iter().filter(|entry| !entry.is_dir).map(|entry| entry.size).sum();
        let total = entries.len();
        if complete {
            crate::ui::workspace::sort_files(&mut entries, sort.0, sort.1);
        }
        Self {
            entries: entries.into_iter().map(Arc::new).collect(),
            total,
            folders,
            files: total - folders,
            file_bytes,
            complete,
        }
    }

    pub(super) fn shown(&self) -> &[Arc<FileEntry>] {
        &self.entries[..self.entries.len().min(SHOWN_PER_FOLDER)]
    }

    pub(super) fn hidden(&self) -> usize {
        self.total - self.shown().len()
    }

    /// The same level in another order; a truncated level keeps reading order.
    pub(super) fn resorted(self, sort: (SortColumn, SortDirection)) -> Self {
        if !self.complete {
            return self;
        }
        let entries = self.entries.into_iter().map(Arc::unwrap_or_clone).collect();
        Self::new(entries, true, sort)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Failure {
    Unreadable,
    /// An archive whose entry list needs its password.
    Locked,
}

pub(super) enum Level {
    Loading,
    Ready(Folder),
    Failed(Failure),
}

/// Whole-archive facts shown above the tree.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct ArchiveInfo {
    pub(super) extracted: u64,
    /// Size of the archive file; unknown for split archives.
    pub(super) packed: Option<u64>,
    pub(super) password: bool,
}

impl ArchiveInfo {
    /// Share of the extracted size the archive takes, when both are known.
    pub(super) fn percent(&self) -> Option<u64> {
        let packed = self.packed?;
        (self.extracted > 0).then(|| (packed.saturating_mul(100) + self.extracted / 2) / self.extracted)
    }
}

/// What a folder holds, known before it is opened.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct Contents {
    /// Any visible entry: the folder can open.
    pub(super) entries: bool,
    pub(super) subfolders: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Chevron {
    None,
    Collapsed,
    Expanded,
    /// Closed while the tree holds `MAX_ROWS` rows.
    Disabled,
}

#[derive(Clone)]
pub(super) enum Row {
    Entry { file: Arc<FileEntry>, depth: usize, chevron: Chevron, subfolders: bool },
    Loading { depth: usize },
    Failed { depth: usize },
    Empty { depth: usize },
    /// Entries of `folder` beyond the shown ones; `exact` is false past `READ_PER_FOLDER`.
    More { folder: String, count: usize, exact: bool, depth: usize },
}

/// What a chevron press needs from the caller.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum Toggle {
    Changed,
    /// The folder opened and its level must be read.
    Load,
    Refused,
}

pub(super) struct Tree {
    pub(super) root: String,
    pub(super) listing: Listing,
    pub(super) levels: HashMap<String, Level>,
    pub(super) expanded: HashSet<String>,
    pub(super) contents: HashMap<String, Contents>,
    pub(super) archive: Option<ArchiveInfo>,
    pub(super) rows: Arc<Vec<Row>>,
    pub(super) list: ListState,
    /// Set when the tree is dropped; background reads stop at their next entry.
    pub(super) cancelled: Arc<AtomicBool>,
}

impl Drop for Tree {
    fn drop(&mut self) {
        self.cancelled.store(true, std::sync::atomic::Ordering::Relaxed);
    }
}

impl Tree {
    pub(super) fn new(root: String, listing: Listing) -> Self {
        let mut levels = HashMap::new();
        levels.insert(root.clone(), Level::Loading);
        Self {
            root,
            listing,
            levels,
            expanded: HashSet::new(),
            contents: HashMap::new(),
            archive: None,
            rows: Arc::new(Vec::new()),
            list: ListState::new(0, ListAlignment::Top, gpui::px(800.0)),
            cancelled: Arc::new(AtomicBool::new(false)),
        }
    }

    pub(super) fn root_level(&self) -> Option<&Level> {
        self.levels.get(&self.root)
    }

    pub(super) fn toggle(&mut self, path: &str) -> Toggle {
        if self.expanded.remove(path) {
            return Toggle::Changed;
        }
        if !self.contents.get(path).is_some_and(|contents| contents.entries) || self.rows.len() >= MAX_ROWS {
            return Toggle::Refused;
        }
        self.expanded.insert(path.to_owned());
        if self.levels.contains_key(path) {
            return Toggle::Changed;
        }
        self.levels.insert(path.to_owned(), Level::Loading);
        Toggle::Load
    }

    /// Rows of the open levels, depth first, in each level's order.
    pub(super) fn flatten(&self) -> Vec<Row> {
        let mut rows = Vec::new();
        match self.root_level() {
            Some(Level::Ready(folder)) if folder.total == 0 => rows.push(Row::Empty { depth: 0 }),
            Some(Level::Ready(folder)) => self.push_level(folder, &self.root, 0, &mut rows),
            _ => {}
        }
        if rows.len() >= MAX_ROWS {
            for row in &mut rows {
                if let Row::Entry { chevron: chevron @ Chevron::Collapsed, .. } = row {
                    *chevron = Chevron::Disabled;
                }
            }
        }
        rows
    }

    fn push_level(&self, folder: &Folder, path: &str, depth: usize, rows: &mut Vec<Row>) {
        for file in folder.shown() {
            let open = self.expanded.contains(&file.path);
            let contents = self.contents.get(&file.path).copied().filter(|_| file.is_dir).unwrap_or_default();
            let chevron = match (contents.entries, open) {
                (false, _) => Chevron::None,
                (true, false) => Chevron::Collapsed,
                (true, true) => Chevron::Expanded,
            };
            rows.push(Row::Entry { file: file.clone(), depth, chevron, subfolders: contents.subfolders });
            if chevron != Chevron::Expanded {
                continue;
            }
            match self.levels.get(&file.path) {
                Some(Level::Ready(child)) if child.total == 0 => rows.push(Row::Empty { depth: depth + 1 }),
                Some(Level::Ready(child)) => self.push_level(child, &file.path, depth + 1, rows),
                Some(Level::Failed(_)) => rows.push(Row::Failed { depth: depth + 1 }),
                Some(Level::Loading) | None => rows.push(Row::Loading { depth: depth + 1 }),
            }
        }
        if folder.hidden() > 0 {
            rows.push(Row::More { folder: path.to_owned(), count: folder.hidden(), exact: folder.complete, depth });
        }
    }

    /// Rebuilds the rows and keeps the first visible row in place: rows change
    /// only below the pressed chevron, which is on screen.
    pub(super) fn refresh(&mut self, row_height: Pixels) {
        let rows = self.flatten();
        let top = self.list.logical_scroll_top();
        self.list.reset_with_uniform_height(rows.len(), row_height);
        if !rows.is_empty() {
            let item_ix = top.item_ix.min(rows.len() - 1);
            self.list.scroll_to(ListOffset { item_ix, offset_in_item: top.offset_in_item });
        }
        self.rows = Arc::new(rows);
    }
}

/// Groups an archive index into its levels, with the folders that only appear
/// in member paths. Paths are built like the pane's virtual paths.
pub(super) fn archive_levels(archive: &Path, entries: Vec<FileEntry>) -> HashMap<String, Vec<FileEntry>> {
    let origin = Arc::new(archive.to_path_buf());
    let mut levels: HashMap<String, HashMap<String, FileEntry>> = HashMap::new();
    levels.entry(archive.to_string_lossy().into_owned()).or_default();
    for entry in entries {
        let Ok(relative) = Path::new(&entry.path).strip_prefix(archive) else { continue };
        let parts: Vec<String> = relative
            .components()
            .map(|part| part.as_os_str().to_string_lossy().into_owned())
            .collect();
        let mut parent = archive.to_path_buf();
        for (index, name) in parts.iter().enumerate() {
            let path = parent.join(name);
            let key = path.to_string_lossy().into_owned();
            let last = index + 1 == parts.len();
            let level = levels.entry(parent.to_string_lossy().into_owned()).or_default();
            if last {
                let directory = entry.is_dir || level.get(name).is_some_and(|known| known.is_dir);
                let mut member = FileEntry { name: name.clone(), path: key.clone(), ..entry.clone() };
                member.archive_path = Some(origin.clone());
                member.is_dir = directory;
                if directory {
                    member.size = 0;
                }
                level.insert(name.clone(), member);
            } else {
                let known = level.entry(name.clone()).or_insert_with(|| FileEntry {
                    archive_path: Some(origin.clone()),
                    name: name.clone(),
                    path: key.clone(),
                    is_dir: true,
                    ..FileEntry::default()
                });
                known.is_dir = true;
                known.size = 0;
            }
            if last && !entry.is_dir {
                break;
            }
            levels.entry(key).or_default();
            parent = path;
        }
    }
    // A member stored both as a file and as a folder prefix is a folder.
    levels
        .into_iter()
        .map(|(path, members)| (path, members.into_values().collect()))
        .collect()
}

#[cfg(test)]
#[path = "tree/tests/model.rs"]
mod tests;
