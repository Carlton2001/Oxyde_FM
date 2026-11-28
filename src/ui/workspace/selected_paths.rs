use std::collections::HashSet;
use std::sync::atomic::{AtomicU64, Ordering};

/// Stamps are unique across tabs, so a set put in place of another never reuses one.
static NEXT_STAMP: AtomicU64 = AtomicU64::new(1);

fn next_stamp() -> u64 {
    NEXT_STAMP.fetch_add(1, Ordering::Relaxed)
}

/// The selected paths of a tab. Each mutable access takes a new stamp, so what is
/// drawn from the selection is checked by comparing stamps instead of two sets of
/// paths at each frame (Ctrl+A on 100 000 files).
#[derive(Clone, Debug)]
pub struct SelectedPaths {
    paths: HashSet<String>,
    stamp: u64,
}

impl SelectedPaths {
    /// Equal stamps mean the same paths; a mutable access, even one that changes
    /// nothing, takes a new stamp.
    pub(crate) fn stamp(&self) -> u64 {
        self.stamp
    }
}

impl Default for SelectedPaths {
    fn default() -> Self {
        HashSet::new().into()
    }
}

impl From<HashSet<String>> for SelectedPaths {
    fn from(paths: HashSet<String>) -> Self {
        Self { paths, stamp: next_stamp() }
    }
}

#[cfg(test)]
impl<const N: usize> From<[String; N]> for SelectedPaths {
    fn from(paths: [String; N]) -> Self {
        HashSet::from(paths).into()
    }
}

#[cfg(test)]
impl PartialEq<HashSet<String>> for SelectedPaths {
    fn eq(&self, other: &HashSet<String>) -> bool {
        self.paths == *other
    }
}

impl FromIterator<String> for SelectedPaths {
    fn from_iter<I: IntoIterator<Item = String>>(iter: I) -> Self {
        iter.into_iter().collect::<HashSet<_>>().into()
    }
}

impl std::ops::Deref for SelectedPaths {
    type Target = HashSet<String>;

    fn deref(&self) -> &Self::Target {
        &self.paths
    }
}

impl std::ops::DerefMut for SelectedPaths {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.stamp = next_stamp();
        &mut self.paths
    }
}

impl<'a> IntoIterator for &'a SelectedPaths {
    type Item = &'a String;
    type IntoIter = std::collections::hash_set::Iter<'a, String>;

    fn into_iter(self) -> Self::IntoIter {
        self.paths.iter()
    }
}

#[cfg(test)]
#[path = "selected_paths/tests/stamps.rs"]
mod tests;
