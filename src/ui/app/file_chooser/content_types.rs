//! The files' MIME types for filters that name them. Reading a type may open the file,
//! so a large folder or a network share takes a while: the listing shows first, types
//! come by batches that a navigation stops, and the types read stay known for Back and
//! Refresh while the file keeps its date and size.
use oxyde_lib::models::FileEntry;
use std::collections::HashMap;

/// Files typed before the list is arranged again.
pub(super) const BATCH: usize = 256;
/// Types kept across folders; past this, the cache starts over.
const KEPT: usize = 100_000;

/// A file to type: its path, as listed, and the stamp its type holds for.
pub(super) type Pending = (String, (u64, u64));

#[derive(Default)]
pub(super) struct TypeCache {
    types: HashMap<String, ((u64, u64), &'static str)>,
}

fn stamp(entry: &FileEntry) -> (u64, u64) {
    (entry.modified, entry.size)
}

impl TypeCache {
    /// The listed files' known types, and the files still to type.
    pub(super) fn split(&self, entries: &[FileEntry]) -> (HashMap<String, &'static str>, Vec<Pending>) {
        let mut known = HashMap::new();
        let mut pending = Vec::new();
        for entry in entries.iter().filter(|entry| !entry.is_dir) {
            match self.types.get(&entry.path) {
                Some((held, mime)) if *held == stamp(entry) => {
                    known.insert(entry.path.clone(), *mime);
                }
                _ => pending.push((entry.path.clone(), stamp(entry))),
            }
        }
        (known, pending)
    }

    pub(super) fn remember(&mut self, typed: &[(Pending, &'static str)]) {
        if self.types.len() + typed.len() > KEPT {
            self.types.clear();
        }
        for ((path, stamp), mime) in typed {
            self.types.insert(path.clone(), (*stamp, mime));
        }
    }
}

/// Reads the types of `files`; blocking.
pub(super) fn read(files: Vec<Pending>) -> Vec<(Pending, &'static str)> {
    let platform = oxyde_lib::platform::platform();
    files.into_iter().filter_map(|file| {
        let mime = platform.content_type(&oxyde_lib::utils::path_text::decode(&file.0), false)?;
        Some((file, mime))
    }).collect()
}

#[cfg(test)]
#[path = "tests/content_types.rs"]
mod tests;
