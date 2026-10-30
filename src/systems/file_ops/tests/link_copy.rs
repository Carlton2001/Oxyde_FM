use super::*;
use std::os::unix::fs::symlink;
use std::path::Path;

fn copy_operation(sources: Vec<PathBuf>, target: &Path) -> Arc<Mutex<FileOperation>> {
    Arc::new(Mutex::new(FileOperation::new(
        FileOpType::Copy,
        sources,
        Some(target.to_path_buf()),
        None,
    )))
}

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
}

fn assert_link(path: &Path, target: &str) {
    assert!(path.symlink_metadata().unwrap().file_type().is_symlink(), "{path:?}");
    assert_eq!(std::fs::read_link(path).unwrap(), Path::new(target));
}

/// A folder holding a link to a file, a link to an outside folder and a broken link.
fn linked_folder(root: &Path) -> (PathBuf, PathBuf) {
    let source = root.join("source");
    let outside = root.join("outside");
    std::fs::create_dir_all(&source).unwrap();
    std::fs::create_dir_all(&outside).unwrap();
    std::fs::write(source.join("data.txt"), b"data").unwrap();
    std::fs::write(outside.join("witness"), b"outside").unwrap();
    symlink("data.txt", source.join("to-file")).unwrap();
    symlink(&outside, source.join("to-folder")).unwrap();
    symlink("missing", source.join("broken")).unwrap();
    let target = root.join("target");
    std::fs::create_dir(&target).unwrap();
    (source, target)
}

#[::core::prelude::v1::test]
fn copied_links_keep_their_targets_and_undo_and_redo_follow_them() {
    let root = tempfile::tempdir().unwrap();
    let (source, target) = linked_folder(root.path());
    let outside = root.path().join("outside");
    let copied = target.join("source");
    let history = Arc::new(HistoryManager::new());
    let op = copy_operation(vec![source.clone()], &target);
    runtime().block_on(async {
        FileOperationManager::execute_operation(history.clone(), op.clone()).await;
        assert_eq!(op.lock().unwrap().status, OpStatus::Completed);
        assert_eq!(std::fs::read(copied.join("data.txt")).unwrap(), b"data");
        assert_link(&copied.join("to-file"), "data.txt");
        assert_link(&copied.join("to-folder"), outside.to_str().unwrap());
        assert_link(&copied.join("broken"), "missing");
        assert_eq!(std::fs::read_dir(&outside).unwrap().count(), 1);

        crate::commands::ops::undo_last_action(&history).await.unwrap();
        assert!(copied.symlink_metadata().is_err());
        assert_eq!(std::fs::read(outside.join("witness")).unwrap(), b"outside");
        assert_link(&source.join("to-folder"), outside.to_str().unwrap());

        crate::commands::ops::redo_last_action(&history).await.unwrap();
        assert_link(&copied.join("to-file"), "data.txt");
        assert_link(&copied.join("broken"), "missing");
        crate::commands::ops::undo_last_action(&history).await.unwrap();
        assert!(copied.symlink_metadata().is_err());
    });
}

#[::core::prelude::v1::test]
fn undo_keeps_a_link_replaced_since_the_copy() {
    let root = tempfile::tempdir().unwrap();
    let (source, target) = linked_folder(root.path());
    let link = target.join("to-file");
    let history = Arc::new(HistoryManager::new());
    let op = copy_operation(vec![source.join("to-file")], &target);
    runtime().block_on(async {
        FileOperationManager::execute_operation(history.clone(), op.clone()).await;
        assert_eq!(op.lock().unwrap().status, OpStatus::Completed);
        assert_link(&link, "data.txt");
        // Kept aside: ext4 would give a removed link's number to its replacement, in the
        // same clock tick, and no identity can tell them apart.
        std::fs::rename(&link, root.path().join("aside")).unwrap();
        symlink("data.txt", &link).unwrap();
        assert!(crate::commands::ops::undo_last_action(&history).await.is_err());
        assert_link(&link, "data.txt");
        assert_eq!(history.get_state().undo_stack.len(), 1);
    });
}

#[::core::prelude::v1::test]
fn a_link_copied_into_its_own_folder_gets_a_copy_name() {
    let root = tempfile::tempdir().unwrap();
    let (source, _) = linked_folder(root.path());
    let op = copy_operation(vec![source.join("broken")], &source);
    FileOperationManager::perform_copy(&op, vec![source.join("broken")], Some(source.clone()), false, None)
        .unwrap();
    let names: Vec<_> = std::fs::read_dir(&source)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .filter(|name| name.starts_with("broken"))
        .collect();
    assert_eq!(names.len(), 2, "{names:?}");
    for name in names {
        assert_link(&source.join(name), "missing");
    }
}

#[::core::prelude::v1::test]
fn a_broken_link_at_the_destination_is_a_conflict() {
    let root = tempfile::tempdir().unwrap();
    let (source, target) = linked_folder(root.path());
    let link = source.join("to-file");
    symlink("elsewhere", target.join("to-file")).unwrap();
    let op = copy_operation(vec![link.clone()], &target);
    FileOperationManager::perform_copy(&op, vec![link.clone()], Some(target.clone()), false, None).unwrap();
    assert_link(&target.join("to-file"), "elsewhere");

    let resolutions = HashMap::from([(link.to_string_lossy().into_owned(), ConflictAction::Replace)]);
    let op = copy_operation(vec![link.clone()], &target);
    FileOperationManager::perform_copy(&op, vec![link.clone()], Some(target.clone()), false, Some(resolutions))
        .unwrap();
    assert_link(&target.join("to-file"), "data.txt");
    assert_eq!(op.lock().unwrap().overwritten_files, vec![target.join("to-file")]);
    assert_eq!(std::fs::read_dir(&target).unwrap().count(), 1);
}

#[::core::prelude::v1::test]
fn a_link_cannot_replace_a_folder() {
    let root = tempfile::tempdir().unwrap();
    let (source, target) = linked_folder(root.path());
    let link = source.join("to-file");
    std::fs::create_dir(target.join("to-file")).unwrap();
    std::fs::write(target.join("to-file/kept"), b"kept").unwrap();
    let resolutions = HashMap::from([(link.to_string_lossy().into_owned(), ConflictAction::Replace)]);
    let op = copy_operation(vec![link.clone()], &target);
    assert!(FileOperationManager::perform_copy(&op, vec![link], Some(target.clone()), false, Some(resolutions)).is_err());
    assert_eq!(std::fs::read(target.join("to-file/kept")).unwrap(), b"kept");
    assert_eq!(std::fs::read_dir(&target).unwrap().count(), 1);
}

fn move_operation(sources: Vec<PathBuf>, target: &Path) -> Arc<Mutex<FileOperation>> {
    Arc::new(Mutex::new(FileOperation::new(
        FileOpType::Move,
        sources,
        Some(target.to_path_buf()),
        None,
    )))
}

fn assert_linked_folder(folder: &Path, outside: &Path) {
    assert_eq!(std::fs::read(folder.join("data.txt")).unwrap(), b"data");
    assert_link(&folder.join("to-file"), "data.txt");
    assert_link(&folder.join("to-folder"), outside.to_str().unwrap());
    assert_link(&folder.join("broken"), "missing");
    assert_eq!(std::fs::read(outside.join("witness")).unwrap(), b"outside");
    assert_eq!(std::fs::read_dir(outside).unwrap().count(), 1);
}

/// Move a folder of links, then undo, redo and undo again.
fn move_round_trip(source_root: &Path, target_root: &Path) {
    let (source, _) = linked_folder(source_root);
    let target = target_root.join("moved-into");
    std::fs::create_dir(&target).unwrap();
    let outside = source_root.join("outside");
    let moved = target.join("source");
    let history = Arc::new(HistoryManager::new());
    let op = move_operation(vec![source.clone()], &target);
    runtime().block_on(async {
        FileOperationManager::execute_operation(history.clone(), op.clone()).await;
        assert_eq!(op.lock().unwrap().status, OpStatus::Completed);
        assert!(source.symlink_metadata().is_err());
        assert_linked_folder(&moved, &outside);

        crate::commands::ops::undo_last_action(&history).await.unwrap();
        assert!(moved.symlink_metadata().is_err());
        assert_linked_folder(&source, &outside);

        crate::commands::ops::redo_last_action(&history).await.unwrap();
        assert!(source.symlink_metadata().is_err());
        assert_linked_folder(&moved, &outside);

        crate::commands::ops::undo_last_action(&history).await.unwrap();
        assert_linked_folder(&source, &outside);
    });
}

#[::core::prelude::v1::test]
fn moved_links_keep_their_targets_and_undo_and_redo_follow_them() {
    let root = tempfile::tempdir().unwrap();
    move_round_trip(root.path(), root.path());
}

#[::core::prelude::v1::test]
fn links_moved_to_another_volume_are_recreated_then_removed() {
    use std::os::unix::fs::MetadataExt;
    let Ok(other) = tempfile::tempdir_in("/dev/shm") else { return };
    let root = tempfile::tempdir().unwrap();
    let device = |path: &Path| std::fs::metadata(path).unwrap().dev();
    // Fedora mounts /tmp and /dev/shm separately: a rename between them fails.
    if device(root.path()) == device(other.path()) {
        return;
    }
    move_round_trip(root.path(), other.path());
}

#[::core::prelude::v1::test]
fn undo_keeps_a_moved_link_replaced_since_the_move() {
    let root = tempfile::tempdir().unwrap();
    let (source, target) = linked_folder(root.path());
    let link = target.join("to-file");
    let history = Arc::new(HistoryManager::new());
    let op = move_operation(vec![source.join("to-file")], &target);
    runtime().block_on(async {
        FileOperationManager::execute_operation(history.clone(), op.clone()).await;
        assert_eq!(op.lock().unwrap().status, OpStatus::Completed);
        std::fs::remove_file(&link).unwrap();
        symlink("data.txt", &link).unwrap();
        assert!(crate::commands::ops::undo_last_action(&history).await.is_err());
        assert_link(&link, "data.txt");
        assert!(source.join("to-file").symlink_metadata().is_err());
    });
}

#[::core::prelude::v1::test]
fn compressing_links_is_refused_before_any_archive_exists() {
    let root = tempfile::tempdir().unwrap();
    let (source, target) = linked_folder(root.path());
    let archive = target.join("source.zip");
    let op = Arc::new(Mutex::new(FileOperation::new(
        FileOpType::Compress,
        vec![source.clone()],
        Some(archive.clone()),
        None,
    )));
    let error = FileOperationManager::perform_compress(&op, vec![source], Some(archive)).unwrap_err();
    assert!(crate::models::error::ArchiveLinkRefusal::from_message(&error).is_some(), "{error}");
    assert_eq!(std::fs::read_dir(&target).unwrap().count(), 0);
}
