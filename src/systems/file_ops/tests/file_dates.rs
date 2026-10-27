use super::*;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::time::{Duration, SystemTime};

fn transfer(kind: FileOpType, source: &Path, target: &Path) -> Arc<HistoryManager> {
    let history = Arc::new(HistoryManager::new());
    let op = Arc::new(Mutex::new(FileOperation::new(
        kind,
        vec![source.to_path_buf()],
        Some(target.to_path_buf()),
        None,
    )));
    let runtime = tokio::runtime::Builder::new_multi_thread().enable_all().build().unwrap();
    runtime.block_on(FileOperationManager::execute_operation(history.clone(), op.clone()));
    assert_eq!(op.lock().unwrap().status, OpStatus::Completed);
    history
}

/// A file last read and written long ago, with a private mode.
fn dated_file(folder: &Path) -> (PathBuf, SystemTime, SystemTime) {
    let source = folder.join("dated.txt");
    std::fs::write(&source, b"dated").unwrap();
    std::fs::set_permissions(&source, std::fs::Permissions::from_mode(0o600)).unwrap();
    let accessed = SystemTime::UNIX_EPOCH + Duration::from_secs(1_500_000_000);
    let modified = SystemTime::UNIX_EPOCH + Duration::from_secs(1_400_000_000);
    std::fs::File::options()
        .write(true)
        .open(&source)
        .unwrap()
        .set_times(std::fs::FileTimes::new().set_accessed(accessed).set_modified(modified))
        .unwrap();
    (source, accessed, modified)
}

fn assert_dated(path: &Path, accessed: SystemTime, modified: SystemTime) {
    let metadata = std::fs::metadata(path).unwrap();
    assert_eq!(std::fs::read(path).unwrap(), b"dated");
    assert_eq!(metadata.permissions().mode() & 0o7777, 0o600);
    assert_eq!(metadata.modified().unwrap(), modified);
    assert_eq!(metadata.accessed().unwrap(), accessed);
}

#[::core::prelude::v1::test]
fn a_copy_keeps_mode_and_dates_through_its_verification() {
    let root = tempfile::tempdir().unwrap();
    let (source, accessed, modified) = dated_file(root.path());
    let target = root.path().join("target");
    std::fs::create_dir(&target).unwrap();
    transfer(FileOpType::Copy, &source, &target);
    assert_dated(&target.join("dated.txt"), accessed, modified);
}

#[::core::prelude::v1::test]
fn a_move_to_another_volume_keeps_mode_and_dates() {
    use std::os::unix::fs::MetadataExt;
    let Ok(other) = tempfile::tempdir_in("/dev/shm") else { return };
    let root = tempfile::tempdir().unwrap();
    if std::fs::metadata(root.path()).unwrap().dev() == std::fs::metadata(other.path()).unwrap().dev() {
        return;
    }
    let (source, accessed, modified) = dated_file(root.path());
    transfer(FileOpType::Move, &source, other.path());
    assert!(source.symlink_metadata().is_err());
    assert_dated(&other.path().join("dated.txt"), accessed, modified);
}
