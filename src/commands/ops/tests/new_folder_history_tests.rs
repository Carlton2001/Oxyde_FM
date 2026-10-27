    use super::*;
    use crate::models::{TransactionDetails, TransactionType};

    fn details(path: &std::path::Path) -> TransactionDetails {
        TransactionDetails {
            deleted_items: None,
            created_links: None,
            moved_links: None,
            directory_attributes: None,
            renamed_item_identity: None,
            moved_directory_identities: None,
            new_folder_identity: None,
            moved_file_identities: None,
            paths: vec![path.to_string_lossy().into_owned()],
            target_dir: None,
            old_path: None,
            new_path: None,
            created_files: None,
            created_dirs: None,
            overwritten_files: None,
            moved_files: None,
            trash_metadata_backups: None,
            moved_dirs: None,
            copied_files: None,
            copied_file_hashes: None,
            copied_file_identities: None,
            copied_directory_identities: None,
        }
    }

    #[test]
    fn new_folder_undo_refuses_a_nonempty_replacement_without_recycling_it() {
        let root = tempfile::tempdir().unwrap();
        let selected = root.path().join("new-folder");
        let displaced = root.path().join("original-folder");
        let simulated_recycle = root.path().join("simulated-recycle");
        std::fs::create_dir(&selected).unwrap();
        let mut journal = details(&selected);
        journal.new_folder_identity = crate::models::transaction::HistoryFileIdentity::from_file(
            &crate::platform::platform()
                .hold_resolved_operation_directory(&std::fs::canonicalize(&selected).unwrap())
                .unwrap(),
        )
        .ok();
        std::fs::rename(&selected, &displaced).unwrap();
        std::fs::create_dir(&selected).unwrap();
        std::fs::write(selected.join("independent.txt"), b"independent contents").unwrap();
        let history = HistoryManager::new();
        history.push(Transaction::new(TransactionType::NewFolder, journal));
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        let result = runtime.block_on(undo_last_action(&history));
        assert!(
            matches!(result, Err(CommandError::NewFolderHistory(failure)) if failure.issue == crate::models::error::NewFolderHistoryIssue::Changed)
        );
        assert_eq!(history.get_state().undo_stack.len(), 1);
        assert!(history.get_state().redo_stack.is_empty());
        assert_eq!(
            std::fs::read(selected.join("independent.txt")).unwrap(),
            b"independent contents"
        );
        assert!(displaced.is_dir());
        assert!(!simulated_recycle.exists());
    }
    fn created_transaction(path: &std::path::Path) -> Transaction {
        let mut journal = details(path);
        journal.new_folder_identity = crate::commands::io::create_local_folder(path).unwrap();
        Transaction::new(TransactionType::NewFolder, journal)
    }

    fn runtime() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap()
    }

    #[test]
    fn new_folder_undo_preserves_later_children_and_can_retry() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("created");
        let history = HistoryManager::new();
        let transaction = created_transaction(&path);
        let identity = transaction.details.new_folder_identity.unwrap();
        history.push(transaction.clone());
        std::fs::write(path.join("later-file.txt"), b"later contents").unwrap();
        std::fs::create_dir(path.join("later-directory")).unwrap();
        let runtime = runtime();
        let error = runtime.block_on(undo_last_action(&history)).unwrap_err();
        assert!(
            matches!(error, CommandError::NewFolderHistory(failure) if failure.issue == crate::models::error::NewFolderHistoryIssue::NotEmpty)
        );
        assert_eq!(
            std::fs::read(path.join("later-file.txt")).unwrap(),
            b"later contents"
        );
        assert!(path.join("later-directory").is_dir());
        assert_eq!(history.get_state().undo_stack[0].id, transaction.id);
        assert!(history.get_state().redo_stack.is_empty());
        assert!(
            identity
                .matches(
                    &crate::platform::platform()
                        .hold_resolved_operation_directory(&std::fs::canonicalize(&path).unwrap())
                        .unwrap()
                )
                .unwrap()
        );
        std::fs::remove_file(path.join("later-file.txt")).unwrap();
        std::fs::remove_dir(path.join("later-directory")).unwrap();
        runtime.block_on(undo_last_action(&history)).unwrap();
        assert!(!path.exists());
        assert_eq!(history.get_state().redo_stack[0].id, transaction.id);
    }

    #[test]
    fn new_folder_history_roundtrips_and_tracks_the_immediate_rename() {
        let root = tempfile::tempdir().unwrap();
        let initial = root.path().join("Nouveau dossier");
        let renamed = root.path().join("Mes données_日本語");
        let history = HistoryManager::new();
        let transaction = created_transaction(&initial);
        history.push(transaction.clone());
        let runtime = runtime();
        let proof = runtime
            .block_on(crate::commands::io::rename_item_controlled_with_identity(
                initial.to_string_lossy().into_owned(),
                renamed.to_string_lossy().into_owned(),
                "normal".into(),
                Arc::new(AtomicBool::new(false)),
            ))
            .unwrap().unwrap();
        assert!(crate::commands::io::record_captured_folder_rename(
            &history,
            &initial.to_string_lossy(),
            &renamed.to_string_lossy(),
            proof,
        ));
        assert_eq!(
            history.get_state().undo_stack[0].details.paths,
            vec![renamed.to_string_lossy().into_owned()]
        );
        for _ in 0..2 {
            let state = history.get_state();
            assert_eq!(state.undo_stack[0].id, transaction.id);
            assert!(
                state.undo_stack[0]
                    .details
                    .new_folder_identity
                    .unwrap()
                    .matches(
                        &crate::platform::platform()
                            .hold_resolved_operation_directory(
                                &std::fs::canonicalize(&renamed).unwrap()
                            )
                            .unwrap()
                    )
                    .unwrap()
            );
            // No journal handle locks the folder between commands.
            std::fs::rename(&renamed, &initial).unwrap();
            std::fs::rename(&initial, &renamed).unwrap();
            runtime.block_on(undo_last_action(&history)).unwrap();
            assert!(!renamed.exists());
            assert!(!initial.exists());
            runtime.block_on(redo_last_action(&history)).unwrap();
            assert!(renamed.is_dir());
            assert!(!initial.exists());
        }
        runtime.block_on(undo_last_action(&history)).unwrap();
        assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 0);
    }

    #[test]
    fn new_folder_rename_tracking_never_adopts_a_replacement_or_changes_a_reservation() {
        let root = tempfile::tempdir().unwrap();
        let initial = root.path().join("created");
        let renamed = root.path().join("renamed");
        let displaced = root.path().join("original");
        let history = HistoryManager::new();
        history.push(created_transaction(&initial));
        std::fs::rename(&initial, &renamed).unwrap();
        let selected = history.begin_undo().unwrap().unwrap();
        assert!(!crate::commands::io::record_created_folder_rename(
            &history,
            &initial.to_string_lossy(),
            &renamed.to_string_lossy()
        ));
        assert_eq!(
            selected.transaction().details.paths,
            vec![initial.to_string_lossy().into_owned()]
        );
        drop(selected);
        std::fs::rename(&renamed, &displaced).unwrap();
        std::fs::create_dir(&renamed).unwrap();
        assert!(!crate::commands::io::record_created_folder_rename(
            &history,
            &initial.to_string_lossy(),
            &renamed.to_string_lossy()
        ));
        assert_eq!(
            history.get_state().undo_stack[0].details.paths,
            vec![initial.to_string_lossy().into_owned()]
        );
        assert!(renamed.is_dir());
        assert!(displaced.is_dir());
    }

    #[test]
    fn new_folder_redo_preserves_occupied_destinations_and_keeps_the_action_retryable() {
        for directory in [false, true] {
            let root = tempfile::tempdir().unwrap();
            let path = root.path().join("created");
            let history = HistoryManager::new();
            let transaction = created_transaction(&path);
            history.push(transaction.clone());
            let runtime = runtime();
            runtime.block_on(undo_last_action(&history)).unwrap();
            if directory {
                std::fs::create_dir(&path).unwrap();
            }
            let witness = if directory {
                path.join("independent.txt")
            } else {
                path.clone()
            };
            std::fs::write(&witness, b"independent contents").unwrap();
            let error = runtime.block_on(redo_last_action(&history)).unwrap_err();
            assert!(
                matches!(error, CommandError::NewFolderHistory(failure) if failure.issue == crate::models::error::NewFolderHistoryIssue::Conflict)
            );
            assert_eq!(std::fs::read(&witness).unwrap(), b"independent contents");
            assert_eq!(history.get_state().redo_stack[0].id, transaction.id);
            assert!(history.get_state().undo_stack.is_empty());
            std::fs::remove_file(&witness).unwrap();
            if directory {
                std::fs::remove_dir(&path).unwrap();
            }
            runtime.block_on(redo_last_action(&history)).unwrap();
            runtime.block_on(undo_last_action(&history)).unwrap();
            assert!(!path.exists());
        }
    }

    #[test]
    fn new_folder_history_requires_a_proof_and_a_single_created_path() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("created");
        let other = root.path().join("other");
        let mut transaction = created_transaction(&path);
        std::fs::create_dir(&other).unwrap();
        let proof = transaction.details.new_folder_identity.take();
        let history = HistoryManager::new();
        history.push(transaction.clone());
        let runtime = runtime();
        let error = runtime.block_on(undo_last_action(&history)).unwrap_err();
        assert!(
            matches!(error, CommandError::NewFolderHistory(failure) if failure.issue == crate::models::error::NewFolderHistoryIssue::Unverified)
        );
        assert!(path.is_dir());
        assert!(other.is_dir());
        history.clear();
        transaction.details.new_folder_identity = proof;
        transaction
            .details
            .paths
            .push(other.to_string_lossy().into_owned());
        history.push(transaction);
        let error = runtime.block_on(undo_last_action(&history)).unwrap_err();
        assert!(
            matches!(error, CommandError::NewFolderHistory(failure) if failure.issue == crate::models::error::NewFolderHistoryIssue::Unverified)
        );
        assert!(path.is_dir());
        assert!(other.is_dir());
        assert_eq!(history.get_state().undo_stack.len(), 1);
    }

    #[test]
    fn new_folder_history_preserves_missing_sources_and_does_not_recreate_parents() {
        let root = tempfile::tempdir().unwrap();
        let parent = root.path().join("parent");
        std::fs::create_dir(&parent).unwrap();
        let path = parent.join("created");
        let displaced = root.path().join("original");
        let history = HistoryManager::new();
        history.push(created_transaction(&path));
        std::fs::rename(&path, &displaced).unwrap();
        let runtime = runtime();
        let error = runtime.block_on(undo_last_action(&history)).unwrap_err();
        assert!(
            matches!(error, CommandError::NewFolderHistory(failure) if failure.issue == crate::models::error::NewFolderHistoryIssue::Missing)
        );
        assert!(displaced.is_dir());
        assert!(!path.exists());
        assert_eq!(history.get_state().undo_stack.len(), 1);
        std::fs::rename(&displaced, &path).unwrap();
        runtime.block_on(undo_last_action(&history)).unwrap();
        std::fs::remove_dir(&parent).unwrap();
        let error = runtime.block_on(redo_last_action(&history)).unwrap_err();
        assert!(
            matches!(error, CommandError::NewFolderHistory(failure) if failure.issue == crate::models::error::NewFolderHistoryIssue::ParentMissing)
        );
        assert!(!parent.exists());
        assert_eq!(history.get_state().redo_stack.len(), 1);
        std::fs::create_dir(&parent).unwrap();
        runtime.block_on(redo_last_action(&history)).unwrap();
        runtime.block_on(undo_last_action(&history)).unwrap();
        assert!(!path.exists());
    }
