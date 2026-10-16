//! Index of canonical source paths, preserving Path component overlap semantics.
use std::collections::BTreeSet;
use std::path::PathBuf;

#[derive(Default)]
pub(super) struct SourcePathSet {
    paths: BTreeSet<PathBuf>,
}

impl SourcePathSet {
    /// Reject duplicates and either direction of ancestor/descendant overlap.
    /// Rejected insertions leave the index unchanged.
    pub fn insert(&mut self, path: PathBuf) -> bool {
        if path
            .ancestors()
            .any(|ancestor| self.paths.contains(ancestor))
        {
            return false;
        }
        // Path ordering compares components: descendants of a path form one
        // contiguous range immediately after that path. Inspect its first member.
        if self
            .paths
            .range(path.clone()..)
            .next()
            .is_some_and(|existing| existing.starts_with(&path))
        {
            return false;
        }
        self.paths.insert(path)
    }
}

#[cfg(test)]
#[path = "source_path_set/tests/overlap.rs"]
mod tests;
