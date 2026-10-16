//! Per-preparation totals, never retained across queue admission/revalidation.
use crate::models::FileEntry;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

#[derive(Clone, Copy, Default)]
pub(super) struct Totals {
    bytes: u64,
    directories: u64,
    files: u64,
}
impl Totals {
    fn add(&mut self, other: Self) {
        self.bytes += other.bytes;
        self.directories += other.directories;
        self.files += other.files;
    }
    pub(super) fn apply(self, entry: &mut FileEntry) {
        entry.size = self.bytes;
        entry.folders_count = Some(self.directories.saturating_sub(1));
        entry.files_count = Some(self.files);
    }
}

#[derive(Default)]
pub(super) struct TreeSummaries {
    totals: HashMap<PathBuf, Totals>,
    #[cfg(test)]
    visits: usize,
}
impl TreeSummaries {
    pub(super) fn get(&mut self, root: &Path) -> Totals {
        if let Some(totals) = self.totals.get(root) {
            return *totals;
        }
        // Prune already summarized child roots, including overlapping selections
        // encountered from deepest to shallowest. Fold fresh directories in reverse.
        let mut collected: HashMap<PathBuf, Totals> = HashMap::new();
        let mut directories = Vec::new();
        let mut walker = walkdir::WalkDir::new(root).into_iter();
        while let Some(result) = walker.next() {
            let Ok(entry) = result else {
                continue;
            };
            #[cfg(test)]
            {
                self.visits += 1;
            }
            let path = entry.path();
            if entry.file_type().is_dir() {
                if let Some(cached) = self.totals.get(path).copied() {
                    if path != root {
                        if let Some(parent) = path.parent() {
                            collected
                                .entry(parent.to_path_buf())
                                .or_default()
                                .add(cached);
                        }
                    }
                    walker.skip_current_dir();
                } else {
                    collected.entry(path.to_path_buf()).or_default().directories += 1;
                    directories.push(path.to_path_buf());
                }
            } else {
                let own = Totals {
                    files: 1,
                    bytes: entry.metadata().map(|m| m.len()).unwrap_or(0),
                    directories: 0,
                };
                let owner = if path == root {
                    path
                } else {
                    path.parent().unwrap()
                };
                collected.entry(owner.to_path_buf()).or_default().add(own);
            }
        }
        for path in directories.into_iter().rev() {
            let total = collected[&path];
            if path != root {
                if let Some(parent) = path.parent() {
                    collected
                        .entry(parent.to_path_buf())
                        .or_default()
                        .add(total);
                }
            }
        }
        self.totals.extend(collected);
        *self.totals.entry(root.to_path_buf()).or_default()
    }
}

#[cfg(test)]
#[path = "tests/summaries.rs"]
mod tests;
