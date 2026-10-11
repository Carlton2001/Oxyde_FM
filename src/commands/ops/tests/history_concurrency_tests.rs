    use super::{redo_last_action, undo_last_action};
    use crate::models::HistoryManager;
    use crate::models::transaction::{Transaction, TransactionDetails, TransactionType};

    #[test]
    fn rename_history_keeps_missing_sources_retryable() {
        for redo in [false, true] {
            let temp = tempfile::tempdir().unwrap();
            let old = temp.path().join("old");
            let new = temp.path().join("new");
            let history = HistoryManager::new();
            let (source, destination) = if redo { (&old, &new) } else { (&new, &old) };
            std::fs::create_dir(source).unwrap();
            std::fs::write(source.join("child"), b"directory child").unwrap();
            let mut tx = rename_transaction(&old, &new);
            let parents = crate::systems::path_parents::HeldParents::new(source).unwrap();
            let file = crate::platform::platform()
                .open_rename_item(parents.directory(), &parents.resolved_path(source).unwrap())
                .unwrap();
            tx.details.renamed_item_identity =
                Some(crate::models::transaction::HistoryFileIdentity::from_file(&file).unwrap());
            drop(file);
            drop(parents);
            let saved = temp.path().join("saved-selected-source");
            std::fs::rename(source, &saved).unwrap();
            history.push(tx);
            if redo {
                history.begin_undo().unwrap().unwrap().complete();
            }
            let runtime = tokio::runtime::Builder::new_current_thread()
                .build()
                .unwrap();
            let result = if redo {
                runtime.block_on(redo_last_action(&history))
            } else {
                runtime.block_on(undo_last_action(&history))
            };
            assert!(result.is_err());
            let state = history.get_state();
            assert_eq!(
                (state.undo_stack.len(), state.redo_stack.len()),
                if redo { (0, 1) } else { (1, 0) }
            );
            assert!(!old.exists());
            assert!(!new.exists());
            // Restore only the benign test source, then retry the same reservation.
            std::fs::rename(&saved, source).unwrap();
            if redo {
                runtime.block_on(redo_last_action(&history)).unwrap();
            } else {
                runtime.block_on(undo_last_action(&history)).unwrap();
            }
            assert!(!source.exists());
            assert_eq!(
                std::fs::read(destination.join("child")).unwrap(),
                b"directory child"
            );
            let state = history.get_state();
            assert_eq!(
                (state.undo_stack.len(), state.redo_stack.len()),
                if redo { (1, 0) } else { (0, 1) }
            );
        }
    }

    #[test]
    fn rename_history_preserves_destinations_created_after_the_original_action() {
        let mut rejected = Vec::new();
        for redo in [false, true] {
            let temp = tempfile::tempdir().unwrap();
            let old = temp.path().join("old.txt");
            let new = temp.path().join("new.txt");
            let history = HistoryManager::new();
            let (source, destination) = if redo { (&old, &new) } else { (&new, &old) };
            std::fs::write(source, b"selected file").unwrap();
            std::fs::write(destination, b"independent later file").unwrap();
            let mut tx = rename_transaction(&old, &new);
            tx.details.renamed_item_identity = Some(
                crate::models::transaction::HistoryFileIdentity::from_file(
                    &std::fs::File::open(source).unwrap(),
                )
                .unwrap(),
            );
            history.push(tx);
            if redo {
                history.begin_undo().unwrap().unwrap().complete();
            }
            let runtime = tokio::runtime::Builder::new_current_thread()
                .build()
                .unwrap();
            let result = if redo {
                runtime.block_on(redo_last_action(&history))
            } else {
                runtime.block_on(undo_last_action(&history))
            };
            rejected.push((
                redo,
                result.is_err(),
                std::fs::read(destination).unwrap() == b"independent later file",
            ));
            if result.is_err() {
                assert_eq!(std::fs::read(source).unwrap(), b"selected file");
                let state = history.get_state();
                assert_eq!(
                    (state.undo_stack.len(), state.redo_stack.len()),
                    if redo { (0, 1) } else { (1, 0) }
                );
            }
        }
        assert!(
            rejected
                .iter()
                .all(|(_, failed, preserved)| *failed && *preserved),
            "rename history rejection: {rejected:?}"
        );
    }

    #[test]
    fn rename_history_preserves_replacements_before_undo_and_redo_and_can_retry() {
        for directory in [false, true] {
            for redo in [false, true] {
                let root = tempfile::tempdir().unwrap();
                let old = root.path().join("old");
                let new = root.path().join("new");
                let (source, destination) = if redo { (&old, &new) } else { (&new, &old) };
                if directory {
                    std::fs::create_dir(source).unwrap();
                } else {
                    std::fs::write(source, b"same contents").unwrap();
                }
                let mut tx = rename_transaction(&old, &new);
                if redo {
                    let parents = crate::systems::path_parents::HeldParents::new(source).unwrap();
                    let file = crate::platform::platform()
                        .open_rename_item(
                            parents.directory(),
                            &parents.resolved_path(source).unwrap(),
                        )
                        .unwrap();
                    tx.details.renamed_item_identity = Some(
                        crate::models::transaction::HistoryFileIdentity::from_file(&file).unwrap(),
                    );
                }
                let saved = root.path().join("saved-selected-source");
                std::fs::rename(source, &saved).unwrap();
                if directory {
                    std::fs::create_dir(source).unwrap();
                } else {
                    std::fs::write(source, b"same contents").unwrap();
                }
                let history = HistoryManager::new();
                history.push(tx);
                if redo {
                    history.begin_undo().unwrap().unwrap().complete();
                }
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .build()
                    .unwrap();
                let result = if redo {
                    runtime.block_on(redo_last_action(&history))
                } else {
                    runtime.block_on(undo_last_action(&history))
                };
                assert!(
                    matches!(result, Err(crate::models::CommandError::HistoryRename(failure))
                    if failure.issue == crate::models::error::HistoryIdentityIssue::Changed)
                );
                assert!(source.exists());
                assert!(saved.exists());
                assert!(!destination.exists());
                let state = history.get_state();
                assert_eq!(
                    (state.undo_stack.len(), state.redo_stack.len()),
                    if redo { (0, 1) } else { (1, 0) }
                );
                if directory {
                    std::fs::remove_dir(source).unwrap();
                } else {
                    std::fs::remove_file(source).unwrap();
                }
                std::fs::rename(saved, source).unwrap();
                if redo {
                    runtime.block_on(redo_last_action(&history)).unwrap();
                } else {
                    runtime.block_on(undo_last_action(&history)).unwrap();
                }
                assert!(destination.exists());
                assert!(!source.exists());
            }
        }
    }

    #[test]
    fn rename_history_requires_complete_paths_and_identity_before_stack_changes() {
        for redo in [false, true] {
            for missing_path in [false, true] {
                let root = tempfile::tempdir().unwrap();
                let old = root.path().join("old");
                let new = root.path().join("new");
                let (source, destination) = if redo { (&old, &new) } else { (&new, &old) };
                std::fs::write(source, b"selected contents").unwrap();
                let mut tx = rename_transaction(&old, &new);
                if missing_path {
                    tx.details.old_path = None;
                } else {
                    tx.details.renamed_item_identity = None;
                }
                let history = HistoryManager::new();
                history.push(tx);
                if redo {
                    history.begin_undo().unwrap().unwrap().complete();
                }
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .build()
                    .unwrap();
                let result = if redo {
                    runtime.block_on(redo_last_action(&history))
                } else {
                    runtime.block_on(undo_last_action(&history))
                };
                assert!(
                    matches!(result, Err(crate::models::CommandError::HistoryRename(failure))
                    if failure.issue == crate::models::error::HistoryIdentityIssue::Unverified)
                );
                assert_eq!(std::fs::read(source).unwrap(), b"selected contents");
                assert!(!destination.exists());
                let state = history.get_state();
                assert_eq!(
                    (state.undo_stack.len(), state.redo_stack.len()),
                    if redo { (0, 1) } else { (1, 0) }
                );
            }
        }
    }

    fn rename_transaction(old: &std::path::Path, new: &std::path::Path) -> Transaction {
        Transaction::new(
            TransactionType::Rename,
            TransactionDetails {
                deleted_items: None,
                renamed_item_identity: [new, old].into_iter().find_map(|path| {
                    let parents = crate::systems::path_parents::HeldParents::new(path).ok()?;
                    let file = crate::platform::platform()
                        .open_rename_item(parents.directory(), &parents.resolved_path(path).ok()?)
                        .ok()?;
                    crate::models::transaction::HistoryFileIdentity::from_file(&file).ok()
                }),
                moved_directory_identities: None,
                new_folder_identity: None,
                moved_file_identities: None,
                copied_directory_identities: None,
                copied_file_identities: None,
                paths: vec![],
                target_dir: None,
                old_path: Some(old.to_string_lossy().into_owned()),
                new_path: Some(new.to_string_lossy().into_owned()),
                created_files: None,
                created_dirs: None,
                overwritten_files: None,
                moved_files: None,
                trash_metadata_backups: None,
                moved_dirs: None,
                copied_files: None,
                copied_file_hashes: None,
            },
        )
    }

    #[test]
    fn concurrent_commands_do_not_touch_files_and_can_retry_after_release() {
        let temp = tempfile::tempdir().unwrap();
        let old = temp.path().join("old.txt");
        let new = temp.path().join("new.txt");
        std::fs::write(&new, b"fixture content").unwrap();
        let history = HistoryManager::new();
        let tx = rename_transaction(&old, &new);
        history.push(tx.clone());
        let selected = history.begin_undo().unwrap().unwrap();
        std::thread::scope(|scope| {
            scope
                .spawn(|| {
                    let runtime = tokio::runtime::Builder::new_current_thread()
                        .build()
                        .unwrap();
                    assert!(runtime.block_on(undo_last_action(&history)).is_err());
                    assert!(runtime.block_on(redo_last_action(&history)).is_err());
                    assert!(!old.exists());
                    assert_eq!(std::fs::read(&new).unwrap(), b"fixture content");
                })
                .join()
                .unwrap();
        });
        drop(selected);
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        assert_eq!(
            runtime
                .block_on(undo_last_action(&history))
                .unwrap()
                .unwrap()
                .id,
            tx.id
        );
        assert!(!new.exists());
        assert_eq!(std::fs::read(&old).unwrap(), b"fixture content");
        let selected = history.begin_redo().unwrap().unwrap();
        assert!(runtime.block_on(redo_last_action(&history)).is_err());
        assert!(runtime.block_on(undo_last_action(&history)).is_err());
        assert!(old.exists());
        assert!(!new.exists());
        drop(selected);
        assert_eq!(
            runtime
                .block_on(redo_last_action(&history))
                .unwrap()
                .unwrap()
                .id,
            tx.id
        );
        assert!(!old.exists());
        assert_eq!(std::fs::read(&new).unwrap(), b"fixture content");
        assert_eq!(history.get_state().undo_stack[0].id, tx.id);
        assert!(history.get_state().redo_stack.is_empty());
    }

    #[test]
    fn io_error_keeps_history_and_allows_another_attempt() {
        let temp = tempfile::tempdir().unwrap();
        let old = temp.path().join("missing-parent").join("old.txt");
        let new = temp.path().join("new.txt");
        std::fs::write(&new, b"fixture content").unwrap();
        let history = HistoryManager::new();
        let tx = rename_transaction(&old, &new);
        history.push(tx.clone());
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        assert!(runtime.block_on(undo_last_action(&history)).is_err());
        assert_eq!(history.get_state().undo_stack[0].id, tx.id);
        assert!(history.get_state().redo_stack.is_empty());
        assert_eq!(std::fs::read(&new).unwrap(), b"fixture content");
        std::fs::create_dir(old.parent().unwrap()).unwrap();
        assert!(
            runtime
                .block_on(undo_last_action(&history))
                .unwrap()
                .is_some()
        );
        assert_eq!(std::fs::read(&old).unwrap(), b"fixture content");
        assert!(history.get_state().undo_stack.is_empty());
        assert_eq!(history.get_state().redo_stack[0].id, tx.id);
    }
