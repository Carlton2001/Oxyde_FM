//! Move a selected folder by renaming it whole, as Nautilus does, when its name is
//! free in the destination on the same volume.
use super::super::{FileOperation, FileOperationManager};
use crate::models::transaction::HistoryRenamedDirectory;
use crate::systems::source_selection::SelectedEntry;
use std::path::Path;
use std::sync::{Arc, Mutex};

impl FileOperationManager {
    /// The plan checked that one rename can reach the destination, on its mount.
    pub(super) fn move_whole_directory(
        op: &Arc<Mutex<FileOperation>>,
        selected: &SelectedEntry,
        destination: &Path,
    ) -> Result<(), String> {
        let source = selected.path();
        op.lock().unwrap().item_started(source);
        let expected = selected
            .directory_identity()
            .map_err(|error| format!("Cannot hold source {:?}: {error}", source))?;
        let renamed = crate::systems::item_rename::rename_captured_with_checkpoint(
            source,
            destination,
            Some(expected),
            &mut || Self::archive_checkpoint(op).map_err(std::io::Error::other),
        );
        let identity = match renamed {
            Ok(identity) => identity,
            // The name now holds another folder than the one selected.
            Err(error)
                if error
                    .get_ref()
                    .is_some_and(|inner| inner.is::<crate::models::error::HistoryRenameFailure>()) =>
            {
                return Err(crate::models::error::SourceSelectionFailure::changed(source).to_string());
            }
            Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => {
                return Err("error_access_denied".to_string());
            }
            Err(error) => return Err(format!("Cannot move {:?} to {:?}: {error}", source, destination)),
        };
        if identity.is_none() {
            log::warn!("Moved folder identity unavailable for {}", destination.display());
        }
        let mut journal = op.lock().unwrap();
        journal.renamed_dirs.push(HistoryRenamedDirectory {
            source: source.to_path_buf(),
            path: destination.to_path_buf(),
            identity,
        });
        journal.item_complete(source);
        journal.current_file = Some(source.to_string_lossy().into_owned());
        Ok(())
    }
}
