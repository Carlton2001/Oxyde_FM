    use super::{perform_copy_with_progress, redo_last_action, undo_last_action};
    use crate::models::HistoryManager;
    use crate::models::transaction::{Transaction, TransactionDetails, TransactionType};
    use std::sync::Arc;
    use std::sync::atomic::AtomicBool;

    #[test]
    fn redo_copy_directory_cleanup_preserves_a_late_empty_replacement() {
        let root = tempfile::tempdir().unwrap();
        let selected = root.path().join("created-directory");
        let displaced = root.path().join("original-directory");
        let mut directories = crate::systems::history_directories::Directories::prepare(
            std::slice::from_ref(&selected),
            std::slice::from_ref(&selected),
        )
        .unwrap();
        directories.create(&selected, false).unwrap();
        std::fs::rename(&selected, &displaced).unwrap();
        std::fs::create_dir(&selected).unwrap();
        let error = super::rollback_copy_directories(
            &mut directories,
            std::slice::from_ref(&selected),
            crate::models::CommandError::Other("injected copy failure".into()),
        );
        assert!(error.to_string().contains("rollback incomplete"), "{error}");
        assert!(
            error.to_string().contains(selected.to_str().unwrap()),
            "{error}"
        );
        assert!(selected.is_dir());
        assert!(displaced.is_dir());
        assert_eq!(std::fs::read_dir(&selected).unwrap().count(), 0);
        assert_eq!(std::fs::read_dir(&displaced).unwrap().count(), 0);
    }

    #[test]
    fn undo_copy_preserves_a_replacement_with_identical_contents() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source");
        let destination = root.path().join("copied");
        let displaced = root.path().join("original-copy");
        std::fs::write(&source, b"same contents").unwrap();
        std::fs::copy(&source, &destination).unwrap();
        let identity = crate::platform::platform()
            .file_identity(&std::fs::File::open(&destination).unwrap())
            .unwrap();
        let name = |path: &std::path::Path| path.to_string_lossy().into_owned();
        let transaction = Transaction::new(
            TransactionType::Copy,
            TransactionDetails {
                deleted_items: None,
                created_links: None,
                moved_links: None,
                directory_attributes: None,
                renamed_item_identity: None,
                moved_directory_identities: None,
                new_folder_identity: None,
                moved_file_identities: None,
                copied_directory_identities: None,
                copied_file_identities: Some(vec![(
                    destination.to_string_lossy().into_owned(),
                    crate::models::transaction::HistoryFileIdentity::from_file(
                        &std::fs::File::open(&destination).unwrap(),
                    )
                    .unwrap(),
                )]),
                paths: vec![name(&source)],
                target_dir: None,
                old_path: None,
                new_path: None,
                created_files: Some(vec![name(&destination)]),
                created_dirs: Some(vec![]),
                overwritten_files: Some(vec![]),
                moved_files: None,
                trash_metadata_backups: None,
                moved_dirs: None,
                copied_files: Some(vec![(name(&source), name(&destination))]),
                copied_file_hashes: Some(vec![(
                    name(&destination),
                    blake3::hash(b"same contents").to_hex().to_string(),
                )]),
            },
        );
        std::fs::rename(&destination, &displaced).unwrap();
        std::fs::write(&destination, b"same contents").unwrap();
        assert_ne!(
            crate::platform::platform()
                .file_identity(&std::fs::File::open(&destination).unwrap())
                .unwrap(),
            identity
        );
        let history = HistoryManager::new();
        history.push(transaction);
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        let result = runtime.block_on(undo_last_action(&history));
        assert!(
            matches!(result, Err(crate::models::CommandError::HistoryIdentity(failure)) if failure.issue == crate::models::error::HistoryIdentityIssue::Changed),
            "Undo must preserve a distinct object with matching contents"
        );
        assert_eq!(std::fs::read(&destination).unwrap(), b"same contents");
        assert_eq!(std::fs::read(&displaced).unwrap(), b"same contents");
        assert_eq!(
            (
                history.get_state().undo_stack.len(),
                history.get_state().redo_stack.len()
            ),
            (1, 0)
        );
        assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 3);
        std::fs::remove_file(&destination).unwrap();
        std::fs::rename(&displaced, &destination).unwrap();
        runtime
            .block_on(undo_last_action(&history))
            .unwrap()
            .unwrap();
        assert!(!destination.exists());
        assert_eq!(
            (
                history.get_state().undo_stack.len(),
                history.get_state().redo_stack.len()
            ),
            (0, 1)
        );
    }

    #[test]
    fn redo_copy_rejects_incomplete_or_changed_journals_and_can_retry() {
        use crate::models::error::HistoryCopyIssue;
        for missing_hash in [false, true] {
            let root = tempfile::tempdir().unwrap();
            let source = root.path().join("source.txt");
            let output_directory = root.path().join("copied-folder");
            std::fs::create_dir(&output_directory).unwrap();
            let destination = output_directory.join("copy.txt");
            std::fs::write(&source, b"original copy").unwrap();
            std::fs::write(&destination, b"original copy").unwrap();
            let name = |path: &std::path::Path| path.to_string_lossy().into_owned();
            let history = HistoryManager::new();
            history.push(Transaction::new(
                TransactionType::Copy,
                TransactionDetails {
                    deleted_items: None,
                    created_links: None,
                    moved_links: None,
                    directory_attributes: None,
                    renamed_item_identity: None,
                    moved_directory_identities: None,
                    new_folder_identity: None,
                    moved_file_identities: None,
                    copied_directory_identities: Some(vec![(
                        name(&output_directory),
                        crate::models::transaction::HistoryDirectoryIdentity::from_file(
                            &crate::platform::platform()
                                .hold_resolved_operation_directory(
                                    &std::fs::canonicalize(&output_directory).unwrap(),
                                )
                                .unwrap(),
                        )
                        .unwrap(),
                    )]),
                    copied_file_identities: Some(vec![(
                        destination.to_string_lossy().into_owned(),
                        crate::models::transaction::HistoryFileIdentity::from_file(
                            &std::fs::File::open(&destination).unwrap(),
                        )
                        .unwrap(),
                    )]),
                    paths: vec![name(&source)],
                    target_dir: None,
                    old_path: None,
                    new_path: None,
                    created_files: Some(vec![name(&destination)]),
                    created_dirs: Some(vec![name(&output_directory)]),
                    overwritten_files: Some(vec![]),
                    moved_files: None,
                    trash_metadata_backups: None,
                    moved_dirs: None,
                    copied_files: Some(vec![(name(&source), name(&destination))]),
                    copied_file_hashes: Some(if missing_hash {
                        vec![]
                    } else {
                        vec![(
                            name(&destination),
                            blake3::hash(b"original copy").to_hex().to_string(),
                        )]
                    }),
                },
            ));
            // Missing-proof fixture reserves the prior phase without guessing an Undo.
            let runtime = tokio::runtime::Builder::new_current_thread()
                .build()
                .unwrap();
            if missing_hash {
                history.begin_undo().unwrap().unwrap().complete();
                std::fs::remove_file(&destination).unwrap();
                std::fs::remove_dir(&output_directory).unwrap();
            } else {
                runtime.block_on(undo_last_action(&history)).unwrap();
                std::fs::write(&source, b"independent change").unwrap();
            }
            let error = runtime.block_on(redo_last_action(&history)).unwrap_err();
            assert!(
                matches!(error, crate::models::CommandError::HistoryCopy(failure) if failure.issue == if missing_hash { HistoryCopyIssue::Unverified } else { HistoryCopyIssue::SourceChanged })
            );
            assert!(!destination.exists());
            assert!(!output_directory.exists());
            assert_eq!(
                (
                    history.get_state().undo_stack.len(),
                    history.get_state().redo_stack.len()
                ),
                (0, 1)
            );
            if !missing_hash {
                assert_eq!(std::fs::read(&source).unwrap(), b"independent change");
                std::fs::write(&source, b"original copy").unwrap();
                runtime.block_on(redo_last_action(&history)).unwrap();
                assert_eq!(std::fs::read(&destination).unwrap(), b"original copy");
                assert_eq!(
                    (
                        history.get_state().undo_stack.len(),
                        history.get_state().redo_stack.len()
                    ),
                    (1, 0)
                );
                runtime.block_on(undo_last_action(&history)).unwrap();
                assert!(!destination.exists());
                runtime.block_on(redo_last_action(&history)).unwrap();
                assert_eq!(std::fs::read(&destination).unwrap(), b"original copy");
            } else {
                assert_eq!(std::fs::read(&source).unwrap(), b"original copy");
            }
        }
    }

    #[test]
    fn redo_copy_rolls_back_earlier_outputs_when_a_later_source_changed() {
        let root = tempfile::tempdir().unwrap();
        let first = root.path().join("first");
        let second = root.path().join("second");
        let first_destination = root.path().join("first-output");
        let second_destination = root.path().join("second-output");
        std::fs::write(&first, b"first copy").unwrap();
        std::fs::write(&second, b"changed second copy").unwrap();
        let result = perform_copy_with_progress(
            vec![
                (
                    first.clone(),
                    first_destination.clone(),
                    blake3::hash(b"first copy").to_hex().to_string(),
                ),
                (
                    second.clone(),
                    second_destination.clone(),
                    blake3::hash(b"old second copy").to_hex().to_string(),
                ),
            ],
            Arc::new(AtomicBool::new(false)),
        );
        assert!(matches!(
            result,
            Err(crate::models::CommandError::HistoryCopy(_))
        ));
        assert!(!first_destination.exists());
        assert!(!second_destination.exists());
        assert_eq!(std::fs::read(first).unwrap(), b"first copy");
        assert_eq!(std::fs::read(second).unwrap(), b"changed second copy");
        assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 2);
    }

    #[test]
    fn undo_copy_directory_error_preserves_outputs_and_allows_retry() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source.txt");
        let destination = root.path().join("copy.txt");
        let blocker = root.path().join("directory");
        std::fs::write(&source, b"copied content").unwrap();
        std::fs::write(&destination, b"copied content").unwrap();
        std::fs::create_dir(&blocker).unwrap();
        let displaced = root.path().join("original-directory");
        let name = |path: &std::path::Path| path.to_string_lossy().into_owned();
        let history = HistoryManager::new();
        history.push(Transaction::new(
            TransactionType::Copy,
            TransactionDetails {
                deleted_items: None,
                created_links: None,
                moved_links: None,
                directory_attributes: None,
                renamed_item_identity: None,
                moved_directory_identities: None,
                new_folder_identity: None,
                moved_file_identities: None,
                copied_directory_identities: Some(vec![(
                    name(&blocker),
                    crate::models::transaction::HistoryDirectoryIdentity::from_file(
                        &crate::platform::platform()
                            .hold_resolved_operation_directory(
                                &std::fs::canonicalize(&blocker).unwrap(),
                            )
                            .unwrap(),
                    )
                    .unwrap(),
                )]),
                copied_file_identities: Some(vec![(
                    destination.to_string_lossy().into_owned(),
                    crate::models::transaction::HistoryFileIdentity::from_file(
                        &std::fs::File::open(&destination).unwrap(),
                    )
                    .unwrap(),
                )]),
                paths: vec![name(&source)],
                target_dir: None,
                old_path: None,
                new_path: None,
                created_files: Some(vec![name(&destination)]),
                created_dirs: Some(vec![name(&blocker)]),
                overwritten_files: Some(vec![]),
                moved_files: None,
                trash_metadata_backups: None,
                moved_dirs: None,
                copied_files: Some(vec![(name(&source), name(&destination))]),
                copied_file_hashes: Some(vec![(
                    name(&destination),
                    blake3::hash(b"copied content").to_hex().to_string(),
                )]),
            },
        ));
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        std::fs::rename(&blocker, &displaced).unwrap();
        std::fs::write(&blocker, b"foreign content").unwrap();
        assert!(runtime.block_on(undo_last_action(&history)).is_err());
        assert_eq!(std::fs::read(&destination).unwrap(), b"copied content");
        assert_eq!(std::fs::read(&blocker).unwrap(), b"foreign content");
        assert_eq!(history.get_state().undo_stack.len(), 1);
        assert!(history.get_state().redo_stack.is_empty());
        std::fs::remove_file(&blocker).unwrap();
        std::fs::rename(&displaced, &blocker).unwrap();
        runtime.block_on(undo_last_action(&history)).unwrap();
        assert!(!destination.exists());
        assert_eq!(std::fs::read(source).unwrap(), b"copied content");
        assert_eq!(history.get_state().redo_stack.len(), 1);
    }

    #[test]
    fn redo_copy_rolls_back_new_files_when_a_later_destination_conflicts() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("source");
        let first_source = source.join("first.txt");
        let second_source = source.join("second.txt");
        let destination = temp.path().join("destination");
        let first_destination = destination.join("first.txt");
        let second_destination = destination.join("second.txt");
        std::fs::create_dir(&source).unwrap();
        std::fs::create_dir(&destination).unwrap();
        std::fs::write(&first_source, b"new file").unwrap();
        std::fs::write(&second_source, b"source version").unwrap();
        std::fs::write(&second_destination, b"existing version").unwrap();

        let result = perform_copy_with_progress(
            vec![
                (
                    first_source,
                    first_destination.clone(),
                    blake3::hash(b"new file").to_hex().to_string(),
                ),
                (
                    second_source,
                    second_destination.clone(),
                    blake3::hash(b"source version").to_hex().to_string(),
                ),
            ],
            Arc::new(AtomicBool::new(false)),
        );

        assert!(result.is_err());
        assert!(!first_destination.exists());
        assert_eq!(
            std::fs::read(second_destination).unwrap(),
            b"existing version"
        );
    }

    #[test]
    fn undo_copy_removes_its_verified_output_without_shell_trash() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("source.txt");
        let destination = temp.path().join("destination.txt");
        std::fs::write(&source, b"copied content").unwrap();
        std::fs::write(&destination, b"copied content").unwrap();
        let hash = blake3::hash(b"copied content").to_hex().to_string();
        let history = HistoryManager::new();
        history.push(Transaction::new(
            TransactionType::Copy,
            TransactionDetails {
                deleted_items: None,
                created_links: None,
                moved_links: None,
                directory_attributes: None,
                renamed_item_identity: None,
                moved_directory_identities: None,
                new_folder_identity: None,
                moved_file_identities: None,
                copied_directory_identities: None,
                copied_file_identities: Some(vec![(
                    destination.to_string_lossy().into_owned(),
                    crate::models::transaction::HistoryFileIdentity::from_file(
                        &std::fs::File::open(&destination).unwrap(),
                    )
                    .unwrap(),
                )]),
                paths: vec![source.to_string_lossy().into_owned()],
                target_dir: Some(temp.path().to_string_lossy().into_owned()),
                old_path: None,
                new_path: None,
                created_files: Some(vec![destination.to_string_lossy().into_owned()]),
                created_dirs: Some(Vec::new()),
                overwritten_files: Some(Vec::new()),
                moved_files: None,
                trash_metadata_backups: None,
                moved_dirs: None,
                copied_files: Some(vec![(
                    source.to_string_lossy().into_owned(),
                    destination.to_string_lossy().into_owned(),
                )]),
                copied_file_hashes: Some(vec![(destination.to_string_lossy().into_owned(), hash)]),
            },
        ));
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();

        runtime.block_on(undo_last_action(&history)).unwrap();

        assert!(!destination.exists());
        assert!(history.get_state().undo_stack.is_empty());
        assert_eq!(history.get_state().redo_stack.len(), 1);
    }
