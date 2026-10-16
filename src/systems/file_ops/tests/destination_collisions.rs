use super::*;

fn assert_no_effects(op: &Arc<Mutex<FileOperation>>) {
    let op = op.lock().unwrap();
    assert!(op.created_files.is_empty() && op.created_dirs.is_empty());
    assert!(op.moved_files.is_empty() && op.moved_dirs.is_empty());
    assert!(op.copied_files.is_empty() && op.overwritten_files.is_empty());
    assert_eq!(op.processed_files, 0);
}

#[test]
fn colliding_file_destinations_preserve_all_sources_and_existing_outputs() {
    for is_move in [false, true] {
        for existing in [false, true] {
            for reverse in [false, true] {
                for turbo in [false, true] {
                    let root = tempfile::tempdir().unwrap();
                    for name in ["a", "b", "target"] {
                        std::fs::create_dir(root.path().join(name)).unwrap();
                    }
                    let a = root.path().join("a/report.txt");
                    let b = root.path().join("b/report.txt");
                    let target = root.path().join("target");
                    std::fs::write(&a, b"source A unique").unwrap();
                    std::fs::write(&b, b"source B unique").unwrap();
                    if existing {
                        std::fs::write(target.join("report.txt"), b"old destination").unwrap();
                    }
                    let mut sources = vec![a.clone(), b.clone()];
                    if reverse {
                        sources.reverse();
                    }
                    let resolutions = sources
                        .iter()
                        .map(|source| {
                            (
                                source.to_string_lossy().into_owned(),
                                ConflictAction::Replace,
                            )
                        })
                        .collect();
                    let mut operation = FileOperation::new(
                        if is_move {
                            FileOpType::Move
                        } else {
                            FileOpType::Copy
                        },
                        sources.clone(),
                        Some(target.clone()),
                        None,
                    );
                    operation.set_turbo(turbo);
                    let op = Arc::new(Mutex::new(operation));
                    let error = FileOperationManager::perform_copy(
                        &op,
                        sources,
                        Some(target.clone()),
                        is_move,
                        Some(resolutions),
                    )
                    .unwrap_err();
                    assert!(error.contains("Transfer destination collision"), "{error}");
                    assert_eq!(std::fs::read(&a).unwrap(), b"source A unique");
                    assert_eq!(std::fs::read(&b).unwrap(), b"source B unique");
                    if existing {
                        assert_eq!(
                            std::fs::read(target.join("report.txt")).unwrap(),
                            b"old destination"
                        );
                    }
                    assert_eq!(
                        std::fs::read_dir(&target).unwrap().count(),
                        usize::from(existing)
                    );
                    assert_no_effects(&op);
                }
            }
        }
    }
}

#[test]
fn nested_collisions_are_refused_before_unrelated_directory_creation() {
    let root = tempfile::tempdir().unwrap();
    let first = root.path().join("a/folder");
    let second = root.path().join("b/folder");
    let target = root.path().join("target");
    for path in [
        first.join("nested"),
        first.join("new-empty"),
        second.join("nested"),
        target.join("folder/nested"),
    ] {
        std::fs::create_dir_all(path).unwrap();
    }
    std::fs::write(first.join("nested/report.txt"), b"first").unwrap();
    std::fs::write(second.join("nested/report.txt"), b"second").unwrap();
    let output = target.join("folder/nested/report.txt");
    std::fs::write(&output, b"existing").unwrap();
    let resolutions = [first.clone(), second.clone()]
        .into_iter()
        .map(|path| (path.to_string_lossy().into_owned(), ConflictAction::Replace))
        .collect();
    let op = operation();
    let error = FileOperationManager::perform_copy(
        &op,
        vec![first.clone(), second.clone()],
        Some(target.clone()),
        true,
        Some(resolutions),
    )
    .unwrap_err();
    assert!(error.contains("Transfer destination collision"), "{error}");
    assert_eq!(
        std::fs::read(first.join("nested/report.txt")).unwrap(),
        b"first"
    );
    assert_eq!(
        std::fs::read(second.join("nested/report.txt")).unwrap(),
        b"second"
    );
    assert_eq!(std::fs::read(output).unwrap(), b"existing");
    assert!(!target.join("folder/new-empty").exists());
    assert_no_effects(&op);
}

#[test]
fn distinct_files_in_merged_directories_are_still_transferred() {
    for is_move in [false, true] {
        let root = tempfile::tempdir().unwrap();
        let first = root.path().join("a/folder");
        let second = root.path().join("b/folder");
        let target = root.path().join("target");
        for path in [
            first.join("nested/empty"),
            second.join("nested"),
            target.clone(),
        ] {
            std::fs::create_dir_all(path).unwrap();
        }
        std::fs::write(first.join("nested/first.txt"), b"first").unwrap();
        std::fs::write(second.join("nested/second.txt"), b"second").unwrap();
        let op = operation();
        FileOperationManager::perform_copy(
            &op,
            vec![first.clone(), second.clone()],
            Some(target.clone()),
            is_move,
            None,
        )
        .unwrap();
        assert_eq!(
            std::fs::read(target.join("folder/nested/first.txt")).unwrap(),
            b"first"
        );
        assert_eq!(
            std::fs::read(target.join("folder/nested/second.txt")).unwrap(),
            b"second"
        );
        assert!(target.join("folder/nested/empty").is_dir());
        assert_eq!(first.exists(), !is_move);
        assert_eq!(second.exists(), !is_move);
    }
}

#[test]
fn explicit_skip_does_not_reserve_an_output_against_the_other_source() {
    for is_move in [false, true] {
        let root = tempfile::tempdir().unwrap();
        for name in ["a", "b", "target"] {
            std::fs::create_dir(root.path().join(name)).unwrap();
        }
        let first = root.path().join("a/report.txt");
        let second = root.path().join("b/report.txt");
        let target = root.path().join("target");
        std::fs::write(&first, b"skip").unwrap();
        std::fs::write(&second, b"transfer").unwrap();
        std::fs::write(target.join("report.txt"), b"old").unwrap();
        let resolutions = HashMap::from([
            (first.to_string_lossy().into_owned(), ConflictAction::Skip),
            (
                second.to_string_lossy().into_owned(),
                ConflictAction::Replace,
            ),
        ]);
        FileOperationManager::perform_copy(
            &operation(),
            vec![first.clone(), second.clone()],
            Some(target.clone()),
            is_move,
            Some(resolutions),
        )
        .unwrap();
        assert_eq!(std::fs::read(&first).unwrap(), b"skip");
        assert_eq!(
            std::fs::read(target.join("report.txt")).unwrap(),
            b"transfer"
        );
        assert_eq!(second.exists(), !is_move);
    }
}

#[test]
fn a_planned_file_cannot_also_be_a_directory_or_a_parent() {
    for is_move in [false, true] {
        for reverse in [false, true] {
            let root = tempfile::tempdir().unwrap();
            let file = root.path().join("a/shared");
            let directory = root.path().join("b/shared");
            let target = root.path().join("target");
            std::fs::create_dir(root.path().join("a")).unwrap();
            std::fs::create_dir_all(&directory).unwrap();
            std::fs::create_dir(&target).unwrap();
            std::fs::write(&file, b"file").unwrap();
            std::fs::write(directory.join("child"), b"child").unwrap();
            let mut sources = vec![file.clone(), directory.clone()];
            if reverse {
                sources.reverse();
            }
            let op = operation();
            let error = FileOperationManager::perform_copy(
                &op,
                sources,
                Some(target.clone()),
                is_move,
                None,
            )
            .unwrap_err();
            assert!(error.contains("Transfer destination collision"), "{error}");
            assert_eq!(std::fs::read(&file).unwrap(), b"file");
            assert_eq!(std::fs::read(directory.join("child")).unwrap(), b"child");
            assert_eq!(std::fs::read_dir(target).unwrap().count(), 0);
            assert_no_effects(&op);
        }
    }
}

#[cfg(target_os = "windows")]
#[test]
fn windows_case_aliases_are_rejected_for_new_and_existing_destinations() {
    for (left, right) in [("Report.txt", "report.txt"), ("École.txt", "école.txt")] {
        for existing in [false, true] {
            let root = tempfile::tempdir().unwrap();
            for name in ["a", "b", "target"] {
                std::fs::create_dir(root.path().join(name)).unwrap();
            }
            let first = root.path().join("a").join(left);
            let second = root.path().join("b").join(right);
            let target = root.path().join("target");
            std::fs::write(&first, b"first").unwrap();
            std::fs::write(&second, b"second").unwrap();
            if existing {
                std::fs::write(target.join(left), b"old").unwrap();
            }
            let sources = vec![first.clone(), second.clone()];
            let resolutions = sources
                .iter()
                .map(|path| (path.to_string_lossy().into_owned(), ConflictAction::Replace))
                .collect();
            let op = operation();
            let error = FileOperationManager::perform_copy(
                &op,
                sources,
                Some(target.clone()),
                true,
                Some(resolutions),
            )
            .unwrap_err();
            assert!(error.contains("Transfer destination collision"), "{error}");
            assert_eq!(std::fs::read(&first).unwrap(), b"first");
            assert_eq!(std::fs::read(&second).unwrap(), b"second");
            assert_eq!(
                std::fs::read_dir(&target).unwrap().count(),
                usize::from(existing)
            );
            if existing {
                assert_eq!(std::fs::read(target.join(left)).unwrap(), b"old");
            }
            assert_no_effects(&op);
        }
    }
}

#[test]
fn rejected_transfer_never_adds_an_undo_entry() {
    let root = tempfile::tempdir().unwrap();
    for name in ["a", "b", "target"] {
        std::fs::create_dir(root.path().join(name)).unwrap();
    }
    let sources: Vec<_> = ["a/report.txt", "b/report.txt"]
        .into_iter()
        .map(|name| root.path().join(name))
        .collect();
    for (index, source) in sources.iter().enumerate() {
        std::fs::write(source, index.to_string()).unwrap();
    }
    let target = root.path().join("target");
    std::fs::write(target.join("report.txt"), b"old").unwrap();
    let resolutions = sources
        .iter()
        .map(|path| (path.to_string_lossy().into_owned(), ConflictAction::Replace))
        .collect();
    let op = Arc::new(Mutex::new(FileOperation::new(
        FileOpType::Move,
        sources.clone(),
        Some(target.clone()),
        Some(resolutions),
    )));
    let history = Arc::new(HistoryManager::new());
    let runtime = tokio::runtime::Runtime::new().unwrap();
    runtime.block_on(FileOperationManager::execute_operation(
        history.clone(),
        op.clone(),
    ));
    assert!(
        matches!(&op.lock().unwrap().status, OpStatus::Error(error) if error.contains("Transfer destination collision"))
    );
    let state = history.get_state();
    assert!(state.undo_stack.is_empty() && state.redo_stack.is_empty());
    for (index, source) in sources.iter().enumerate() {
        assert_eq!(std::fs::read_to_string(source).unwrap(), index.to_string());
    }
    assert_eq!(std::fs::read(target.join("report.txt")).unwrap(), b"old");
    assert_no_effects(&op);
}
