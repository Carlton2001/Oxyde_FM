use super::*;

fn operation() -> FileOperation {
    FileOperation::new(
        FileOpType::Copy,
        vec!["first".into(), "second".into(), "third".into()],
        None,
        None,
    )
}

#[test]
fn cancellation_keeps_proven_success_separate_from_partial_and_unstarted() {
    let mut op = operation();
    for path in ["first", "second", "third"] {
        op.item_plan(
            Path::new(path),
            PathBuf::from(format!("target/{path}")),
            1,
            false,
            false,
        );
    }
    op.item_done(Path::new("second"));
    op.item_started(Path::new("first"));
    op.status = OpStatus::Cancelled;
    let outcomes = op.snapshot_item_outcomes();
    assert_eq!(outcomes[Path::new("first")], ItemOutcome::Interrupted);
    assert_eq!(outcomes[Path::new("second")], ItemOutcome::Completed);
    assert_eq!(outcomes[Path::new("third")], ItemOutcome::NotProcessed);
}

#[test]
fn directory_with_unfinished_or_skipped_descendants_is_not_completed() {
    let mut op = operation();
    op.item_plan(Path::new("first"), "target/first".into(), 3, false, false);
    op.item_directory(Path::new("target/first"), true);
    op.item_done(Path::new("first/a"));
    op.status = OpStatus::Cancelled;
    assert_eq!(
        op.snapshot_item_outcomes()[Path::new("first")],
        ItemOutcome::Partial
    );
    op.item_done(Path::new("first/b"));
    assert_eq!(
        op.snapshot_item_outcomes()[Path::new("first")],
        ItemOutcome::Completed
    );
    op.item_skipped(Path::new("first/c"));
    assert_eq!(
        op.snapshot_item_outcomes()[Path::new("first")],
        ItemOutcome::Partial
    );
}

#[test]
fn moved_directory_requires_source_removal_and_conflict_skips_stay_distinct() {
    let mut op = operation();
    op.op_type = FileOpType::Move;
    op.item_plan(Path::new("first"), "target/first".into(), 1, false, true);
    op.item_directory_created(Path::new("target/first"));
    op.item_directory(Path::new("target/first"), true);
    op.item_skipped(Path::new("second"));
    op.status = OpStatus::Cancelled;
    assert_eq!(
        op.snapshot_item_outcomes()[Path::new("first")],
        ItemOutcome::Partial
    );
    assert_eq!(
        op.snapshot_item_outcomes()[Path::new("second")],
        ItemOutcome::Skipped
    );
    op.item_directory_removed(Path::new("first"));
    assert_eq!(
        op.snapshot_item_outcomes()[Path::new("first")],
        ItemOutcome::Completed
    );
}

#[test]
fn legacy_records_without_receipts_do_not_invent_successes() {
    let mut op = operation();
    op.item_progress.clear();
    op.status = OpStatus::Cancelled;
    op.processed_files = 2;
    assert!(op
        .snapshot_item_outcomes()
        .values()
        .all(|outcome| *outcome == ItemOutcome::Unknown));
}

#[test]
fn real_copy_records_files_directories_and_conflict_skips() {
    use std::sync::{Arc, Mutex};
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("source");
    let target = temp.path().join("target");
    std::fs::create_dir(&source).unwrap();
    std::fs::create_dir(&target).unwrap();
    let file = source.join("file.txt");
    let skipped = source.join("skipped.txt");
    let folder = source.join("folder");
    std::fs::write(&file, b"copied").unwrap();
    std::fs::write(&skipped, b"skip this").unwrap();
    std::fs::write(target.join("skipped.txt"), b"keep this").unwrap();
    std::fs::create_dir_all(folder.join("empty")).unwrap();
    std::fs::write(folder.join("child.txt"), b"child").unwrap();
    let sources = vec![file.clone(), skipped.clone(), folder.clone()];
    let op = Arc::new(Mutex::new(FileOperation::new(
        FileOpType::Copy,
        sources.clone(),
        Some(target.clone()),
        None,
    )));
    FileOperationManager::perform_copy(&op, sources, Some(target.clone()), false, None).unwrap();
    let mut op = op.lock().unwrap();
    op.status = OpStatus::Cancelled;
    let result = op.snapshot_item_outcomes();
    assert_eq!(result[&file], ItemOutcome::Completed);
    assert_eq!(result[&folder], ItemOutcome::Completed);
    assert_eq!(result[&skipped], ItemOutcome::Skipped);
    assert_eq!(
        std::fs::read(target.join("skipped.txt")).unwrap(),
        b"keep this"
    );
}

#[test]
fn real_move_records_directory_completion_after_removal() {
    use std::sync::{Arc, Mutex};
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("folder");
    let target = temp.path().join("target");
    std::fs::create_dir(&source).unwrap();
    std::fs::create_dir(&target).unwrap();
    std::fs::write(source.join("child.txt"), b"child").unwrap();
    let op = Arc::new(Mutex::new(FileOperation::new(
        FileOpType::Move,
        vec![source.clone()],
        Some(target.clone()),
        None,
    )));
    FileOperationManager::perform_copy(&op, vec![source.clone()], Some(target.clone()), true, None)
        .unwrap();
    let mut op = op.lock().unwrap();
    op.status = OpStatus::Cancelled;
    assert!(!source.exists());
    assert!(target.join("folder/child.txt").exists());
    assert_eq!(op.snapshot_item_outcomes()[&source], ItemOutcome::Completed);
}
