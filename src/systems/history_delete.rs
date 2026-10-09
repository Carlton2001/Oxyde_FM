//! Delete history verifies the object; Shell deletion itself still uses a path.
use super::path_parents::HeldParents;
use crate::{
    models::{
        CommandError, TransactionDetails,
        error::HistoryDeleteFailure,
        transaction::{HistoryDeletedItem, HistoryFileIdentity},
    },
    platform::platform,
};
use std::{fs::File, path::Path};
pub(crate) fn failure(path: &str, changed: bool) -> CommandError {
    CommandError::TrashError(
        HistoryDeleteFailure {
            changed,
            path: path.into(),
        }
        .to_string(),
    )
}
struct SelectedSource {
    _parents: HeldParents,
    file: File,
}
impl SelectedSource {
    fn new(path: &Path) -> Result<Self, CommandError> {
        let parents = HeldParents::new(path)?;
        let file =
            platform().track_source_item(parents.directory(), &parents.resolved_path(path)?)?;
        Ok(Self {
            _parents: parents,
            file,
        })
    }
    fn expected(entry: &HistoryDeletedItem) -> Result<Self, CommandError> {
        let proof = entry.identity.ok_or_else(|| failure(&entry.path, false))?;
        let selected = Self::new(Path::new(&entry.path)).map_err(|_| failure(&entry.path, true))?;
        if !proof
            .matches(&selected.file)
            .map_err(|_| failure(&entry.path, false))?
        {
            return Err(failure(&entry.path, true));
        }
        Ok(selected)
    }
}
fn source_failure(path: &Path, changed: bool) -> CommandError {
    CommandError::TrashError(
        crate::models::error::SourceSelectionFailure {
            changed,
            path: path.to_string_lossy().into_owned(),
        }
        .to_string(),
    )
}
pub(super) struct PreparedDelete {
    path: std::path::PathBuf,
    source: SelectedSource,
    identity: Option<HistoryFileIdentity>,
    resolved: String,
    ordinary_directory: bool,
}
impl PreparedDelete {
    pub(super) fn new(path: &Path) -> Result<Self, CommandError> {
        let source = SelectedSource::new(path)?;
        let identity = HistoryFileIdentity::from_file(&source.file)
            .map_err(|error| {
                log::warn!(
                    "Delete history proof unavailable for {}: {error}",
                    path.display()
                );
            })
            .ok();
        let resolved = source
            ._parents
            .resolved_path(path)?
            .to_string_lossy()
            .replace('/', "\\");
        let metadata = source.file.metadata()?;
        let ordinary_directory = metadata.is_dir() && !platform().file_attributes(&metadata, "").2;
        Ok(Self {
            path: path.to_path_buf(),
            source,
            identity,
            resolved,
            ordinary_directory,
        })
    }
    pub(super) fn path(&self) -> &Path {
        &self.path
    }
    pub(super) fn recycle(&self) -> Result<HistoryDeletedItem, CommandError> {
        self.recycle_with(&mut |path| {
            platform()
                .shell_delete(&[path.to_path_buf()], true)
                .map_err(CommandError::TrashError)
        })
    }
    fn recycle_with(
        &self,
        delete: &mut dyn FnMut(&Path) -> Result<(), CommandError>,
    ) -> Result<HistoryDeletedItem, CommandError> {
        if let Some(identity) = self.identity {
            let current =
                SelectedSource::new(&self.path).map_err(|_| source_failure(&self.path, true))?;
            if !identity
                .matches(&current.file)
                .map_err(|_| source_failure(&self.path, false))?
            {
                return Err(source_failure(&self.path, true));
            }
        }
        // Keep the captured object alive through dispatch, even if its name moves.
        // Shared DELETE access permits the Shell operation; its lookup is not atomic.
        let _source = &self.source;
        delete(&self.path)?;
        Ok(HistoryDeletedItem::new(
            self.path.to_string_lossy().into_owned(),
            self.identity,
        ))
    }
}
/// Drop descendant guards before recycling a parent directory; otherwise their
/// retained ancestor handles would block the parent's legitimate Shell move.
pub(super) fn collapse_roots(selected: Vec<PreparedDelete>) -> Vec<PreparedDelete> {
    // One lookup per ancestor, rather than comparing every pair of roots.
    let keep = {
        let directories = selected
            .iter()
            .filter(|item| item.ordinary_directory)
            .map(|item| item.resolved.as_str())
            .collect::<std::collections::HashSet<_>>();
        let mut seen = std::collections::HashSet::new();
        selected
            .iter()
            .map(|item| {
                if !seen.insert(item.resolved.as_str()) {
                    return false;
                }
                let mut ancestor = item.resolved.as_str();
                while let Some((parent, _)) = ancestor.rsplit_once('\\') {
                    if directories.contains(parent) {
                        return false;
                    }
                    ancestor = parent;
                }
                true
            })
            .collect::<Vec<_>>()
    };
    selected
        .into_iter()
        .zip(keep)
        .filter_map(|(item, keep)| keep.then_some(item))
        .collect()
}
#[cfg(test)]
fn recycle_with(
    path: &Path,
    delete: &mut dyn FnMut(&Path) -> Result<(), CommandError>,
) -> Result<HistoryDeletedItem, CommandError> {
    PreparedDelete::new(path)?.recycle_with(delete)
}
fn entries(details: &TransactionDetails) -> Result<&[HistoryDeletedItem], CommandError> {
    let Some(entries) = details.deleted_items.as_deref() else {
        return Err(failure("", false));
    };
    if entries.is_empty() || entries.len() != details.paths.len() {
        return Err(failure("", false));
    }
    let mut seen = std::collections::HashSet::new();
    for (path, entry) in details.paths.iter().zip(entries) {
        if path != &entry.path
            || entry.identity.is_none()
            || !seen.insert(path.to_lowercase().replace('/', "\\"))
        {
            return Err(failure(path, false));
        }
    }
    Ok(entries)
}
pub(crate) fn undo(details: &TransactionDetails) -> Result<(), CommandError> {
    let entries = entries(details)?;
    let items = platform()
        .list_trash_items()
        .map_err(CommandError::TrashError)?;
    let items = items
        .iter()
        .map(crate::commands::ops::item_with_correct_restore_name)
        .collect();
    platform()
        .restore_trash_history(items, entries)
        .map_err(CommandError::TrashError)
}
pub(crate) fn redo(details: &TransactionDetails) -> Result<(), CommandError> {
    redo_with(entries(details)?, &mut |path| {
        platform()
            .shell_delete(&[path.to_path_buf()], true)
            .map_err(CommandError::TrashError)
    })
}
fn redo_with(
    entries: &[HistoryDeletedItem],
    delete: &mut dyn FnMut(&Path) -> Result<(), CommandError>,
) -> Result<(), CommandError> {
    // Preflight the whole remaining batch and keep metadata/parents retained.
    let selected = entries
        .iter()
        .filter(|entry| entry.is_restored())
        .map(|entry| SelectedSource::expected(entry).map(|source| (entry, source)))
        .collect::<Result<Vec<_>, _>>()?;
    for (entry, source) in selected {
        // The path-based Shell call has a residual race after this check.
        let current = SelectedSource::expected(entry)?;
        delete(Path::new(&entry.path))?;
        entry.set_restored(false);
        drop((current, source));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    fn selected(path: &Path) -> HistoryDeletedItem {
        let source = SelectedSource::new(path).unwrap();
        let entry = HistoryDeletedItem::new(
            path.to_string_lossy().into_owned(),
            Some(HistoryFileIdentity::from_file(&source.file).unwrap()),
        );
        entry.set_restored(true);
        entry
    }
    fn details(items: Vec<HistoryDeletedItem>) -> TransactionDetails {
        TransactionDetails {
            paths: items.iter().map(|entry| entry.path.clone()).collect(),
            deleted_items: Some(items),
            target_dir: None,
            old_path: None,
            new_path: None,
            created_files: None,
            created_dirs: None,
            overwritten_files: None,
            moved_files: None,
            moved_file_identities: None,
            trash_metadata_backups: None,
            renamed_item_identity: None,
            moved_directory_identities: None,
            moved_dirs: None,
            copied_files: None,
            copied_file_hashes: None,
            copied_file_identities: None,
            copied_directory_identities: None,
            new_folder_identity: None,
        }
    }
    #[test]
    fn delete_records_the_object_before_dispatch_and_shares_phases_without_file_locks() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("selected");
        let payload = root.path().join("payload");
        fs::write(&source, b"selected contents").unwrap();
        let entry = recycle_with(&source, &mut |path| {
            fs::rename(path, &payload)?;
            Ok(())
        })
        .unwrap();
        assert_eq!(entry.path, source.to_string_lossy());
        assert!(!entry.is_restored());
        assert!(
            entry
                .identity
                .unwrap()
                .matches(&File::open(&payload).unwrap())
                .unwrap()
        );
        let clone = entry.clone();
        clone.set_restored(true);
        assert!(entry.is_restored());
        fs::rename(&payload, &source).unwrap(); // Journal values hold no handles.
        assert!(
            entry
                .identity
                .unwrap()
                .matches(&File::open(&source).unwrap())
                .unwrap()
        );
    }
    #[test]
    fn delete_history_failures_keep_the_selected_undo_or_redo_reservation() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("selected");
        let saved = root.path().join("saved");
        std::fs::write(&source, b"selected contents").unwrap();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let history = crate::models::HistoryManager::new();
        let mut no_proof = details(vec![selected(&source)]);
        no_proof.deleted_items = None;
        let transaction =
            crate::models::Transaction::new(crate::models::TransactionType::Delete, no_proof);
        let id = transaction.id.clone();
        history.push(transaction);
        assert!(
            runtime
                .block_on(crate::commands::ops::undo_last_action(&history))
                .is_err()
        );
        assert_eq!(history.get_state().undo_stack[0].id, id);
        assert!(history.get_state().redo_stack.is_empty());
        history.clear();
        let transaction = crate::models::Transaction::new(
            crate::models::TransactionType::Delete,
            details(vec![selected(&source)]),
        );
        let id = transaction.id.clone();
        history.push(transaction);
        history.begin_undo().unwrap().unwrap().complete(); // Simulate the already restored phase.
        std::fs::rename(&source, &saved).unwrap();
        std::fs::write(&source, b"independent contents").unwrap();
        assert!(
            runtime
                .block_on(crate::commands::ops::redo_last_action(&history))
                .is_err()
        );
        assert_eq!(history.get_state().redo_stack[0].id, id);
        assert!(history.get_state().undo_stack.is_empty());
        assert_eq!(std::fs::read(source).unwrap(), b"independent contents");
        assert_eq!(std::fs::read(saved).unwrap(), b"selected contents");
    }
    #[test]
    fn delete_preparation_preserves_an_object_replaced_before_dispatch() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("selected");
        let saved = root.path().join("saved");
        std::fs::write(&source, b"matching contents").unwrap();
        let prepared = PreparedDelete::new(&source).unwrap();
        std::fs::rename(&source, &saved).unwrap();
        std::fs::write(&source, b"matching contents").unwrap();
        let mut called = false;
        let error = prepared
            .recycle_with(&mut |_| {
                called = true;
                Ok(())
            })
            .unwrap_err();
        assert!(!called);
        assert_eq!(
            crate::models::error::SourceSelectionFailure::from_message(&error.to_string()),
            Some(true)
        );
        assert_eq!(std::fs::read(source).unwrap(), b"matching contents");
        assert_eq!(std::fs::read(saved).unwrap(), b"matching contents");
    }
    #[test]
    fn delete_roots_collapse_duplicates_and_descendants_before_parent_dispatch() {
        let root = tempfile::tempdir().unwrap();
        let directory = root.path().join("selected");
        std::fs::create_dir(&directory).unwrap();
        let child = directory.join("child");
        std::fs::write(&child, b"selected contents").unwrap();
        let selected = [child.clone(), directory.clone(), directory.clone()]
            .iter()
            .map(|path| PreparedDelete::new(path).unwrap())
            .collect();
        let selected = collapse_roots(selected);
        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].path(), directory);
        let saved = root.path().join("saved");
        let entry = selected[0]
            .recycle_with(&mut |path| {
                std::fs::rename(path, &saved)?;
                Ok(())
            })
            .unwrap();
        assert_eq!(entry.path, directory.to_string_lossy());
        assert_eq!(
            std::fs::read(saved.join("child")).unwrap(),
            b"selected contents"
        );
    }
    #[test]
    fn delete_redo_preflights_every_object_and_preserves_same_content_replacements() {
        let root = tempfile::tempdir().unwrap();
        let first = root.path().join("first");
        let second = root.path().join("second");
        let saved = root.path().join("saved");
        fs::write(&first, b"matching contents").unwrap();
        fs::write(&second, b"matching contents").unwrap();
        let journal = [selected(&first), selected(&second)];
        fs::rename(&second, &saved).unwrap();
        fs::write(&second, b"matching contents").unwrap();
        let mut called = false;
        let error = redo_with(&journal, &mut |_| {
            called = true;
            Ok(())
        })
        .unwrap_err();
        assert!(!called);
        assert_eq!(
            HistoryDeleteFailure::from_message(&error.to_string()),
            Some(true)
        );
        for path in [&first, &second, &saved] {
            assert_eq!(fs::read(path).unwrap(), b"matching contents");
        }
        assert!(journal.iter().all(HistoryDeletedItem::is_restored));
    }
    #[test]
    fn delete_redo_remembers_partial_effects_and_retries_only_the_remaining_objects() {
        let root = tempfile::tempdir().unwrap();
        let first = root.path().join("first");
        let second = root.path().join("second");
        fs::write(&first, b"first contents").unwrap();
        fs::write(&second, b"second contents").unwrap();
        let journal = [selected(&first), selected(&second)];
        let clone = journal.clone();
        let error = redo_with(&journal, &mut |path| {
            if path == second {
                return Err(CommandError::TrashError("injected stop".into()));
            }
            fs::rename(path, path.with_extension("payload"))?;
            Ok(())
        })
        .unwrap_err();
        assert!(error.to_string().contains("injected stop"));
        assert!(!clone[0].is_restored());
        assert!(clone[1].is_restored());
        fs::write(&first, b"independent later contents").unwrap();
        let mut calls = 0;
        redo_with(&clone, &mut |path| {
            calls += 1;
            assert_eq!(path, second);
            fs::rename(path, path.with_extension("payload"))?;
            Ok(())
        })
        .unwrap();
        assert_eq!(calls, 1);
        assert_eq!(fs::read(first).unwrap(), b"independent later contents");
        assert!(clone.iter().all(|entry| !entry.is_restored()));
        redo_with(&clone, &mut |_| {
            panic!("completed effects must not be replayed")
        })
        .unwrap();
    }
    #[test]
    fn delete_history_refuses_missing_proofs_or_inconsistent_journals_before_native_calls() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("selected");
        fs::write(&source, b"selected contents").unwrap();
        let entry = selected(&source);
        for mode in 0..4 {
            let mut details = details(vec![entry.clone()]);
            match mode {
                0 => details.deleted_items = None,
                1 => details.deleted_items.as_mut().unwrap()[0].identity = None,
                2 => details.paths.push("unrecorded".into()),
                _ => details.paths[0] = "different path".into(),
            }
            for error in [undo(&details).unwrap_err(), redo(&details).unwrap_err()] {
                assert_eq!(
                    HistoryDeleteFailure::from_message(&error.to_string()),
                    Some(false)
                );
            }
            assert_eq!(fs::read(&source).unwrap(), b"selected contents");
        }
    }
}
