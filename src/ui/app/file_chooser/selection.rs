//! The file chooser's selection, positions in the entries shown, under the main
//! window's rules (`selection.rs`, `toggle_selection`): a press selects one entry,
//! Ctrl toggles one, Shift extends from the anchor, and the arrows move a cursor. It
//! follows the entries by path when they are arranged again.
use oxyde_lib::models::FileEntry;
use std::collections::{BTreeSet, HashMap};

#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct Selection {
    /// The entry last pressed or reached, where the arrows start.
    cursor: Option<usize>,
    /// Where Shift extends from.
    anchor: Option<usize>,
    marked: BTreeSet<usize>,
}

/// A selection by path, to find again in entries arranged anew.
#[derive(Clone, Debug, Default)]
pub(super) struct Kept {
    cursor: Option<String>,
    anchor: Option<String>,
    marked: Vec<String>,
}

impl Kept {
    /// One entry, such as the folder just left.
    pub(super) fn path(path: String) -> Self {
        Self { cursor: Some(path.clone()), anchor: Some(path.clone()), marked: vec![path] }
    }
}

impl Selection {
    pub(super) fn cursor(&self) -> Option<usize> {
        self.cursor
    }

    pub(super) fn contains(&self, position: usize) -> bool {
        self.marked.contains(&position)
    }

    pub(super) fn positions(&self) -> impl Iterator<Item = usize> + '_ {
        self.marked.iter().copied()
    }

    /// The entry selected, when it is the only one.
    pub(super) fn single(&self) -> Option<usize> {
        let mut marked = self.marked.iter().copied();
        marked.next().filter(|_| marked.next().is_none())
    }

    pub(super) fn is_empty(&self) -> bool {
        self.marked.is_empty()
    }

    /// Whether anything changed.
    pub(super) fn clear(&mut self) -> bool {
        let changed = *self != Self::default();
        *self = Self::default();
        changed
    }

    /// A plain press or arrow: this entry alone.
    pub(super) fn only(&mut self, position: usize) {
        self.cursor = Some(position);
        self.anchor = Some(position);
        self.marked = BTreeSet::from([position]);
    }

    /// Ctrl and a press: the entry joins or leaves the selection.
    pub(super) fn toggle(&mut self, position: usize) {
        self.cursor = Some(position);
        if !self.marked.remove(&position) {
            self.marked.insert(position);
            self.anchor = Some(position);
        }
    }

    /// Shift: the entries from the anchor to this one, added to the selection with
    /// Ctrl, else in its place. Without an anchor, the entry alone.
    pub(super) fn extend(&mut self, position: usize, add: bool) {
        let Some(anchor) = self.anchor else {
            self.only(position);
            return;
        };
        if !add {
            self.marked.clear();
        }
        self.marked.extend(anchor.min(position)..=anchor.max(position));
        self.cursor = Some(position);
    }

    /// Ctrl+A: every entry shown, the cursor kept.
    pub(super) fn all(&mut self, count: usize) {
        self.marked = (0..count).collect();
        if count == 0 {
            self.cursor = None;
            self.anchor = None;
        } else if self.anchor.is_none() {
            self.anchor = Some(0);
        }
    }

    pub(super) fn keep(&self, shown: &[FileEntry]) -> Kept {
        let path = |position: Option<usize>| position.and_then(|position| shown.get(position)).map(|entry| entry.path.clone());
        Kept {
            cursor: path(self.cursor),
            anchor: path(self.anchor),
            marked: self.marked.iter().filter_map(|position| shown.get(*position)).map(|entry| entry.path.clone()).collect(),
        }
    }

    /// The kept entries still shown.
    pub(super) fn restore(kept: &Kept, shown: &[FileEntry]) -> Self {
        let positions: HashMap<&str, usize> = shown.iter().enumerate().map(|(position, entry)| (entry.path.as_str(), position)).collect();
        let find = |path: &Option<String>| path.as_deref().and_then(|path| positions.get(path).copied());
        let marked: BTreeSet<usize> = kept.marked.iter().filter_map(|path| positions.get(path.as_str()).copied()).collect();
        let cursor = find(&kept.cursor).or_else(|| marked.first().copied());
        let anchor = find(&kept.anchor).or(cursor);
        if marked.is_empty() {
            return Self::default();
        }
        Self { cursor, anchor, marked }
    }
}

#[cfg(test)]
#[path = "tests/selection.rs"]
mod tests;
