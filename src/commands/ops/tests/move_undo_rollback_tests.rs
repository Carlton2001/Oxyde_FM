    use super::*;
    use crate::models::TransactionDetails;

    #[test]
    fn move_history_preserves_identical_replacements_before_undo_and_redo() {
        for redo in [false, true] {
            let root = tempfile::tempdir().unwrap();
            let source = root.path().join("source.txt");
            let destination = root.path().join("destination.txt");
            std::fs::write(&destination, b"moving file").unwrap();
            let history = HistoryManager::new();
            history.push(Transaction::new(
                TransactionType::Move,
                details(&source, &destination),
            ));
            let runtime = tokio::runtime::Builder::new_current_thread()
                .build()
                .unwrap();
            if redo {
                runtime.block_on(undo_last_action(&history)).unwrap();
            }
            let selected = if redo { &source } else { &destination };
            let saved = root.path().join("selected-original.txt");
            let timestamp = std::fs::metadata(selected).unwrap().modified().unwrap();
            std::fs::rename(selected, &saved).unwrap();
            std::fs::write(selected, b"moving file").unwrap();
            std::fs::File::options()
                .write(true)
                .open(selected)
                .unwrap()
                .set_modified(timestamp)
                .unwrap();
            let result = if redo {
                runtime.block_on(redo_last_action(&history))
            } else {
                runtime.block_on(undo_last_action(&history))
            };
            assert!(
                matches!(result, Err(CommandError::HistoryMove(failure)) if failure.issue == crate::models::error::HistoryMoveIssue::Changed)
            );
            assert_eq!(std::fs::read(selected).unwrap(), b"moving file");
            assert_eq!(std::fs::read(&saved).unwrap(), b"moving file");
            assert!(!(if redo { &destination } else { &source }).exists());
            assert_eq!(history.get_state().undo_stack.len(), usize::from(!redo));
            assert_eq!(history.get_state().redo_stack.len(), usize::from(redo));
            std::fs::remove_file(selected).unwrap();
            std::fs::rename(saved, selected).unwrap();
            if redo {
                runtime.block_on(redo_last_action(&history)).unwrap();
            } else {
                runtime.block_on(undo_last_action(&history)).unwrap();
            }
        }
    }

    #[test]
    fn move_history_without_identity_preserves_data_and_stack() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source.txt");
        let destination = root.path().join("destination.txt");
        std::fs::write(&destination, b"moving file").unwrap();
        let mut journal = details(&source, &destination);
        journal.moved_file_identities = None;
        let history = HistoryManager::new();
        history.push(Transaction::new(TransactionType::Move, journal));
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        let error = runtime.block_on(undo_last_action(&history)).unwrap_err();
        assert!(
            matches!(error, CommandError::HistoryMove(failure) if failure.issue == crate::models::error::HistoryMoveIssue::Unverified)
        );
        assert_eq!(std::fs::read(&destination).unwrap(), b"moving file");
        assert!(!source.exists());
        assert_eq!(history.get_state().undo_stack.len(), 1);
        assert!(history.get_state().redo_stack.is_empty());
    }

    fn details(source: &std::path::Path, destination: &std::path::Path) -> TransactionDetails {
        TransactionDetails {
            deleted_items: None,
            created_links: None,
            renamed_item_identity: None,
            moved_directory_identities: None,
            new_folder_identity: None,
            moved_file_identities: std::fs::File::open(destination).ok().map(|file| {
                vec![(
                    destination.to_string_lossy().into_owned(),
                    crate::models::transaction::HistoryMoveIdentity::from_file(&file).unwrap(),
                )]
            }),
            copied_directory_identities: None,
            copied_file_identities: None,
            paths: vec![],
            target_dir: None,
            old_path: None,
            new_path: None,
            created_files: Some(vec![]),
            created_dirs: Some(vec![]),
            overwritten_files: Some(vec![]),
            moved_files: Some(vec![(
                source.to_string_lossy().into_owned(),
                destination.to_string_lossy().into_owned(),
            )]),
            trash_metadata_backups: None,
            moved_dirs: Some(vec![]),
            copied_files: Some(vec![]),
            copied_file_hashes: Some(vec![(
                destination.to_string_lossy().into_owned(),
                blake3::hash(b"moving file").to_hex().to_string(),
            )]),
        }
    }

    fn check_directory_failure(child_of_blocker: bool) {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source/file.txt");
        let destination = root.path().join("destination.txt");
        std::fs::write(&destination, b"moving file").unwrap();
        let blocker = root.path().join("directory-blocker");
        std::fs::write(&blocker, b"external file").unwrap();
        let mut journal = details(&source, &destination);
        let directory = if child_of_blocker {
            blocker.join("child")
        } else {
            blocker.clone()
        };
        journal.moved_dirs = Some(vec![(
            directory.to_string_lossy().into_owned(),
            root.path()
                .join("old-directory")
                .to_string_lossy()
                .into_owned(),
        )]);
        journal.moved_directory_identities = Some(
            journal
                .moved_dirs
                .as_ref()
                .unwrap()
                .iter()
                .map(|(path, _)| {
                    (
                        path.clone(),
                        crate::models::transaction::HistoryDirectoryIdentity::vacant(),
                    )
                })
                .collect(),
        );
        journal.copied_directory_identities = Some(
            journal
                .created_dirs
                .as_ref()
                .unwrap()
                .iter()
                .map(|path| {
                    (
                        path.clone(),
                        crate::models::transaction::HistoryDirectoryIdentity::from_file(
                            &crate::platform::platform()
                                .hold_resolved_operation_directory(
                                    &std::fs::canonicalize(path).unwrap(),
                                )
                                .unwrap(),
                        )
                        .unwrap(),
                    )
                })
                .collect(),
        );
        let history = HistoryManager::new();
        history.push(Transaction::new(TransactionType::Move, journal));
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        assert!(runtime.block_on(undo_last_action(&history)).is_err());
        assert_eq!(std::fs::read(&destination).unwrap(), b"moving file");
        assert!(!source.exists());
        assert_eq!(std::fs::read(&blocker).unwrap(), b"external file");
        assert_eq!(history.get_state().undo_stack.len(), 1);
        assert!(history.get_state().redo_stack.is_empty());
        std::fs::remove_file(blocker).unwrap();
        runtime.block_on(undo_last_action(&history)).unwrap();
        assert_eq!(std::fs::read(source).unwrap(), b"moving file");
        assert!(!destination.exists());
        assert!(history.get_state().undo_stack.is_empty());
    }

    #[test]
    fn directory_failure_keeps_files_and_history_for_retry() {
        check_directory_failure(true);
    }

    #[test]
    fn source_directory_cannot_be_replaced_by_a_file() {
        check_directory_failure(false);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn redo_move_roundtrips_nested_directory_history_twice() {
        let root = tempfile::tempdir().unwrap();
        let source_root = root.path().join("source");
        let source = source_root.join("child/file.txt");
        let destination_root = root.path().join("destination");
        let destination = destination_root.join("child/file.txt");
        std::fs::create_dir_all(destination.parent().unwrap()).unwrap();
        std::fs::write(&destination, b"moving file").unwrap();
        let mut journal = details(&source, &destination);
        journal.created_dirs = Some(vec![
            destination_root.to_string_lossy().into_owned(),
            destination.parent().unwrap().to_string_lossy().into_owned(),
        ]);
        journal.moved_dirs = Some(vec![
            (
                source_root.to_string_lossy().into_owned(),
                destination_root.to_string_lossy().into_owned(),
            ),
            (
                source.parent().unwrap().to_string_lossy().into_owned(),
                destination.parent().unwrap().to_string_lossy().into_owned(),
            ),
        ]);
        journal.moved_directory_identities = Some(
            journal
                .moved_dirs
                .as_ref()
                .unwrap()
                .iter()
                .map(|(path, _)| {
                    (
                        path.clone(),
                        crate::models::transaction::HistoryDirectoryIdentity::vacant(),
                    )
                })
                .collect(),
        );
        journal.copied_directory_identities = Some(
            journal
                .created_dirs
                .as_ref()
                .unwrap()
                .iter()
                .map(|path| {
                    (
                        path.clone(),
                        crate::models::transaction::HistoryDirectoryIdentity::from_file(
                            &crate::platform::platform()
                                .hold_resolved_operation_directory(
                                    &std::fs::canonicalize(path).unwrap(),
                                )
                                .unwrap(),
                        )
                        .unwrap(),
                    )
                })
                .collect(),
        );
        let transaction = Transaction::new(TransactionType::Move, journal);
        let history = HistoryManager::new();
        history.push(transaction.clone());
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        for _ in 0..2 {
            runtime.block_on(undo_last_action(&history)).unwrap();
            assert_eq!(std::fs::read(&source).unwrap(), b"moving file");
            assert!(!destination_root.exists());
            assert_eq!(history.get_state().redo_stack[0].id, transaction.id);
            runtime.block_on(redo_last_action(&history)).unwrap();
            assert_eq!(std::fs::read(&destination).unwrap(), b"moving file");
            assert!(!source_root.exists());
            assert_eq!(history.get_state().undo_stack[0].id, transaction.id);
        }
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn redo_move_destination_conflict_keeps_history_and_can_retry() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source.txt");
        let destination = root.path().join("destination.txt");
        std::fs::write(&destination, b"moving file").unwrap();
        let transaction = Transaction::new(TransactionType::Move, details(&source, &destination));
        let history = HistoryManager::new();
        history.push(transaction.clone());
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        runtime.block_on(undo_last_action(&history)).unwrap();
        std::fs::write(&destination, b"independent").unwrap();
        let error = runtime.block_on(redo_last_action(&history)).unwrap_err();
        assert!(
            matches!(error, CommandError::HistoryMove(failure) if failure.issue == crate::models::error::HistoryMoveIssue::Conflict)
        );
        assert_eq!(std::fs::read(&source).unwrap(), b"moving file");
        assert_eq!(std::fs::read(&destination).unwrap(), b"independent");
        assert_eq!(history.get_state().redo_stack[0].id, transaction.id);
        assert!(history.get_state().undo_stack.is_empty());
        std::fs::remove_file(&destination).unwrap();
        runtime.block_on(redo_last_action(&history)).unwrap();
        assert_eq!(std::fs::read(destination).unwrap(), b"moving file");
        assert!(!source.exists());
    }

    #[test]
    fn missing_destination_is_refused_before_history_changes() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source.txt");
        let destination = root.path().join("missing.txt");
        let history = HistoryManager::new();
        history.push(Transaction::new(
            TransactionType::Move,
            details(&source, &destination),
        ));
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        assert!(runtime.block_on(undo_last_action(&history)).is_err());
        assert_eq!(history.get_state().undo_stack.len(), 1);
        assert!(history.get_state().redo_stack.is_empty());
        assert!(!source.exists());
    }
