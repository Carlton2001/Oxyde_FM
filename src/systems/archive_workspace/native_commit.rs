//! Native archive reconstruction and publication.
use super::*;
use crate::{
    platform::platform,
    systems::{archive_owned_tree::OwnedTree, archive_recovery, path_parents::HeldParents},
    utils::archive_mutation_test::{checkpoint as hook, Stage},
};
use std::{fs::File, io};

pub(super) struct Prepared<'a> {
    workspace: &'a ArchiveWorkspace,
    original: File,
    rebuilt: File,
    destination_parents: HeldParents,
    // Destination rename guards must close before owned output cleanup.
    _output: OwnedTree,
    destination: PathBuf,
    archive: PathBuf,
    backup: PathBuf,
    backup_moved: bool,
    published: bool,
}
impl ArchiveWorkspace {
    #[cfg(all(target_os = "windows", not(test)))]
    pub(super) fn commit_native(
        &self,
        op: &Arc<Mutex<FileOperation>>,
        retained: &HashMap<PathBuf, std::fs::File>,
    ) -> Result<(), String> {
        let prepared = self.prepare_native(op, retained)?;
        publish_all(&mut [prepared], op)
    }
    pub(super) fn prepare_native(
        &self,
        op: &Arc<Mutex<FileOperation>>,
        retained: &HashMap<PathBuf, File>,
    ) -> Result<Prepared<'_>, String> {
        if !writable(&self.archive) {
            return Err("This archive format is read-only".into());
        }
        hook(Stage::BeforeRebuild, self.root(), &self.archive).map_err(|e| e.to_string())?;
        let archive = self
            ._parents
            .resolved_path(&self.archive)
            .map_err(|e| e.to_string())?;
        let mut output =
            OwnedTree::temporary(self._parents.resolved_directory()).map_err(|e| e.to_string())?;
        let destination = output
            .path()
            .join(archive.file_name().ok_or("Invalid archive name")?);
        let root = HeldParents::new(&self.root().join(".oxyde-rebuild-root"))
            .map_err(|e| e.to_string())?;
        let owner = self.directory.owner.lock().unwrap();
        if platform()
            .file_identity(root.directory())
            .map_err(|e| e.to_string())?
            != owner.root_identity()
        {
            return Err(crate::models::error::ArchiveSourceConflict.to_string());
        }
        let expected = owner.identities();
        drop(owner);
        let mut sources = Vec::new();
        platform()
            .enumerate_source_directory(root.directory(), &mut |name| {
                FileOperationManager::archive_checkpoint(op).map_err(std::io::Error::other)?;
                if sources.len() >= ArchiveLimits::WORKSPACE.max_entries {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        crate::models::error::ArchiveLimitFailure {
                            kind: crate::models::error::ArchiveLimitKind::Entries,
                            limit: ArchiveLimits::WORKSPACE.max_entries as u64,
                        },
                    ));
                }
                let path = root.resolved_directory().join(name);
                if !expected.contains_key(&path) {
                    return Err(crate::models::error::SourceSelectionFailure::changed(&path));
                }
                sources.push(path);
                Ok(())
            })
            .map_err(|e| e.to_string())?;
        let (original_destination, original_quality, quality) = {
            let locked = op.lock().unwrap();
            let extension = archive
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("zip")
                .to_ascii_lowercase();
            (
                locked.destination.clone(),
                locked.compression_quality.clone(),
                locked
                    .archive_qualities
                    .get(&extension)
                    .cloned()
                    .unwrap_or_else(|| locked.compression_quality.clone()),
            )
        };
        op.lock().unwrap().compression_quality = quality;
        let rename = self.rename.lock().unwrap();
        let result = FileOperationManager::perform_compress_workspace(
            op,
            sources,
            destination.clone(),
            retained,
            &expected,
            rename
                .as_ref()
                .map(|(source, destination)| (source.as_path(), destination.as_path())),
        );
        {
            let mut locked = op.lock().unwrap();
            locked.destination = original_destination;
            locked.compression_quality = original_quality;
        }
        let rebuilt = result?;
        output
            .adopt_created_file(&destination, &rebuilt)
            .map_err(|e| e.to_string())?;
        FileOperationManager::archive_checkpoint(op)?;
        hook(Stage::BeforeOriginalCheck, self.root(), &archive).map_err(|e| e.to_string())?;
        let mut original = self
            ._parents
            .open_file(&archive)
            .map_err(|e| e.to_string())?;
        let metadata = original.metadata().map_err(|e| e.to_string())?;
        if !metadata.is_file()
            || platform().file_attributes(&metadata, "").2
            || metadata.len() != self.length
            || metadata.modified().ok() != self.modified
            || platform()
                .file_identity(&original)
                .map_err(|e| e.to_string())?
                != self.original_identity
            || fingerprint(
                &mut original,
                &|| FileOperationManager::archive_checkpoint(op).map_err(std::io::Error::other),
                &ArchiveBudget::new(ArchiveLimits::WORKSPACE),
            )
            .map_err(|e| e.to_string())?
                != self.original_hash
            || platform()
                .read_archive_output_provenance(self._parents.directory(), &original, &archive)
                .map_err(|e| e.to_string())?
                != self.original_provenance
        {
            return Err(crate::models::error::ArchiveSourceConflict.to_string());
        }
        if let Some(data) = platform()
            .read_archive_output_provenance(self._parents.directory(), &original, &archive)
            .map_err(|e| e.to_string())?
        {
            output
                .mark(&destination, &rebuilt, &data)
                .map_err(|e| e.to_string())?;
        }
        rebuilt.sync_all().map_err(|e| e.to_string())?;
        FileOperationManager::archive_checkpoint(op)?;
        let backup = self
            ._parents
            .resolved_directory()
            .join(format!(".oxyde-{}.archive-backup", uuid::Uuid::new_v4()));
        let destination_parents = HeldParents::new(&destination).map_err(|e| e.to_string())?;
        Ok(Prepared {
            workspace: self,
            original,
            rebuilt,
            _output: output,
            destination_parents,
            destination,
            archive,
            backup,
            backup_moved: false,
            published: false,
        })
    }
}

impl Prepared<'_> {
    fn publish(&mut self) -> io::Result<()> {
        self.workspace
            ._parents
            .rename(&self.original, &self.archive, &self.backup, false)?;
        self.backup_moved = true;
        hook(Stage::AfterBackup, &self.archive, &self.backup)?;
        self.workspace
            ._parents
            .rename(&self.rebuilt, &self.destination, &self.archive, false)?;
        self.published = true;
        Ok(())
    }
    fn rollback(&mut self) -> io::Result<()> {
        if !self.backup_moved {
            return Ok(());
        }
        hook(Stage::BeforeRollback, &self.archive, &self.backup)?;
        if self.published {
            // Move only the held rebuilt object, never remove a replacement by
            // name. A foreign destination blocks rollback and is preserved.
            self.destination_parents.rename(
                &self.rebuilt,
                &self.archive,
                &self.destination,
                false,
            )?;
            self.published = false;
        }
        self.workspace
            ._parents
            .rename(&self.original, &self.backup, &self.archive, false)?;
        self.backup_moved = false;
        Ok(())
    }
    fn finish(&mut self) {
        archive_recovery::published(&self.archive);
        if let Err(error) = hook(Stage::BeforeBackupRemoval, &self.archive, &self.backup)
            .and_then(|()| platform().remove_open_file(&self.original, &self.backup))
        {
            archive_recovery::retained(&self.backup);
            log::warn!(
                "Archive updated, original backup retained at {}: {error}",
                self.backup.display()
            );
        }
        // Publication was committed; a retained backup is recovery data, not
        // a reason to undo the whole successful group during destruction.
        self.backup_moved = false;
        self.published = false;
    }
}
impl Drop for Prepared<'_> {
    fn drop(&mut self) {
        if self.backup_moved {
            archive_recovery::retained(&self.backup);
        }
        if self.published {
            archive_recovery::published(&self.archive);
        }
    }
}

struct RollbackOnUnwind<'a, 'b> {
    entries: &'a mut [Prepared<'b>],
    handled: bool,
}
impl Drop for RollbackOnUnwind<'_, '_> {
    fn drop(&mut self) {
        if self.handled {
            return;
        }
        for entry in self.entries.iter_mut().rev() {
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| entry.rollback()));
        }
    }
}
pub(super) fn publish_all(
    entries: &mut [Prepared<'_>],
    op: &Arc<Mutex<FileOperation>>,
) -> Result<(), String> {
    let mut group = RollbackOnUnwind {
        entries,
        handled: false,
    };
    for index in 0..group.entries.len() {
        // Cancellation between complete publications rolls the earlier ones
        // back. There is no checkpoint inside backup/publication or rollback.
        let result = FileOperationManager::archive_checkpoint(op).and_then(|()| {
            group.entries[index]
                .publish()
                .map_err(|error| error.to_string())
        });
        if let Err(mut error) = result {
            for entry in group.entries.iter_mut().rev() {
                if let Err(restore) = entry.rollback() {
                    error.push_str(&format!(
                        "; original kept at {}: {restore}",
                        entry.backup.display()
                    ));
                }
            }
            group.handled = true;
            return Err(error);
        }
    }
    FileOperationManager::archive_publication_finished(op);
    group.handled = true;
    for entry in group.entries.iter_mut() {
        entry.finish();
    }
    Ok(())
}
