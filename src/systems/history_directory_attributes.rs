//! Give the folders Undo or Redo recreates the mode and dates the transfer gave them.
use super::history_directories::Directories;
use crate::models::transaction::TransactionDetails;
use crate::platform::platform;
use std::{
    collections::HashMap,
    fs::Metadata,
    path::{Path, PathBuf},
};

#[derive(Default)]
pub(crate) struct DirectoryAttributes(HashMap<PathBuf, Metadata>);

impl DirectoryAttributes {
    /// The folders the transfer created, by their path: a Redo recreates them.
    pub fn created(details: &TransactionDetails) -> Self {
        Self(details.directory_attributes.iter().flatten().cloned().collect())
    }

    /// A move undone recreates its source folders with what their destinations got.
    pub fn sources(details: &TransactionDetails) -> Self {
        let created = Self::created(details).0;
        Self(
            details
                .moved_dirs
                .iter()
                .flatten()
                .filter_map(|(source, destination)| {
                    let metadata = created.get(Path::new(destination))?;
                    Some((PathBuf::from(source), metadata.clone()))
                })
                .collect(),
        )
    }

    /// Runs once the recreated folders are complete, deepest first. A refused
    /// attribute leaves the folder as it is and does not fail the history action.
    pub fn apply<'a>(&self, directories: &mut Directories, recreated: impl IntoIterator<Item = &'a PathBuf>) {
        let mut recreated: Vec<_> = recreated
            .into_iter()
            .filter_map(|path| self.0.get(path).map(|source| (path, source)))
            .collect();
        recreated.sort_by_key(|(path, _)| std::cmp::Reverse(path.components().count()));
        for (path, source) in recreated {
            let restored = directories
                .directory(path)
                .and_then(|directory| platform().restore_directory_attributes(directory, source));
            if let Err(error) = restored {
                log::warn!("Directory attributes not kept for {}: {error}", path.display());
            }
        }
    }
}
