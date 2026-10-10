use super::*;

#[cfg(target_os = "windows")]
#[test]
fn copy_undo_preserves_a_replacement_directory_containing_the_original_output() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source");
    let target_dir = root.path().join("target");
    std::fs::create_dir(&source).unwrap();
    std::fs::create_dir(&target_dir).unwrap();
    std::fs::write(source.join("copied.txt"), b"original copied contents").unwrap();
    let target = target_dir.join("source");
    let displaced = root.path().join("original-directory");
    let history = Arc::new(HistoryManager::new());
    let op = Arc::new(Mutex::new(FileOperation::new(
        FileOpType::Copy,
        vec![source],
        Some(target_dir),
        None,
    )));
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        FileOperationManager::execute_operation(history.clone(), op.clone()).await;
        assert_eq!(op.lock().unwrap().status, OpStatus::Completed);
        let proof = history.get_state().undo_stack[0].details.copied_file_identities.as_ref().unwrap()[0].1;
        std::fs::rename(&target, &displaced).unwrap();
        std::fs::create_dir(&target).unwrap();
        std::fs::rename(displaced.join("copied.txt"), target.join("copied.txt")).unwrap();
        assert!(proof.matches(&std::fs::File::open(target.join("copied.txt")).unwrap()).unwrap());
        let error = crate::commands::ops::undo_last_action(&history).await.unwrap_err();
        assert!(matches!(error, crate::models::CommandError::HistoryIdentity(failure) if failure.issue == crate::models::error::HistoryIdentityIssue::Changed));
        assert_eq!(std::fs::read(target.join("copied.txt")).unwrap(), b"original copied contents");
        assert!(displaced.is_dir());
        assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 3);
        assert_eq!(history.get_state().undo_stack.len(), 1);
        assert!(history.get_state().redo_stack.is_empty());
    });
}

#[cfg(target_os = "windows")]
#[test]
fn copy_undo_preserves_an_empty_directory_replaced_since_creation() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source-empty");
    let target_dir = root.path().join("target");
    std::fs::create_dir_all(source.join("nested-empty")).unwrap();
    std::fs::create_dir(&target_dir).unwrap();
    let target = target_dir.join("source-empty");
    let displaced = root.path().join("original-copy");
    let history = Arc::new(HistoryManager::new());
    let op = Arc::new(Mutex::new(FileOperation::new(
        FileOpType::Copy,
        vec![source],
        Some(target_dir.clone()),
        None,
    )));
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        FileOperationManager::execute_operation(history.clone(), op.clone()).await;
        assert_eq!(op.lock().unwrap().status, OpStatus::Completed);
        let original = platform().hold_resolved_operation_directory(&std::fs::canonicalize(&target).unwrap()).unwrap();
        let identity = platform().file_identity(&original).unwrap();
        drop(original);
        std::fs::rename(&target, &displaced).unwrap();
        std::fs::create_dir(&target).unwrap();
        let replacement = platform().hold_resolved_operation_directory(&std::fs::canonicalize(&target).unwrap()).unwrap();
        assert_ne!(platform().file_identity(&replacement).unwrap(), identity);
        drop(replacement);
        let result = crate::commands::ops::undo_last_action(&history).await;
        assert!(matches!(result, Err(crate::models::CommandError::HistoryIdentity(failure)) if failure.issue == crate::models::error::HistoryIdentityIssue::Changed), "Undo must preserve an independent empty replacement directory");
        assert!(target.is_dir());
        assert!(displaced.join("nested-empty").is_dir());
        assert_eq!(std::fs::read_dir(&target).unwrap().count(), 0);
        assert_eq!(history.get_state().undo_stack.len(), 1);
        assert!(history.get_state().redo_stack.is_empty());
        std::fs::remove_dir(&target).unwrap();
        std::fs::rename(&displaced, &target).unwrap();
        let transaction_id = history.get_state().undo_stack[0].id.clone();
        for _ in 0..2 {
            let state = history.get_state();
            assert_eq!(state.undo_stack[0].id, transaction_id);
            let identities = state.undo_stack[0].details.copied_directory_identities.as_ref().unwrap();
            assert_eq!(identities.len(), 2);
            for (path, identity) in identities {
                let directory = platform().hold_resolved_operation_directory(&std::fs::canonicalize(path).unwrap()).unwrap();
                assert!(identity.snapshot().unwrap().matches(&directory).unwrap());
            }
            // Keeping history must not pin either copied directory in place.
            std::fs::rename(&target, &displaced).unwrap();
            std::fs::rename(&displaced, &target).unwrap();
            crate::commands::ops::undo_last_action(&history).await.unwrap();
            assert!(!target.exists());
            crate::commands::ops::redo_last_action(&history).await.unwrap();
            assert!(target.join("nested-empty").is_dir());
        }
        crate::commands::ops::undo_last_action(&history).await.unwrap();
        assert_eq!(std::fs::read_dir(&target_dir).unwrap().count(), 0);
    });
}

#[cfg(target_os = "windows")]
#[test]
fn native_readonly_copy_history_preserves_streams_on_redo() {
    struct WritablePaths(Vec<PathBuf>);
    impl Drop for WritablePaths {
        fn drop(&mut self) {
            for path in &self.0 {
                if let Ok(metadata) = std::fs::metadata(path) {
                    let mut permissions = metadata.permissions();
                    permissions.set_readonly(false);
                    std::fs::set_permissions(path, permissions).unwrap();
                }
            }
        }
    }
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source.txt");
    let target_dir = root.path().join("target");
    std::fs::create_dir(&target_dir).unwrap();
    let target = target_dir.join("source.txt");
    let _cleanup = WritablePaths(vec![source.clone(), target.clone()]);
    std::fs::write(&source, b"readonly main").unwrap();
    std::fs::write(root.path().join("source.txt:extra"), b"readonly stream").unwrap();
    let modified = std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_700_000_000);
    std::fs::File::options()
        .write(true)
        .open(&source)
        .unwrap()
        .set_times(std::fs::FileTimes::new().set_modified(modified))
        .unwrap();
    let mut permissions = std::fs::metadata(&source).unwrap().permissions();
    permissions.set_readonly(true);
    std::fs::set_permissions(&source, permissions).unwrap();
    let history = Arc::new(HistoryManager::new());
    let op = Arc::new(Mutex::new(FileOperation::new(
        FileOpType::Copy,
        vec![source.clone()],
        Some(target_dir.clone()),
        None,
    )));
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        FileOperationManager::execute_operation(history.clone(), op.clone()).await;
        assert_eq!(op.lock().unwrap().status, OpStatus::Completed);
        let transaction_id = history.get_state().undo_stack[0].id.clone();
        for _ in 0..2 {
            let state = history.get_state();
            assert_eq!(state.undo_stack[0].id, transaction_id);
            let identities = state.undo_stack[0]
                .details
                .copied_file_identities
                .as_ref()
                .unwrap();
            assert_eq!(identities.len(), 1);
            assert_eq!(
                PathBuf::from(&identities[0].0),
                std::fs::canonicalize(&target).unwrap()
            );
            assert!(
                identities[0]
                    .1
                    .matches(&std::fs::File::open(&target).unwrap())
                    .unwrap()
            );
            // The journal contains values only; it must not keep a rename lock.
            let renamed = target_dir.join("temporarily-renamed.txt");
            std::fs::rename(&target, &renamed).unwrap();
            std::fs::rename(&renamed, &target).unwrap();
            let metadata = std::fs::metadata(&target).unwrap();
            assert!(metadata.permissions().readonly());
            assert_eq!(metadata.modified().unwrap(), modified);
            assert_eq!(
                std::fs::read(target_dir.join("source.txt:extra")).unwrap(),
                b"readonly stream"
            );
            crate::commands::ops::undo_last_action(&history)
                .await
                .unwrap();
            assert!(!target.exists());
            assert!(std::fs::metadata(&source).unwrap().permissions().readonly());
            crate::commands::ops::redo_last_action(&history)
                .await
                .unwrap();
        }
        crate::commands::ops::undo_last_action(&history)
            .await
            .unwrap();
        assert_eq!(std::fs::read_dir(&target_dir).unwrap().count(), 0);
    });
}
#[test]
fn overwrite_only_copy_is_recorded_as_non_undoable_history_entry() {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        let temp = tempfile::tempdir().unwrap();
        let source_dir = temp.path().join("source");
        let target_dir = temp.path().join("target");
        std::fs::create_dir(&source_dir).unwrap();
        std::fs::create_dir(&target_dir).unwrap();
        let source = source_dir.join("item.txt");
        let destination = target_dir.join("item.txt");
        std::fs::write(&source, b"new version").unwrap();
        std::fs::write(&destination, b"old version").unwrap();
        let resolutions = HashMap::from([(
            source.to_string_lossy().into_owned(),
            ConflictAction::Replace,
        )]);
        let history = Arc::new(HistoryManager::new());
        let manager = FileOperationManager::new();
        let id = manager.queue_operation_on(
            &tokio::runtime::Handle::current(),
            history.clone(),
            FileOperation::new(
                FileOpType::Copy,
                vec![source],
                Some(target_dir),
                Some(resolutions),
            ),
        );
        tokio::time::timeout(std::time::Duration::from_secs(5), async {
            loop {
                let status = manager.get_operation(&id).unwrap().status;
                if FileOperationManager::is_final(&status) {
                    assert_eq!(status, OpStatus::Completed);
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();

        let undo_stack = history.get_state().undo_stack;
        assert_eq!(undo_stack.len(), 1);
        assert!(undo_stack[0].undo_blocked_by_overwrite());
        assert_eq!(std::fs::read(&destination).unwrap(), b"new version");
        assert!(crate::commands::ops::undo_last_action(&history)
            .await
            .is_err());
        assert_eq!(history.get_state().undo_stack.len(), 1);
    });
}

#[test]
fn undo_mixed_copy_with_overwrite_is_blocked_as_a_whole() {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        let temp = tempfile::tempdir().unwrap();
        let source_dir = temp.path().join("source");
        let target_dir = temp.path().join("target");
        std::fs::create_dir(&source_dir).unwrap();
        std::fs::create_dir(&target_dir).unwrap();
        let existing_source = source_dir.join("existing.txt");
        let new_source = source_dir.join("new.txt");
        let existing_target = target_dir.join("existing.txt");
        let new_target = target_dir.join("new.txt");
        std::fs::write(&existing_source, b"new version").unwrap();
        std::fs::write(&new_source, b"new file").unwrap();
        std::fs::write(&existing_target, b"old version").unwrap();
        let resolutions = HashMap::from([(
            existing_source.to_string_lossy().into_owned(),
            ConflictAction::Replace,
        )]);
        let history = Arc::new(HistoryManager::new());
        let manager = FileOperationManager::new();
        let id = manager.queue_operation_on(
            &tokio::runtime::Handle::current(),
            history.clone(),
            FileOperation::new(
                FileOpType::Copy,
                vec![existing_source, new_source],
                Some(target_dir.clone()),
                Some(resolutions),
            ),
        );
        tokio::time::timeout(std::time::Duration::from_secs(5), async {
            loop {
                let status = manager.get_operation(&id).unwrap().status;
                if FileOperationManager::is_final(&status) {
                    assert_eq!(status, OpStatus::Completed);
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
        assert_eq!(history.get_state().undo_stack.len(), 1);
        assert!(crate::commands::ops::undo_last_action(&history)
            .await
            .is_err());
        assert_eq!(std::fs::read(&existing_target).unwrap(), b"new version");
        assert_eq!(std::fs::read(&new_target).unwrap(), b"new file");
        assert_eq!(std::fs::read_dir(&target_dir).unwrap().count(), 2);
        assert_eq!(history.get_state().undo_stack.len(), 1);
    });
}
