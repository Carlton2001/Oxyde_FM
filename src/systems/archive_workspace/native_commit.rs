//! Native archive reconstruction and publication.
use super::*;
impl ArchiveWorkspace {
    #[cfg(all(target_os = "windows", not(test)))]
    pub(super) fn commit_native(&self, op: &Arc<Mutex<FileOperation>>, retained: &HashMap<PathBuf, std::fs::File>) -> Result<(), String> {
        use crate::platform::platform;
        use crate::systems::{archive_owned_tree::OwnedTree, path_parents::HeldParents};
        use crate::utils::archive_mutation_test::{checkpoint as hook, Stage};
        if !writable(&self.archive) { return Err("This archive format is read-only".into()); }
        hook(Stage::BeforeRebuild, self.root(), &self.archive).map_err(|e| e.to_string())?;
        let archive = self._parents.resolved_path(&self.archive).map_err(|e| e.to_string())?;
        let mut output = OwnedTree::temporary(self._parents.resolved_directory()).map_err(|e| e.to_string())?;
        let destination = output.path().join(archive.file_name().ok_or("Invalid archive name")?);
        let root = HeldParents::new(&self.root().join(".oxyde-rebuild-root")).map_err(|e| e.to_string())?;
        let owner = self.directory.owner.lock().unwrap();
        if platform().file_identity(root.directory()).map_err(|e| e.to_string())? != owner.root_identity() { return Err(crate::models::error::ArchiveSourceConflict.to_string()); }
        let expected = owner.identities();
        drop(owner);
        let mut sources = Vec::new();
        platform().enumerate_source_directory(root.directory(), &mut |name| {
            FileOperationManager::archive_checkpoint(op).map_err(std::io::Error::other)?;
            if sources.len() >= ArchiveLimits::WORKSPACE.max_entries { return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, crate::models::error::ArchiveLimitFailure { kind: crate::models::error::ArchiveLimitKind::Entries, limit: ArchiveLimits::WORKSPACE.max_entries as u64 })); }
            let path = root.resolved_directory().join(name);
            if !expected.contains_key(&path) { return Err(crate::models::error::SourceSelectionFailure::changed(&path)); }
            sources.push(path); Ok(())
        }).map_err(|e| e.to_string())?;
        let (original_destination, original_quality, quality) = {
            let locked = op.lock().unwrap();
            let extension = archive.extension().and_then(|e| e.to_str()).unwrap_or("zip").to_ascii_lowercase();
            (locked.destination.clone(), locked.compression_quality.clone(), locked.archive_qualities.get(&extension).cloned().unwrap_or_else(|| locked.compression_quality.clone()))
        };
        op.lock().unwrap().compression_quality = quality;
        let rename = self.rename.lock().unwrap();
        let result = FileOperationManager::perform_compress_workspace(op, sources, destination.clone(), retained, &expected, rename.as_ref().map(|(source, destination)| (source.as_path(), destination.as_path())));
        { let mut locked = op.lock().unwrap(); locked.destination = original_destination; locked.compression_quality = original_quality; }
        let rebuilt = result?;
        output.adopt_created_file(&destination, &rebuilt).map_err(|e| e.to_string())?;
        FileOperationManager::archive_checkpoint(op)?;
        hook(Stage::BeforeOriginalCheck, self.root(), &archive).map_err(|e| e.to_string())?;
        let mut original = self._parents.open_file(&archive).map_err(|e| e.to_string())?;
        let metadata = original.metadata().map_err(|e| e.to_string())?;
        if !metadata.is_file() || platform().file_attributes(&metadata, "").2 || metadata.len() != self.length || metadata.modified().ok() != self.modified
            || platform().file_identity(&original).map_err(|e| e.to_string())? != self.original_identity
            || fingerprint(&mut original, &|| FileOperationManager::archive_checkpoint(op).map_err(std::io::Error::other), &ArchiveBudget::new(ArchiveLimits::WORKSPACE)).map_err(|e| e.to_string())? != self.original_hash
            || platform().read_archive_output_provenance(self._parents.directory(), &original, &archive).map_err(|e| e.to_string())? != self.original_provenance {
            return Err(crate::models::error::ArchiveSourceConflict.to_string());
        }
        if let Some(data) = platform().read_archive_output_provenance(self._parents.directory(), &original, &archive).map_err(|e| e.to_string())? { output.mark(&destination, &rebuilt, &data).map_err(|e| e.to_string())?; }
        rebuilt.sync_all().map_err(|e| e.to_string())?;
        FileOperationManager::archive_checkpoint(op)?;
        let backup = self._parents.resolved_directory().join(format!(".oxyde-{}.archive-backup", uuid::Uuid::new_v4()));
        self._parents.rename(&original, &archive, &backup, false).map_err(|e| e.to_string())?;
        // No cancellation checkpoint between backup and publication/rollback.
        if let Err(error) = hook(Stage::AfterBackup, &archive, &backup).and_then(|()| self._parents.rename(&rebuilt, &destination, &archive, false)) {
            return match hook(Stage::BeforeRollback, &archive, &backup).and_then(|()| self._parents.rename(&original, &backup, &archive, false)) {
                Ok(()) => Err(error.to_string()),
                Err(restore) => { crate::systems::archive_recovery::retained(&backup); Err(format!("{error}; original kept at {}: {restore}", backup.display())) },
            };
        }
        crate::systems::archive_recovery::published(&archive);
        FileOperationManager::archive_publication_finished(op);
        if let Err(error) = hook(Stage::BeforeBackupRemoval, &archive, &backup).and_then(|()| platform().remove_open_file(&original, &backup)) { crate::systems::archive_recovery::retained(&backup); log::warn!("Archive updated, original backup retained at {}: {error}", backup.display()); }
        Ok(())
    }
}
