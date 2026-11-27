use super::super::{FileOpType, FileOperation, FileOperationManager, OpStatus};
use crate::models::ConflictAction;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

fn failed_copy(sources: &[&str]) -> FileOperation {
    let mut resolutions = HashMap::new();
    resolutions.insert("first".to_string(), ConflictAction::Replace);
    resolutions.insert("second/inner".to_string(), ConflictAction::Skip);
    let mut op = FileOperation::new(
        FileOpType::Copy,
        sources.iter().map(PathBuf::from).collect(),
        Some("target".into()),
        Some(resolutions),
    );
    op.copy_name_suffix = "Copy".into();
    op.compression_quality = "best".into();
    op.set_turbo(true);
    op.execution_started = true;
    op
}

#[test]
fn retry_takes_only_untouched_sources_with_their_decisions_and_settings() {
    let mut op = failed_copy(&["first", "second", "third"]);
    op.item_plan(Path::new("first"), "target/first".into(), 1, false, false);
    op.item_done(Path::new("first"));
    // An extraction or a compression renames its destination; the retry asks the original.
    op.destination = Some("target (1)".into());
    op.status = OpStatus::Error("password".into());

    let retry = op.untouched_retry().unwrap();
    assert_eq!(retry.sources, vec![PathBuf::from("second"), PathBuf::from("third")]);
    assert_eq!(retry.destination, Some(PathBuf::from("target")));
    let resolutions = retry.resolutions.unwrap();
    assert_eq!(resolutions.len(), 1);
    assert_eq!(resolutions["second/inner"], ConflictAction::Skip);
    assert_eq!(retry.copy_name_suffix, "Copy");
    assert_eq!(retry.compression_quality, "best");
    assert!(retry.turbo);
    assert_ne!(retry.id, op.id);
}

#[test]
fn retry_is_refused_while_a_source_may_hold_part_of_the_work() {
    let mut op = failed_copy(&["first", "second"]);
    op.item_plan(Path::new("first"), "target/first".into(), 2, false, false);
    op.item_done(Path::new("first/a"));
    op.status = OpStatus::Error("password".into());
    assert!(op.untouched_retry().is_none());

    let mut running = failed_copy(&["first"]);
    running.status = OpStatus::Running;
    assert!(running.untouched_retry().is_none());
}

#[test]
fn archive_transfer_is_retried_whole_only_before_it_wrote_anything() {
    let mut op = failed_copy(&["archive.zip/a", "archive.zip/b"]);
    op.item_tracking_atomic = true;
    op.status = OpStatus::Error("password".into());
    assert_eq!(op.untouched_retry().unwrap().sources.len(), 2);

    op.item_started(Path::new("archive.zip/a"));
    assert!(op.untouched_retry().is_none());
}

#[test]
fn deletion_in_several_archives_runs_again_for_the_archive_refused_only() {
    use crate::systems::archive_workspace::{mutation_operation, ArchiveWorkspace};

    let directory = tempfile::tempdir().unwrap();
    let content = tempfile::tempdir().unwrap();
    std::fs::write(content.path().join("keep.txt"), b"keep").unwrap();
    std::fs::write(content.path().join("remove.txt"), b"remove").unwrap();
    let done = directory.path().join("a.zip");
    FileOperationManager::perform_compress(
        &mutation_operation("normal"),
        vec![content.path().join("keep.txt"), content.path().join("remove.txt")],
        Some(done.clone()),
    )
    .unwrap();
    // Sorted after the first: it is reached once the first is rewritten.
    let refused = directory.path().join("b.zip");
    std::fs::write(&refused, b"not an archive").unwrap();

    let sources = vec![refused.join("remove.txt"), done.join("remove.txt")];
    let operation = Arc::new(Mutex::new(FileOperation::new(FileOpType::Delete, sources.clone(), None, None)));
    {
        let mut locked = operation.lock().unwrap();
        locked.item_tracking_atomic = true;
        locked.execution_started = true;
    }
    let error = FileOperationManager::perform_delete(&operation, sources).unwrap_err();
    operation.lock().unwrap().status = OpStatus::Error(error);

    let workspace = ArchiveWorkspace::open(done.clone()).unwrap();
    assert!(!workspace.root().join("remove.txt").exists());
    assert!(workspace.root().join("keep.txt").exists());
    drop(workspace);
    let retry = operation.lock().unwrap().untouched_retry().unwrap();
    assert_eq!(retry.sources, vec![refused.join("remove.txt")]);
}
