use super::*;

#[test]
fn native_batch_move_undo_and_redo_do_not_read_file_contents() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source");
    let target = root.path().join("target");
    std::fs::create_dir(&source).unwrap();
    std::fs::create_dir(&target).unwrap();
    let paths: Vec<_> = (0..17).map(|index| source.join(format!("file-{index}.bin"))).collect();
    for path in &paths { std::fs::write(path, vec![0x5a; 2 * 1024 * 1024]).unwrap(); }
    let history = Arc::new(HistoryManager::new());
    let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
    runtime.block_on(async {
        let op = Arc::new(Mutex::new(FileOperation::new(FileOpType::Move, paths.clone(), Some(target.clone()), None)));
        FileOperationManager::execute_operation(history.clone(), op.clone()).await;
        assert_eq!(op.lock().unwrap().status, OpStatus::Completed);
        for _ in 0..2 {
            crate::systems::history_move::HASH_BYTES.with(|bytes| bytes.set(0));
            let started = std::time::Instant::now();
            crate::commands::ops::undo_last_action(&history).await.unwrap();
            eprintln!("17-file native undo: {:?}", started.elapsed());
            crate::systems::history_move::HASH_BYTES.with(|bytes| assert_eq!(bytes.get(), 0, "Undo must not hash native moved files"));
            for path in &paths { assert!(path.exists()); assert!(!target.join(path.file_name().unwrap()).exists()); }
            crate::commands::ops::redo_last_action(&history).await.unwrap();
            crate::systems::history_move::HASH_BYTES.with(|bytes| assert_eq!(bytes.get(), 0, "Redo must not hash native moved files"));
            for path in &paths { assert!(!path.exists()); assert!(target.join(path.file_name().unwrap()).exists()); }
        }
    });
}

#[test]
fn successive_archive_file_moves_roundtrip_history() {
    let root = tempfile::tempdir().unwrap();
    let folders: Vec<_> = ["Tests", "1", "2"].into_iter().map(|name| root.path().join(name)).collect();
    for folder in &folders { std::fs::create_dir(folder).unwrap(); }
    let name = "3JH40xKM35P_B2-29272-0030.7z";
    std::fs::write(folders[0].join(name), b"ordinary archive file fixture").unwrap();
    let history = Arc::new(HistoryManager::new());
    let runtime = tokio::runtime::Builder::new_multi_thread().enable_all().build().unwrap();
    runtime.block_on(async {
        for step in 0..2 {
            let op = Arc::new(Mutex::new(FileOperation::new(FileOpType::Move,
                vec![folders[step].join(name)], Some(folders[step + 1].clone()), None)));
            FileOperationManager::execute_operation(history.clone(), op.clone()).await;
            assert_eq!(op.lock().unwrap().status, OpStatus::Completed);
            assert!(!folders[step].join(name).exists());
            assert!(folders[step + 1].join(name).exists());
        }
        for _ in 0..2 {
            for step in (0..2).rev() {
                crate::commands::ops::undo_last_action(&history).await.unwrap();
                assert!(folders[step].join(name).exists());
                assert!(!folders[step + 1].join(name).exists());
                assert_eq!(history.get_state().undo_stack.len(), step);
            }
            for step in 0..2 {
                crate::commands::ops::redo_last_action(&history).await.unwrap();
                assert!(!folders[step].join(name).exists());
                assert!(folders[step + 1].join(name).exists());
            }
        }
    });
}

#[test]
#[cfg(target_os = "windows")]
fn directory_move_cleans_initial_directories_and_journals_empty_descendants() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source");
    let target = root.path().join("target");
    std::fs::create_dir_all(source.join("nested/empty")).unwrap();
    std::fs::create_dir(&target).unwrap();
    std::fs::write(source.join("nested/data"), b"moved data").unwrap();
    let op = operation();
    FileOperationManager::perform_copy(&op, vec![source.clone()], Some(target.clone()), true, None).unwrap();
    assert!(!source.exists());
    assert_eq!(std::fs::read(target.join("source/nested/data")).unwrap(), b"moved data");
    assert!(target.join("source/nested/empty").is_dir());
    let moved = op.lock().unwrap().moved_dirs.clone();
    assert_eq!(moved.len(), 3);
    let resolved_target = std::fs::canonicalize(&target).unwrap();
    assert_eq!(moved[0], (source.join("nested").join("empty"), resolved_target.join("source/nested/empty")));
    assert_eq!(moved[2], (source, resolved_target.join("source")));
}

#[cfg(target_os = "windows")]
#[test]
fn move_directory_history_captures_native_proofs_and_preserves_replacements() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source-empty");
    let target_dir = root.path().join("target");
    std::fs::create_dir_all(source.join("nested-empty")).unwrap();
    std::fs::create_dir(&target_dir).unwrap();
    let target = target_dir.join("source-empty");
    let displaced = root.path().join("original-copy");
    let history = Arc::new(HistoryManager::new());
    let op = Arc::new(Mutex::new(FileOperation::new(
        FileOpType::Move,
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
        assert!(!source.exists());
        let state = history.get_state();
        let restored = state.undo_stack[0].details.moved_directory_identities.as_ref().unwrap();
        assert_eq!(restored.len(), 2);
        assert!(restored.iter().all(|(_, proof)| proof.snapshot().is_none()));
        let original = platform().hold_resolved_operation_directory(&std::fs::canonicalize(&target).unwrap()).unwrap();
        let identity = platform().file_identity(&original).unwrap();
        drop(original);
        std::fs::rename(&target, &displaced).unwrap();
        std::fs::create_dir(&target).unwrap();
        let replacement = platform().hold_resolved_operation_directory(&std::fs::canonicalize(&target).unwrap()).unwrap();
        assert_ne!(platform().file_identity(&replacement).unwrap(), identity);
        drop(replacement);
        let result = crate::commands::ops::undo_last_action(&history).await;
        assert!(matches!(result, Err(crate::models::CommandError::HistoryMove(failure)) if failure.issue == crate::models::error::HistoryMoveIssue::Changed), "Undo must preserve an independent empty replacement directory");
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
            assert!(source.join("nested-empty").is_dir());
            let state = history.get_state();
            for (path, proof) in state.redo_stack[0].details.moved_directory_identities.as_ref().unwrap() {
                let handle = platform().hold_resolved_operation_directory(&std::fs::canonicalize(path).unwrap()).unwrap();
                assert!(proof.snapshot().unwrap().matches(&handle).unwrap());
            }
            crate::commands::ops::redo_last_action(&history).await.unwrap();
            assert!(target.join("nested-empty").is_dir());
            assert!(!source.exists());
        }
        crate::commands::ops::undo_last_action(&history).await.unwrap();
        assert_eq!(std::fs::read_dir(&target_dir).unwrap().count(), 0);
    });
}

#[test]
fn undo_replacing_move_restores_source_without_a_backup() {
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
        let source = source_dir.join("file.txt");
        let target = target_dir.join("file.txt");
        std::fs::write(&source, b"moving file").unwrap();
        std::fs::write(&target, b"old target").unwrap();
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
                FileOpType::Move,
                vec![source.clone()],
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
        assert!(!source.exists());
        assert_eq!(std::fs::read(&target).unwrap(), b"moving file");
        crate::commands::ops::undo_last_action(&history)
            .await
            .unwrap();
        assert_eq!(std::fs::read(&source).unwrap(), b"moving file");
        assert!(!target.exists());
        assert_eq!(std::fs::read_dir(&target_dir).unwrap().count(), 0);
    });
}

#[cfg(target_os = "windows")]
#[test]
#[ignore = "requires OXYDE_TEST_SECOND_VOLUME; moves only generated fixtures between two volumes"]
fn native_cross_volume_move_history_roundtrip() {
    let second_root = PathBuf::from(
        std::env::var_os("OXYDE_TEST_SECOND_VOLUME")
            .expect("set OXYDE_TEST_SECOND_VOLUME to a writable second volume"),
    );
    assert!(second_root.is_absolute() && second_root.is_dir());
    let first = tempfile::tempdir().unwrap();
    let second = tempfile::Builder::new()
        .prefix("oxyde-a19-")
        .tempdir_in(second_root)
        .unwrap();
    let probe = first.path().join("volume-probe");
    std::fs::write(&probe, b"probe").unwrap();
    assert_eq!(
        std::fs::rename(&probe, second.path().join("volume-probe"))
            .unwrap_err()
            .kind(),
        std::io::ErrorKind::CrossesDevices
    );
    std::fs::remove_file(probe).unwrap();

    let source = first.path().join("Données 日本語 🦀");
    std::fs::create_dir_all(source.join("nested/empty")).unwrap();
    std::fs::write(source.join("nested/file.txt"), b"nested fixture").unwrap();
    std::fs::write(source.join("nested/file.txt:extra"), b"initial move stream").unwrap();
    let modified = std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_700_000_000);
    std::fs::File::options()
        .write(true)
        .open(source.join("nested/file.txt"))
        .unwrap()
        .set_times(std::fs::FileTimes::new().set_modified(modified))
        .unwrap();
    std::fs::write(
        source.join("shortcut.url"),
        b"[InternetShortcut]\nURL=https://example.org\n",
    )
    .unwrap();
    let destination = second.path().join(source.file_name().unwrap());
    let history = Arc::new(HistoryManager::new());
    let manager = FileOperationManager::new();
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        let id = manager.queue_operation_on(
            &tokio::runtime::Handle::current(),
            history.clone(),
            FileOperation::new(
                FileOpType::Move,
                vec![source.clone()],
                Some(second.path().into()),
                None,
            ),
        );
        tokio::time::timeout(std::time::Duration::from_secs(15), async {
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
        assert!(!source.exists());
        assert_eq!(
            std::fs::read(destination.join("nested/file.txt:extra")).unwrap(),
            b"initial move stream"
        );
        assert_eq!(
            std::fs::metadata(destination.join("nested/file.txt"))
                .unwrap()
                .modified()
                .unwrap(),
            modified
        );
        // A27: the files have already crossed volumes when this original
        // empty directory is recreated. Its replacement by a foreign file
        // must roll back the whole Undo and keep the history retryable.
        let blocker = source.join("nested/empty");
        std::fs::create_dir_all(blocker.parent().unwrap()).unwrap();
        std::fs::write(&blocker, b"foreign directory blocker").unwrap();
        let error = crate::commands::ops::undo_last_action(&history)
            .await
            .unwrap_err();
        assert!(error.to_string().contains("Not a directory"), "{error}");
        assert_eq!(history.get_state().undo_stack.len(), 1);
        assert!(history.get_state().redo_stack.is_empty());
        assert_eq!(
            std::fs::read(&blocker).unwrap(),
            b"foreign directory blocker"
        );
        assert!(!source.join("nested/file.txt").exists());
        assert!(!source.join("shortcut.url").exists());
        assert_eq!(
            std::fs::read(destination.join("nested/file.txt")).unwrap(),
            b"nested fixture"
        );
        assert!(destination.join("nested/empty").is_dir());
        std::fs::remove_file(blocker).unwrap();
        for cycle in 0..2 {
            crate::commands::ops::undo_last_action(&history)
                .await
                .unwrap();
            assert_eq!(
                std::fs::read(source.join("nested/file.txt")).unwrap(),
                b"nested fixture"
            );
            assert!(source.join("nested/empty").is_dir());
            assert_eq!(
                std::fs::read(source.join("nested/file.txt:extra")).unwrap(),
                b"initial move stream"
            );
            assert_eq!(
                std::fs::metadata(source.join("nested/file.txt"))
                    .unwrap()
                    .modified()
                    .unwrap(),
                modified
            );
            assert!(source.join("shortcut.url").is_file());
            assert!(!destination.exists());
            if cycle == 0 {
                // Preflight can read this file, but the second move cannot
                // acquire its exclusive handle. The first moved file must
                // roll back across volumes before history can be retried.
                let state = history.get_state();
                let moved = state
                    .redo_stack
                    .last()
                    .unwrap()
                    .details
                    .moved_files
                    .as_ref()
                    .unwrap();
                assert!(moved.len() >= 2);
                let writer = std::fs::File::options()
                    .write(true)
                    .open(&moved[1].0)
                    .unwrap();
                assert!(crate::commands::ops::redo_last_action(&history)
                    .await
                    .is_err());
                assert_eq!(history.get_state().redo_stack.len(), 1);
                assert!(history.get_state().undo_stack.is_empty());
                for (original, target) in moved {
                    assert!(std::path::Path::new(original).is_file());
                    assert!(!std::path::Path::new(target).exists());
                }
                assert!(!destination.exists());
                drop(writer);
            }
            crate::commands::ops::redo_last_action(&history)
                .await
                .unwrap();
            assert!(!source.exists());
            assert_eq!(
                std::fs::read(destination.join("nested/file.txt")).unwrap(),
                b"nested fixture"
            );
            assert!(destination.join("nested/empty").is_dir());
            assert_eq!(
                std::fs::read(destination.join("nested/file.txt:extra")).unwrap(),
                b"initial move stream"
            );
            assert_eq!(
                std::fs::metadata(destination.join("nested/file.txt"))
                    .unwrap()
                    .modified()
                    .unwrap(),
                modified
            );
            assert!(destination.join("shortcut.url").is_file());
        }
    });
}

#[cfg(target_os = "windows")]
#[test]
#[ignore = "requires OXYDE_TEST_SECOND_VOLUME; checks partial initial Move using generated fixtures"]
fn native_cross_volume_refused_source_delete_can_be_undone() {
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
    let second_root = PathBuf::from(
        std::env::var_os("OXYDE_TEST_SECOND_VOLUME").expect("set a writable second volume"),
    );
    assert!(second_root.is_absolute() && second_root.is_dir());
    let first = tempfile::tempdir().unwrap();
    let second = tempfile::Builder::new()
        .prefix("oxyde-a31-")
        .tempdir_in(second_root)
        .unwrap();
    let regular = first.path().join("regular.txt");
    let readonly = first.path().join("readonly.txt");
    let target = second.path().join("readonly.txt");
    let _cleanup = WritablePaths(vec![readonly.clone(), target.clone()]);
    std::fs::write(&regular, b"regular move").unwrap();
    std::fs::write(&readonly, b"readonly move").unwrap();
    assert_eq!(
        std::fs::rename(&regular, second.path().join("probe"))
            .unwrap_err()
            .kind(),
        std::io::ErrorKind::CrossesDevices
    );
    let mut permissions = std::fs::metadata(&readonly).unwrap().permissions();
    permissions.set_readonly(true);
    std::fs::set_permissions(&readonly, permissions).unwrap();
    let history = Arc::new(HistoryManager::new());
    let op = Arc::new(Mutex::new(FileOperation::new(
        FileOpType::Move,
        vec![regular.clone(), readonly.clone()],
        Some(second.path().into()),
        None,
    )));
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        FileOperationManager::execute_operation(history.clone(), op.clone()).await;
        let state = op.lock().unwrap().clone();
        assert!(matches!(state.status, OpStatus::Error(ref message) if message.contains("cannot remove")), "{:?}", state.status);
        assert_eq!(state.moved_files.len(), 1);
        assert!(!regular.exists());
        assert_eq!(std::fs::read(&readonly).unwrap(), b"readonly move");
        assert_eq!(std::fs::read(&target).unwrap(), b"readonly move");
        assert!(std::fs::metadata(&readonly).unwrap().permissions().readonly());
        assert_eq!(history.get_state().undo_stack.len(), 1);
        crate::commands::ops::undo_last_action(&history).await.unwrap();
        assert_eq!(std::fs::read(&regular).unwrap(), b"regular move");
        assert_eq!(std::fs::read(&readonly).unwrap(), b"readonly move");
        assert!(std::fs::metadata(&readonly).unwrap().permissions().readonly());
        assert_eq!(std::fs::read_dir(second.path()).unwrap().count(), 0);
        assert_eq!(history.get_state().redo_stack.len(), 1);
    });
}
