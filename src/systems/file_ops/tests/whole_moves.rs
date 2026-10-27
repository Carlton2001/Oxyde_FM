use super::*;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::Path;

fn move_operation(source: &Path, target: &Path) -> Arc<Mutex<FileOperation>> {
    Arc::new(Mutex::new(FileOperation::new(
        FileOpType::Move,
        vec![source.to_path_buf()],
        Some(target.to_path_buf()),
        None,
    )))
}

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_multi_thread().enable_all().build().unwrap()
}

fn inode(path: &Path) -> u64 {
    std::fs::symlink_metadata(path).unwrap().ino()
}

/// `album` holding `locked` (550) and its photo: its files cannot leave `locked` one by one.
fn album(root: &Path) -> PathBuf {
    let album = root.join("album");
    std::fs::create_dir_all(album.join("locked")).unwrap();
    std::fs::write(album.join("locked/photo.jpg"), b"photo").unwrap();
    std::fs::set_permissions(album.join("locked"), std::fs::Permissions::from_mode(0o550)).unwrap();
    album
}

fn unlock(path: &Path) {
    let _ = std::fs::set_permissions(path.join("locked"), std::fs::Permissions::from_mode(0o750));
}

#[::core::prelude::v1::test]
fn a_folder_is_moved_whole_on_its_volume_and_undo_and_redo_rename_it_back_and_forth() {
    let root = tempfile::tempdir().unwrap();
    let source = album(root.path());
    let target = root.path().join("target");
    std::fs::create_dir(&target).unwrap();
    let moved = target.join("album");
    let folder = inode(&source);
    let history = Arc::new(HistoryManager::new());
    let op = move_operation(&source, &target);
    let runtime = runtime();
    runtime.block_on(FileOperationManager::execute_operation(history.clone(), op.clone()));
    assert_eq!(op.lock().unwrap().status, OpStatus::Completed);
    assert!(source.symlink_metadata().is_err());
    // The same folder, not a recreated one, and its locked subfolder came along.
    assert_eq!(inode(&moved), folder);
    assert_eq!(std::fs::read(moved.join("locked/photo.jpg")).unwrap(), b"photo");

    runtime.block_on(crate::commands::ops::undo_last_action(&history)).unwrap();
    assert!(moved.symlink_metadata().is_err());
    assert_eq!(inode(&source), folder);
    runtime.block_on(crate::commands::ops::redo_last_action(&history)).unwrap();
    assert!(source.symlink_metadata().is_err());
    assert_eq!(inode(&moved), folder);
    unlock(&moved);
}

#[::core::prelude::v1::test]
fn undo_keeps_the_folder_moved_when_its_old_name_is_taken() {
    let root = tempfile::tempdir().unwrap();
    let source = album(root.path());
    let target = root.path().join("target");
    std::fs::create_dir(&target).unwrap();
    let history = Arc::new(HistoryManager::new());
    let runtime = runtime();
    runtime.block_on(FileOperationManager::execute_operation(history.clone(), move_operation(&source, &target)));
    std::fs::create_dir(&source).unwrap();
    std::fs::write(source.join("new.txt"), b"new").unwrap();
    assert!(runtime.block_on(crate::commands::ops::undo_last_action(&history)).is_err());
    assert_eq!(std::fs::read(source.join("new.txt")).unwrap(), b"new");
    assert_eq!(std::fs::read(target.join("album/locked/photo.jpg")).unwrap(), b"photo");
    assert_eq!(history.get_state().undo_stack.len(), 1);
    unlock(&target.join("album"));
}

#[::core::prelude::v1::test]
fn a_folder_merged_into_an_existing_one_still_moves_item_by_item() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("album");
    std::fs::create_dir(&source).unwrap();
    std::fs::write(source.join("new.jpg"), b"new").unwrap();
    let target = root.path().join("target");
    std::fs::create_dir_all(target.join("album")).unwrap();
    std::fs::write(target.join("album/old.jpg"), b"old").unwrap();
    let existing = inode(&target.join("album"));
    let resolutions = HashMap::from([(source.to_string_lossy().into_owned(), ConflictAction::Replace)]);
    let op = move_operation(&source, &target);
    FileOperationManager::perform_copy(&op, vec![source.clone()], Some(target.clone()), true, Some(resolutions)).unwrap();
    assert_eq!(inode(&target.join("album")), existing);
    assert_eq!(std::fs::read(target.join("album/old.jpg")).unwrap(), b"old");
    assert_eq!(std::fs::read(target.join("album/new.jpg")).unwrap(), b"new");
    assert!(source.symlink_metadata().is_err());
    assert!(op.lock().unwrap().renamed_dirs.is_empty());
}
