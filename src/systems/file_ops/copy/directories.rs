//! Give the directories a copy created the attributes of their sources.
use super::super::{FileOperation, FileOperationManager, FILE_OPS_LOG_TARGET};
use crate::platform::DirectoryAttributes;
use crate::systems::destination_tree::DestinationTree;
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Mutex},
};

impl FileOperationManager {
    /// Runs once every child is in place: a later creation would change the dates
    /// again. Existing directories merged into are never touched. A destination
    /// that cannot keep an attribute does not fail the copy.
    pub(super) fn restore_created_directories(
        op: &Arc<Mutex<FileOperation>>,
        tree: &DestinationTree,
        sources: &HashMap<PathBuf, DirectoryAttributes>,
    ) {
        let created = op.lock().unwrap().created_dirs.clone();
        let mut created: Vec<_> = created
            .into_iter()
            .filter_map(|path| sources.get(&path).map(|source| (path, source)))
            .collect();
        created.sort_by_key(|(path, _)| std::cmp::Reverse(path.components().count()));
        // Undo and Redo give them again to the folders they recreate.
        op.lock().unwrap().directory_attributes.extend(
            created.iter().map(|(path, source)| (path.clone(), (*source).clone())),
        );
        for (path, source) in created {
            if let Err(error) = tree.restore_directory(&path, source) {
                log::warn!(
                    target: FILE_OPS_LOG_TARGET,
                    "Directory attributes not kept for {}: {error}",
                    path.display()
                );
            }
        }
    }
}
