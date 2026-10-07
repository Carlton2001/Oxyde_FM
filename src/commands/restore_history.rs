use crate::models::transaction::{RestoreEntry, RestoreState};
use crate::models::{CommandError, Transaction};
use std::{collections::HashSet, path::Path};

pub(super) trait TrashBackend {
    fn list(&self) -> Result<Vec<trash::TrashItem>, CommandError>;
    fn recycle(&self, path: &Path) -> Result<(), CommandError>;
    fn restore(&self, item: trash::TrashItem) -> Result<(), CommandError>;
}

pub(super) struct NativeTrash;

impl TrashBackend for NativeTrash {
    fn list(&self) -> Result<Vec<trash::TrashItem>, CommandError> {
        crate::platform::platform()
            .history_trash_items()
            .map_err(CommandError::TrashError)
    }
    fn recycle(&self, path: &Path) -> Result<(), CommandError> {
        crate::platform::platform()
            .recycle_for_history(path)
            .map_err(CommandError::TrashError)
    }
    fn restore(&self, item: trash::TrashItem) -> Result<(), CommandError> {
        crate::platform::platform()
            .restore_history_item(super::ops::item_with_correct_restore_name(&item))
            .map_err(CommandError::TrashError)
    }
}

fn failure(message: impl Into<String>) -> CommandError {
    CommandError::TrashError(message.into())
}

fn exists(path: &Path) -> Result<bool, CommandError> {
    match std::fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error.into()),
    }
}

fn same_path(left: &Path, right: &Path) -> bool {
    #[cfg(target_os = "windows")]
    {
        let normalize = |path: &Path| {
            path.to_string_lossy()
                .replace('/', "\\")
                .trim_start_matches("\\\\?\\")
                .to_lowercase()
        };
        normalize(left) == normalize(right)
    }
    #[cfg(not(target_os = "windows"))]
    {
        left == right
    }
}

pub(super) fn journal(transaction: &Transaction) -> Result<Vec<RestoreEntry>, CommandError> {
    let entries = transaction
        .restore_journal
        .clone()
        .filter(|entries| !entries.is_empty())
        .ok_or_else(|| {
            failure("This restoration has no effect journal and cannot be replayed safely")
        })?;
    for (index, entry) in entries.iter().enumerate() {
        if !entry.destination.is_absolute()
            || entries[..index].iter().any(|previous| {
                same_path(&previous.destination, &entry.destination)
                    || previous.destination.starts_with(&entry.destination)
                    || entry.destination.starts_with(&previous.destination)
            })
        {
            return Err(failure(
                "The restoration journal contains invalid or overlapping destinations",
            ));
        }
    }
    Ok(entries)
}

fn recorded_item<'a>(
    entry: &RestoreEntry,
    items: &'a [trash::TrashItem],
) -> Result<&'a trash::TrashItem, CommandError> {
    let RestoreState::Recycled { id } = &entry.state else {
        return Err(failure(
            "The restoration has no confirmed Recycle Bin receipt",
        ));
    };
    items
        .iter()
        .find(|item| {
            item.id == *id && same_path(&super::ops::original_trash_path(item), &entry.destination)
        })
        .ok_or_else(|| {
            failure(format!(
                "The exact recycled item is missing or changed: {}",
                entry.destination.display()
            ))
        })
}

// An interrupted enumeration must never cause a retry to delete another file.
fn capture_receipt(
    entry: &mut RestoreEntry,
    items: &[trash::TrashItem],
) -> Result<(), CommandError> {
    let RestoreState::Recycling { previous_ids } = &entry.state else {
        return Ok(());
    };
    let mut candidates = items.iter().filter(|item| {
        !previous_ids.contains(&item.id)
            && same_path(&super::ops::original_trash_path(item), &entry.destination)
    });
    let first = candidates.next();
    if candidates.next().is_some() {
        return Err(failure(format!(
            "Several new recycled items match {}; refusing an ambiguous replay",
            entry.destination.display()
        )));
    }
    if let Some(item) = first {
        if exists(&entry.destination)? {
            return Err(failure(format!(
                "A new file occupies the destination during Undo: {}",
                entry.destination.display()
            )));
        }
        entry.state = RestoreState::Recycled {
            id: item.id.clone(),
        };
        Ok(())
    } else {
        Err(failure(format!(
            "No confirmed recycled item for {}",
            entry.destination.display()
        )))
    }
}

pub(super) fn undo(
    entries: &mut [RestoreEntry],
    backend: &impl TrashBackend,
) -> Result<(), CommandError> {
    let items = backend.list()?;
    for entry in entries.iter_mut() {
        if matches!(entry.state, RestoreState::Recycling { .. }) {
            capture_receipt(entry, &items)?;
        }
        match &entry.state {
            RestoreState::Present if !exists(&entry.destination)? => {
                return Err(failure(format!(
                    "Cannot undo restoration; destination is missing: {}",
                    entry.destination.display()
                )))
            }
            RestoreState::Recycled { .. } => {
                recorded_item(entry, &items)?;
            }
            RestoreState::Restoring { .. } => {
                return Err(failure("A partial Redo must finish before another Undo"))
            }
            _ => {}
        }
    }
    for entry in entries {
        if !matches!(entry.state, RestoreState::Present) {
            continue;
        }
        if !exists(&entry.destination)? {
            return Err(failure("The destination disappeared before Undo"));
        }
        let previous_ids: HashSet<_> = backend.list()?.into_iter().map(|item| item.id).collect();
        entry.state = RestoreState::Recycling { previous_ids };
        let result = backend.recycle(&entry.destination);
        // Record observed effects even when the native operation reports an error.
        let after = backend.list()?;
        if result.is_err() && exists(&entry.destination)? {
            if let RestoreState::Recycling { previous_ids } = &entry.state {
                if !after.iter().any(|item| {
                    !previous_ids.contains(&item.id)
                        && same_path(&super::ops::original_trash_path(item), &entry.destination)
                }) {
                    entry.state = RestoreState::Present;
                    return result;
                }
            }
        }
        capture_receipt(entry, &after)?;
        result?;
    }
    Ok(())
}

fn capture_restoration(
    entry: &mut RestoreEntry,
    items: &[trash::TrashItem],
) -> Result<(), CommandError> {
    let RestoreState::Restoring { id } = &entry.state else {
        return Ok(());
    };
    let remains = items.iter().any(|item| item.id == *id);
    let present = exists(&entry.destination)?;
    match (remains, present) {
        (false, true) => entry.state = RestoreState::Present,
        (true, false) => entry.state = RestoreState::Recycled { id: id.clone() },
        _ => {
            return Err(failure(format!(
                "Restoration was not confirmed at {}",
                entry.destination.display()
            )))
        }
    }
    Ok(())
}

pub(super) fn redo(
    entries: &mut [RestoreEntry],
    backend: &impl TrashBackend,
) -> Result<(), CommandError> {
    let items = backend.list()?;
    for entry in entries.iter_mut() {
        capture_restoration(entry, &items)?;
        match entry.state {
            RestoreState::Present => {
                if !exists(&entry.destination)? {
                    return Err(failure(
                        "A restored destination disappeared during the partial replay",
                    ));
                }
            }
            RestoreState::Recycled { .. } => {
                recorded_item(entry, &items)?;
                if exists(&entry.destination)? {
                    return Err(failure(format!(
                        "Cannot redo restoration; destination already exists: {}",
                        entry.destination.display()
                    )));
                }
            }
            RestoreState::Recycling { .. } | RestoreState::Restoring { .. } => {
                return Err(failure(
                    "Undo has no confirmed receipt; Redo is unavailable",
                ))
            }
        }
    }
    for entry in entries {
        if matches!(entry.state, RestoreState::Present) {
            continue;
        }
        let item = recorded_item(entry, &backend.list()?)?.clone();
        if exists(&entry.destination)? {
            return Err(failure("A destination appeared before Redo"));
        }
        entry.state = RestoreState::Restoring {
            id: item.id.clone(),
        };
        let result = backend.restore(item);
        capture_restoration(entry, &backend.list()?)?;
        if !matches!(entry.state, RestoreState::Present) {
            return Err(result
                .err()
                .unwrap_or_else(|| failure("The native restore did not restore the item")));
        }
        result?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::transaction::{TransactionDetails, TransactionType};
    use crate::models::HistoryManager;
    use std::{cell::RefCell, ffi::OsString, path::PathBuf};

    #[derive(Default)]
    struct Effects {
        items: Vec<trash::TrashItem>,
        recycled: usize,
        restored: usize,
        lists: usize,
        fail_recycle: Option<(usize, bool)>,
        fail_restore: Option<(usize, bool)>,
        fail_list: Option<usize>,
        panic_after_effect: Option<&'static str>,
    }
    struct FixtureTrash {
        directory: tempfile::TempDir,
        effects: RefCell<Effects>,
    }
    impl FixtureTrash {
        fn new() -> Self {
            Self {
                directory: tempfile::tempdir().unwrap(),
                effects: RefCell::new(Effects::default()),
            }
        }
    }
    impl TrashBackend for FixtureTrash {
        fn list(&self) -> Result<Vec<trash::TrashItem>, CommandError> {
            let mut effects = self.effects.borrow_mut();
            effects.lists += 1;
            if effects.fail_list == Some(effects.lists) {
                return Err(failure("injected enumeration error"));
            }
            Ok(effects.items.clone())
        }
        fn recycle(&self, path: &Path) -> Result<(), CommandError> {
            let mut effects = self.effects.borrow_mut();
            effects.recycled += 1;
            let failing = effects
                .fail_recycle
                .filter(|(count, _)| *count == effects.recycled);
            if failing == Some((effects.recycled, false)) {
                return Err(failure("injected recycle error"));
            }
            let id = self
                .directory
                .path()
                .join(format!("$R{}", effects.recycled));
            std::fs::rename(path, &id)?;
            effects.items.push(trash::TrashItem {
                id: id.into_os_string(),
                name: path.file_name().unwrap().to_owned(),
                original_parent: path.parent().unwrap().to_owned(),
                time_deleted: 1,
            });
            if effects.panic_after_effect == Some("recycle") {
                effects.panic_after_effect = None;
                panic!("injected panic after recycling");
            }
            if failing.is_some() {
                Err(failure("injected error after recycling"))
            } else {
                Ok(())
            }
        }
        fn restore(&self, item: trash::TrashItem) -> Result<(), CommandError> {
            let mut effects = self.effects.borrow_mut();
            effects.restored += 1;
            let failing = effects
                .fail_restore
                .filter(|(count, _)| *count == effects.restored);
            if failing == Some((effects.restored, false)) {
                return Err(failure("injected restore error"));
            }
            std::fs::rename(&item.id, item.original_path())?;
            effects.items.retain(|remaining| remaining.id != item.id);
            if effects.panic_after_effect == Some("restore") {
                effects.panic_after_effect = None;
                panic!("injected panic after restoring");
            }
            if failing.is_some() {
                Err(failure("injected error after restoring"))
            } else {
                Ok(())
            }
        }
    }

    fn transaction(paths: &[PathBuf]) -> Transaction {
        Transaction::new(
            TransactionType::Restore,
            TransactionDetails {
                paths: Vec::new(),
                target_dir: None,
                old_path: None,
                new_path: None,
                created_files: Some(
                    paths
                        .iter()
                        .map(|p| p.to_string_lossy().into_owned())
                        .collect(),
                ),
                created_dirs: None,
                replaced_files: None,
                overwritten_files: None,
                moved_files: None,
                moved_dirs: None,
                copied_files: None,
                copied_file_hashes: None,
            },
        )
    }
    fn apply(
        history: &HistoryManager,
        backend: &impl TrashBackend,
        is_redo: bool,
    ) -> Result<(), CommandError> {
        let mut operation = if is_redo {
            history.begin_redo()
        } else {
            history.begin_undo()
        }
        .unwrap()
        .unwrap();
        operation.set_restore_journal(journal(operation.transaction())?);
        if is_redo {
            redo(operation.restore_journal_mut(), backend)?;
        } else {
            undo(operation.restore_journal_mut(), backend)?;
        }
        operation.complete();
        Ok(())
    }
    fn files(root: &Path) -> Vec<PathBuf> {
        let paths = vec![
            root.join("Document école.txt"),
            root.join("Raccourci école.lnk"),
        ];
        for path in &paths {
            std::fs::write(path, b"fixture contents").unwrap();
        }
        paths
    }

    #[test]
    fn receipts_replay_files_and_folders_with_new_ids_over_multiple_cycles() {
        let root = tempfile::tempdir().unwrap();
        let mut paths = files(root.path());
        let folder = root.path().join("Dossier école");
        std::fs::create_dir(&folder).unwrap();
        std::fs::write(folder.join("nested.txt"), b"nested contents").unwrap();
        paths.push(folder.clone());
        let backend = FixtureTrash::new();
        let history = HistoryManager::new();
        history.push(transaction(&paths));
        let mut previous_ids = HashSet::new();
        for _ in 0..2 {
            apply(&history, &backend, false).unwrap();
            assert!(paths.iter().all(|p| !p.exists()));
            let ids: HashSet<_> = backend.list().unwrap().into_iter().map(|i| i.id).collect();
            assert!(ids.is_disjoint(&previous_ids));
            previous_ids = ids;
            apply(&history, &backend, true).unwrap();
            assert_eq!(std::fs::read(&paths[0]).unwrap(), b"fixture contents");
            assert_eq!(
                std::fs::read(folder.join("nested.txt")).unwrap(),
                b"nested contents"
            );
            assert_eq!(history.get_state().undo_stack.len(), 1);
            assert!(history.get_state().redo_stack.is_empty());
        }
    }

    #[test]
    fn partial_undo_and_redo_keep_progress_and_retry_only_remaining_effects() {
        for after_effect in [false, true] {
            let root = tempfile::tempdir().unwrap();
            let paths = files(root.path());
            let backend = FixtureTrash::new();
            backend.effects.borrow_mut().fail_recycle = Some((2, after_effect));
            let history = HistoryManager::new();
            history.push(transaction(&paths));
            assert!(apply(&history, &backend, false).is_err());
            assert_eq!(history.get_state().undo_stack.len(), 1);
            assert!(history.get_state().redo_stack.is_empty());
            apply(&history, &backend, false).unwrap();
            assert_eq!(backend.list().unwrap().len(), 2);
            backend.effects.borrow_mut().fail_restore = Some((2, after_effect));
            assert!(apply(&history, &backend, true).is_err());
            assert_eq!(history.get_state().redo_stack.len(), 1);
            apply(&history, &backend, true).unwrap();
            assert!(paths.iter().all(|p| p.exists()));
            let effects = backend.effects.borrow();
            assert_eq!(effects.recycled, if after_effect { 2 } else { 3 });
            assert_eq!(effects.restored, if after_effect { 2 } else { 3 });
            assert!(effects.items.is_empty());
        }
    }

    #[test]
    fn panic_after_a_native_effect_keeps_progress_and_releases_the_reservation() {
        for is_redo in [false, true] {
            let root = tempfile::tempdir().unwrap();
            let paths = files(root.path());
            let backend = FixtureTrash::new();
            let history = HistoryManager::new();
            history.push(transaction(&paths));
            if is_redo {
                apply(&history, &backend, false).unwrap();
            }
            backend.effects.borrow_mut().panic_after_effect =
                Some(if is_redo { "restore" } else { "recycle" });
            assert!(
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| apply(
                    &history, &backend, is_redo
                )))
                .is_err()
            );
            apply(&history, &backend, is_redo).unwrap();
            assert!(paths.iter().all(|p| p.exists() == is_redo));
        }
    }

    #[test]
    fn interrupted_enumeration_recovers_receipts_and_restore_acknowledgements() {
        for is_redo in [false, true] {
            let root = tempfile::tempdir().unwrap();
            let paths = files(root.path());
            let backend = FixtureTrash::new();
            let history = HistoryManager::new();
            history.push(transaction(&paths));
            if is_redo {
                apply(&history, &backend, false).unwrap();
            }
            let mut effects = backend.effects.borrow_mut();
            // First list is preflight, second is before mutation, third acknowledges it.
            effects.fail_list = Some(effects.lists + 3);
            drop(effects);
            assert!(apply(&history, &backend, is_redo).is_err());
            apply(&history, &backend, is_redo).unwrap();
            assert!(paths.iter().all(|p| p.exists() == is_redo));
        }
    }

    #[test]
    fn redo_preflights_all_conflicts_and_does_not_select_an_older_homonym() {
        let root = tempfile::tempdir().unwrap();
        let paths = files(root.path());
        let backend = FixtureTrash::new();
        backend.effects.borrow_mut().items.push(trash::TrashItem {
            id: OsString::from("older-unrelated-item"),
            name: paths[0].file_name().unwrap().to_owned(),
            original_parent: root.path().to_owned(),
            time_deleted: 0,
        });
        let history = HistoryManager::new();
        history.push(transaction(&paths));
        apply(&history, &backend, false).unwrap();
        std::fs::write(&paths[1], b"new occupant").unwrap();
        assert!(apply(&history, &backend, true).is_err());
        assert_eq!(backend.effects.borrow().restored, 0);
        assert_eq!(std::fs::read(&paths[1]).unwrap(), b"new occupant");
        std::fs::remove_file(&paths[1]).unwrap();
        let first_id = backend.effects.borrow().items[1].id.clone();
        backend
            .effects
            .borrow_mut()
            .items
            .retain(|item| item.id != first_id);
        assert!(apply(&history, &backend, true).is_err());
        assert_eq!(backend.effects.borrow().restored, 0);
        assert_eq!(history.get_state().redo_stack.len(), 1);
    }

    #[test]
    fn missing_destinations_and_legacy_or_overlapping_journals_fail_before_mutation() {
        let root = tempfile::tempdir().unwrap();
        let paths = files(root.path());
        let backend = FixtureTrash::new();
        std::fs::remove_file(&paths[1]).unwrap();
        let history = HistoryManager::new();
        history.push(transaction(&paths));
        assert!(apply(&history, &backend, false).is_err());
        assert_eq!(backend.effects.borrow().recycled, 0);
        assert!(journal(&transaction(&[])).is_err());
        assert!(journal(&transaction(&[paths[0].clone(), paths[0].clone()])).is_err());
        assert!(journal(&transaction(&[root.path().to_owned(), paths[0].clone()])).is_err());
    }

    #[test]
    fn ambiguous_receipts_are_never_guessed_or_recycled_again() {
        let root = tempfile::tempdir().unwrap();
        let paths = files(root.path());
        let backend = FixtureTrash::new();
        let mut entries = journal(&transaction(&paths[..1])).unwrap();
        backend.recycle(&paths[0]).unwrap();
        let mut duplicate = backend.list().unwrap()[0].clone();
        duplicate.id = OsString::from("another-new-receipt");
        backend.effects.borrow_mut().items.push(duplicate);
        entries[0].state = RestoreState::Recycling {
            previous_ids: HashSet::new(),
        };
        assert!(undo(&mut entries, &backend).is_err());
        assert_eq!(backend.effects.borrow().recycled, 1);
    }

    #[cfg(target_os = "windows")]
    #[test]
    #[ignore = "sends only generated fixtures to the native Recycle Bin for history roundtrips"]
    fn native_restore_history_roundtrip_and_conflict() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("original");
        let destination = root.path().join("destination");
        std::fs::create_dir(&source).unwrap();
        std::fs::create_dir(&destination).unwrap();
        let mut sources = files(&source);
        let url = source.join("Lien école.url");
        std::fs::write(&url, b"URL fixture contents").unwrap();
        sources.push(url);
        let folder = source.join("Dossier école");
        std::fs::create_dir(&folder).unwrap();
        std::fs::write(folder.join("nested.txt"), b"nested contents").unwrap();
        sources.push(folder);
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let history = HistoryManager::new();
        // Restore any remaining fixtures before the temporary root is removed, even on panic.
        struct Cleanup(PathBuf);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                if let Ok(items) = NativeTrash.list() {
                    for item in items {
                        let path = super::super::ops::original_trash_path(&item);
                        if path.starts_with(&self.0)
                            && !path
                                .components()
                                .any(|c| c == std::path::Component::ParentDir)
                        {
                            // The UUID temporary root owns only this test's fixtures and blockers.
                            if let Ok(metadata) = std::fs::symlink_metadata(&path) {
                                if metadata.is_dir() {
                                    let _ = std::fs::remove_dir_all(&path);
                                } else {
                                    let _ = std::fs::remove_file(&path);
                                }
                            }
                            let _ = NativeTrash.restore(item);
                        }
                    }
                }
            }
        }
        let _cleanup = Cleanup(root.path().to_owned());
        for path in &sources {
            NativeTrash.recycle(path).unwrap();
        }
        let ids: Vec<_> = NativeTrash
            .list()
            .unwrap()
            .into_iter()
            .filter(|item| {
                super::super::ops::original_trash_path(item).parent() == Some(source.as_path())
            })
            .map(|item| item.id.to_string_lossy().into_owned())
            .collect();
        assert_eq!(ids.len(), sources.len());
        runtime
            .block_on(crate::commands::ops::move_from_trash(
                &history,
                ids,
                destination.to_string_lossy().into_owned(),
            ))
            .unwrap();
        let destinations: Vec<_> = sources
            .iter()
            .map(|p| destination.join(p.file_name().unwrap()))
            .collect();
        for _ in 0..2 {
            runtime
                .block_on(crate::commands::ops::undo_last_action(&history))
                .unwrap();
            assert!(destinations.iter().all(|p| !p.exists()));
            std::fs::write(&destinations[1], b"existing occupant").unwrap();
            assert!(runtime
                .block_on(crate::commands::ops::redo_last_action(&history))
                .is_err());
            assert_eq!(
                std::fs::read(&destinations[1]).unwrap(),
                b"existing occupant"
            );
            assert!(!destinations[0].exists());
            std::fs::remove_file(&destinations[1]).unwrap();
            runtime
                .block_on(crate::commands::ops::redo_last_action(&history))
                .unwrap();
            assert_eq!(
                std::fs::read(&destinations[0]).unwrap(),
                b"fixture contents"
            );
            assert_eq!(
                std::fs::read(destinations[3].join("nested.txt")).unwrap(),
                b"nested contents"
            );
            assert_eq!(history.get_state().undo_stack.len(), 1);
            assert!(history.get_state().redo_stack.is_empty());
        }
    }
}
