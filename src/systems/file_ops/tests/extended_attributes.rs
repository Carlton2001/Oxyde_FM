use super::*;
use rustix::fs::XattrFlags;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

/// Access or default ACL granting `nobody` read and search, in the kernel's layout.
fn acl(mode: u32) -> Vec<u8> {
    let mut value = 2_u32.to_le_bytes().to_vec();
    for (tag, permissions, id) in [
        (0x01_u16, (mode >> 6) & 7, u32::MAX),
        (0x02, 5, 65534),
        (0x04, (mode >> 3) & 7, u32::MAX),
        (0x10, (mode >> 3) & 7, u32::MAX),
        (0x20, mode & 7, u32::MAX),
    ] {
        value.extend(tag.to_le_bytes());
        value.extend((permissions as u16).to_le_bytes());
        value.extend(id.to_le_bytes());
    }
    value
}

fn attribute(path: &Path, name: &str) -> Vec<u8> {
    let size = rustix::fs::getxattr(path, name, &mut [0_u8; 0][..]).unwrap();
    let mut value = vec![0; size];
    let size = rustix::fs::getxattr(path, name, &mut value[..]).unwrap();
    value.truncate(size);
    value
}

const KEPT: [&str; 3] = ["user.tag", "system.posix_acl_access", "system.posix_acl_default"];

fn assert_same_attributes(copy: &Path, source: &Path) {
    for name in KEPT {
        assert_eq!(attribute(copy, name), attribute(source, name), "{name} of {copy:?}");
    }
    assert_eq!(attribute(&copy.join("note.txt"), "user.tag"), b"note");
}

#[::core::prelude::v1::test]
fn copied_folders_keep_their_extended_attributes_and_acls_through_undo_and_redo() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("shared");
    std::fs::create_dir(&source).unwrap();
    std::fs::write(source.join("note.txt"), b"note").unwrap();
    rustix::fs::setxattr(source.join("note.txt"), "user.tag", b"note", XattrFlags::empty()).unwrap();
    rustix::fs::setxattr(&source, "user.tag", b"folder", XattrFlags::empty()).unwrap();
    rustix::fs::setxattr(&source, "system.posix_acl_default", &acl(0o750), XattrFlags::empty()).unwrap();
    std::fs::set_permissions(&source, std::fs::Permissions::from_mode(0o750)).unwrap();
    rustix::fs::setxattr(&source, "system.posix_acl_access", &acl(0o750), XattrFlags::empty()).unwrap();
    let target = root.path().join("target");
    std::fs::create_dir(&target).unwrap();
    let copied = target.join("shared");

    let history = Arc::new(HistoryManager::new());
    let op = Arc::new(Mutex::new(FileOperation::new(
        FileOpType::Copy,
        vec![source.clone()],
        Some(target.clone()),
        None,
    )));
    let runtime = tokio::runtime::Builder::new_multi_thread().enable_all().build().unwrap();
    runtime.block_on(FileOperationManager::execute_operation(history.clone(), op.clone()));
    assert_eq!(op.lock().unwrap().status, OpStatus::Completed);
    assert_same_attributes(&copied, &source);

    runtime.block_on(crate::commands::ops::undo_last_action(&history)).unwrap();
    assert!(copied.symlink_metadata().is_err());
    runtime.block_on(crate::commands::ops::redo_last_action(&history)).unwrap();
    assert_same_attributes(&copied, &source);
}
