use super::*;
use std::sync::atomic::AtomicUsize;

#[test]
fn speed_graph_history_is_bounded_and_present_in_snapshots() {
    let mut operation = FileOperation::new(FileOpType::Copy, Vec::new(), None, None);
    for speed in 0..75 {
        operation.record_speed(speed);
    }
    let snapshot = operation.snapshot();
    assert_eq!(snapshot.speed_history.len(), SPEED_HISTORY_LIMIT);
    assert_eq!(snapshot.speed_history.first(), Some(&15));
    assert_eq!(snapshot.speed_history.last(), Some(&74));
}

#[test]
fn copy_and_archive_progress_feed_speed_graph() {
    let copy = Arc::new(Mutex::new(FileOperation::new(
        FileOpType::Copy,
        Vec::new(),
        None,
        None,
    )));
    let bytes = Arc::new(AtomicU64::new(2048));
    let files = Arc::new(AtomicUsize::new(1));
    let mut last_bytes = 0;
    let mut last_emit = std::time::Instant::now() - std::time::Duration::from_millis(600);
    let mut samples = std::collections::VecDeque::new();
    FileOperationManager::emit_progress(
        &copy,
        &bytes,
        &files,
        &mut last_bytes,
        &mut last_emit,
        &mut samples,
    );
    assert!(copy.lock().unwrap().speed_history[0] > 0);

    let archive = Arc::new(Mutex::new(FileOperation::new(
        FileOpType::Compress,
        Vec::new(),
        None,
        None,
    )));
    archive.lock().unwrap().speed_sample_at =
        Some(std::time::Instant::now() - std::time::Duration::from_millis(600));
    FileOperationManager::archive_progress(&archive, 2048);
    assert!(archive.lock().unwrap().speed_history[0] > 0);
}

#[test]
fn copy_uses_parallel_workers_only_for_many_small_files() {
    assert_eq!(FileOperationManager::copy_worker_count(0, 0, 8), 1);
    assert_eq!(
        FileOperationManager::copy_worker_count(3, 3 * 1024 * 1024, 8),
        1
    );
    assert_eq!(
        FileOperationManager::copy_worker_count(8, 8 * 1024 * 1024, 8),
        4
    );
    assert_eq!(
        FileOperationManager::copy_worker_count(8, 8 * 64 * 1024 * 1024, 8),
        1
    );
}
