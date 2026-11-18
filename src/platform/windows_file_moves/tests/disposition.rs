use super::*;
use std::fs::OpenOptions;
use std::os::windows::fs::OpenOptionsExt;

fn open_for_delete(path: &Path) -> File {
    OpenOptions::new()
        .access_mode(DELETE.0 | FILE_READ_ATTRIBUTES.0)
        .share_mode(7)
        // FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT
        .custom_flags(0x02000000 | 0x00200000)
        .open(path)
        .unwrap()
}

// Defender, the indexer or a thumbnailer keeps a child open with delete
// sharing: its parent must still be removable once the child is deleted.
#[test]
fn a_child_held_by_another_process_leaves_its_parent_removable() {
    let root = tempfile::tempdir().unwrap();
    let folder = root.path().join("folder");
    std::fs::create_dir(&folder).unwrap();
    let child = folder.join("held.txt");
    std::fs::write(&child, b"x").unwrap();
    let holder = OpenOptions::new().read(true).share_mode(7).open(&child).unwrap();
    remove(&open_for_delete(&child)).unwrap();
    remove(&open_for_delete(&folder)).unwrap();
    drop(holder);
    assert!(!folder.exists());
}

#[test]
fn read_only_files_are_still_refused() {
    let root = tempfile::tempdir().unwrap();
    let file = root.path().join("locked.txt");
    std::fs::write(&file, b"x").unwrap();
    let mut permissions = std::fs::metadata(&file).unwrap().permissions();
    permissions.set_readonly(true);
    std::fs::set_permissions(&file, permissions.clone()).unwrap();
    assert!(remove(&open_for_delete(&file)).is_err());
    permissions.set_readonly(false);
    std::fs::set_permissions(&file, permissions).unwrap();
}
