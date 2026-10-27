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

fn set_dates(path: &Path, accessed: SystemTime, modified: SystemTime) {
    std::fs::File::open(path)
        .unwrap()
        .set_times(std::fs::FileTimes::new().set_accessed(accessed).set_modified(modified))
        .unwrap();
}

/// `private` (700) holding `locked` (`locked_mode`), each with its own dates.
fn dated_folders(root: &Path, locked_mode: u32) -> (PathBuf, [SystemTime; 2]) {
    let private = root.join("private");
    let locked = private.join("locked");
    std::fs::create_dir_all(&locked).unwrap();
    std::fs::write(locked.join("note.txt"), b"note").unwrap();
    let older = SystemTime::UNIX_EPOCH + Duration::from_secs(1_300_000_000);
    let newer = SystemTime::UNIX_EPOCH + Duration::from_secs(1_350_000_000);
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(locked_mode)).unwrap();
    set_dates(&locked, newer, older);
    std::fs::set_permissions(&private, std::fs::Permissions::from_mode(0o700)).unwrap();
    set_dates(&private, newer, newer);
    (private, [older, newer])
}

fn assert_folder(path: &Path, mode: u32, accessed: SystemTime, modified: SystemTime) {
    let metadata = std::fs::metadata(path).unwrap();
    assert_eq!(metadata.permissions().mode() & 0o7777, mode, "{path:?}");
    assert_eq!(metadata.modified().unwrap(), modified, "{path:?}");
    assert_eq!(metadata.accessed().unwrap(), accessed, "{path:?}");
}

fn assert_copied_folders(copied: &Path, [older, newer]: [SystemTime; 2]) {
    assert_eq!(std::fs::read(copied.join("locked/note.txt")).unwrap(), b"note");
    assert_folder(copied, 0o700, newer, newer);
    // The owner keeps write access, so Undo can still empty it.
    assert_folder(&copied.join("locked"), 0o750, newer, older);
}

fn unlock(root: &Path) {
    for entry in walkdir::WalkDir::new(root).into_iter().flatten() {
        if entry.file_type().is_dir() {
            let _ = std::fs::set_permissions(entry.path(), std::fs::Permissions::from_mode(0o700));
        }
    }
}

#[::core::prelude::v1::test]
fn copied_folders_keep_their_mode_and_dates_and_undo_removes_them() {
    let root = tempfile::tempdir().unwrap();
    let (source, dates) = dated_folders(root.path(), 0o550);
    let target = root.path().join("target");
    std::fs::create_dir(&target).unwrap();
    let history = transfer(FileOpType::Copy, &source, &target);
    assert_copied_folders(&target.join("private"), dates);
    tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(crate::commands::ops::undo_last_action(&history))
        .unwrap();
    assert!(target.join("private").symlink_metadata().is_err());
    unlock(root.path());
}

#[::core::prelude::v1::test]
fn a_folder_merged_into_keeps_its_own_mode_and_dates() {
    let root = tempfile::tempdir().unwrap();
    let (source, [older, newer]) = dated_folders(root.path(), 0o550);
    let target = root.path().join("target");
    let existing = target.join("private");
    std::fs::create_dir_all(&existing).unwrap();
    std::fs::set_permissions(&existing, std::fs::Permissions::from_mode(0o755)).unwrap();
    let mut resolutions = HashMap::new();
    resolutions.insert(source.to_string_lossy().into_owned(), ConflictAction::Replace);
    let op = Arc::new(Mutex::new(FileOperation::new(
        FileOpType::Copy,
        vec![source.clone()],
        Some(target.clone()),
        None,
    )));
    FileOperationManager::perform_copy(&op, vec![source], Some(target.clone()), false, Some(resolutions)).unwrap();
    let metadata = std::fs::metadata(&existing).unwrap();
    assert_eq!(metadata.permissions().mode() & 0o7777, 0o755);
    assert_ne!(metadata.modified().unwrap(), older);
    assert_folder(&existing.join("locked"), 0o750, newer, older);
    unlock(root.path());
}

#[::core::prelude::v1::test]
fn folders_moved_to_another_volume_keep_their_mode_and_dates() {
    use std::os::unix::fs::MetadataExt;
    let Ok(other) = tempfile::tempdir_in("/dev/shm") else { return };
    let root = tempfile::tempdir().unwrap();
    if std::fs::metadata(root.path()).unwrap().dev() == std::fs::metadata(other.path()).unwrap().dev() {
        return;
    }
    // A move must empty the source: `locked` stays writable here.
    let (source, dates) = dated_folders(root.path(), 0o750);
    transfer(FileOpType::Move, &source, other.path());
    assert!(source.symlink_metadata().is_err());
    assert_copied_folders(&other.path().join("private"), dates);
    unlock(other.path());
}

#[::core::prelude::v1::test]
fn folders_moved_on_one_volume_keep_their_mode_and_dates() {
    let root = tempfile::tempdir().unwrap();
    let (source, dates) = dated_folders(root.path(), 0o750);
    let target = root.path().join("target");
    std::fs::create_dir(&target).unwrap();
    transfer(FileOpType::Move, &source, &target);
    assert!(source.symlink_metadata().is_err());
    assert_copied_folders(&target.join("private"), dates);
}
