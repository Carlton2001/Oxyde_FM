//! Copy or move symbolic links as links: never followed, kept with the same target.
use super::super::{FileOperation, FileOperationManager, FILE_OPS_LOG_TARGET};
use crate::models::transaction::{HistoryLink, HistoryLinkIdentity};
use crate::platform::platform;
use crate::systems::{destination_tree::DestinationTree, source_selection::SelectedLink};
use std::path::Path;
use std::sync::{Arc, Mutex};

impl FileOperationManager {
    pub(super) fn transfer_link(
        op: &Arc<Mutex<FileOperation>>,
        tree: &DestinationTree,
        link: &SelectedLink,
        destination: &Path,
        replace_existing: bool,
        is_move: bool,
    ) -> Result<(), String> {
        let source = link.path();
        op.lock().unwrap().item_started(source);
        let parents = tree
            .file_parents(
                destination,
                &mut || Self::archive_checkpoint(op).map_err(std::io::Error::other),
                &mut |created| Self::record_created_copy_directory(op, tree, created, is_move),
            )
            .map_err(|error| format!("Cannot hold destination {:?}: {error}", destination))?;
        let published = if is_move {
            link.move_to(parents.directory(), destination, replace_existing)
                .map_err(|error| match error.kind() {
                    std::io::ErrorKind::PermissionDenied => "error_access_denied".to_string(),
                    _ => format!("Cannot move {:?} to {:?}: {error}", source, destination),
                })?
        } else {
            link.verify()
                .map_err(|error| format!("Cannot hold copy source {:?}: {error}", source))?;
            platform()
                .create_link(parents.directory(), destination, link.target(), replace_existing)
                .map_err(|error| match error.kind() {
                    std::io::ErrorKind::PermissionDenied => "error_access_denied".to_string(),
                    _ => format!("Cannot copy {:?} to {:?}: {error}", source, destination),
                })?
        };
        let identity = HistoryLinkIdentity::from_file(&published).unwrap_or_else(|error| {
            log::warn!(
                target: FILE_OPS_LOG_TARGET,
                "Link identity unavailable for {}: {error}",
                destination.display()
            );
            HistoryLinkIdentity::vacant()
        });
        let record = HistoryLink {
            source: source.to_path_buf(),
            path: destination.to_path_buf(),
            target: link.target().to_path_buf(),
            identity,
        };
        let mut journal = op.lock().unwrap();
        if replace_existing {
            journal.overwritten_files.push(destination.to_path_buf());
        }
        if is_move {
            journal.moved_links.push(record);
        } else if !replace_existing {
            journal.created_links.push(record);
        }
        journal.current_file = Some(source.to_string_lossy().into_owned());
        journal.item_effect(source);
        journal.item_done(source);
        Ok(())
    }
}
