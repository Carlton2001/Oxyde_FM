use super::*;

/// A folder on the home trash's file system, so that tests use the home trash.
fn folder() -> tempfile::TempDir {
    let data = dirs::data_dir().unwrap();
    std::fs::create_dir_all(&data).unwrap();
    tempfile::tempdir_in(data).unwrap()
}

/// Removes an entry a test created, content and record.
fn discard(content: &Path) {
    let trash = content.parent().unwrap().parent().unwrap();
    let mut record = content.file_name().unwrap().to_os_string();
    record.push(".trashinfo");
    let _ = std::fs::remove_dir_all(content).or_else(|_| std::fs::remove_file(content));
    std::fs::remove_file(trash.join("info").join(record)).unwrap();
}

fn record_of(content: &Path) -> String {
    let mut record = content.file_name().unwrap().to_os_string();
    record.push(".trashinfo");
    std::fs::read_to_string(content.parent().unwrap().parent().unwrap().join("info").join(record)).unwrap()
}

#[test]
fn an_item_goes_to_a_new_entry_with_its_record() {
    let root = folder();
    let first = root.path().join("même nom.txt");
    std::fs::write(&first, b"first").unwrap();
    let sent = send(&first, None).unwrap();
    assert!(!first.exists());
    assert_eq!(std::fs::read(&sent.content).unwrap(), b"first");
    let record = record_of(&sent.content);
    assert!(record.starts_with("[Trash Info]\nPath=/"), "{record}");
    assert!(record.contains("m%C3%AAme%20nom.txt\nDeletionDate="), "{record}");
    // The same name again takes the next free entry.
    std::fs::write(&first, b"second").unwrap();
    let again = send(&first, None).unwrap();
    assert_ne!(again.content, sent.content);
    assert_eq!(std::fs::read(&again.content).unwrap(), b"second");
    let listed = trash::os_limited::list().unwrap();
    for content in [&sent.content, &again.content] {
        assert!(listed.iter().any(|item| super::super::content_path(item).as_deref() == Some(content.as_path())
            && item.original_path() == first), "{content:?}");
        discard(content);
    }
}

#[test]
fn another_object_in_the_expected_place_stays() {
    let root = folder();
    let path = root.path().join("doc.txt");
    std::fs::write(&path, b"deleted").unwrap();
    let expected = HistoryFileIdentity::from_file(&File::open(&path).unwrap()).unwrap();
    std::fs::remove_file(&path).unwrap();
    std::fs::write(root.path().join("other"), b"keeps the inode busy").unwrap();
    std::fs::write(&path, b"replacement").unwrap();
    assert!(matches!(send(&path, Some(expected)), Err(SendError::Changed)));
    assert_eq!(std::fs::read(&path).unwrap(), b"replacement");
    let current = HistoryFileIdentity::from_file(&File::open(&path).unwrap()).unwrap();
    let sent = send(&path, Some(current)).unwrap();
    discard(&sent.content);
}

#[test]
fn folders_and_links_go_whole_and_unfollowed() {
    let root = folder();
    let tree = root.path().join("tree");
    std::fs::create_dir_all(tree.join("sub")).unwrap();
    std::fs::write(tree.join("sub/file"), b"inside").unwrap();
    let link = root.path().join("link");
    std::os::unix::fs::symlink(&tree, &link).unwrap();
    let link_sent = send(&link, None).unwrap();
    assert!(tree.join("sub/file").exists());
    assert!(std::fs::symlink_metadata(&link_sent.content).unwrap().file_type().is_symlink());
    let tree_sent = send(&tree, None).unwrap();
    assert_eq!(std::fs::read(tree_sent.content.join("sub/file")).unwrap(), b"inside");
    discard(&link_sent.content);
    discard(&tree_sent.content);
}

#[test]
fn an_item_in_the_trash_is_refused() {
    let root = folder();
    let path = root.path().join("once");
    std::fs::write(&path, b"x").unwrap();
    let sent = send(&path, None).unwrap();
    assert!(send(&sent.content, None).is_err());
    discard(&sent.content);
}

/// Another file system has its own trash at its root. Only an administrator of a mount
/// namespace can mount one: `unshare --map-root-user --mount` runs these tests as one.
#[test]
fn another_file_system_uses_its_own_trash() {
    use std::os::unix::ffi::OsStrExt;
    let root = tempfile::tempdir().unwrap();
    let target = std::ffi::CString::new(root.path().as_os_str().as_bytes()).unwrap();
    // SAFETY: a path and constant strings for a tmpfs mount.
    if unsafe { libc::mount(c"tmpfs".as_ptr(), target.as_ptr(), c"tmpfs".as_ptr(), 0, std::ptr::null()) } != 0 {
        return;
    }
    let deep = root.path().join("a/b");
    std::fs::create_dir_all(&deep).unwrap();
    std::fs::write(deep.join("file"), b"x").unwrap();
    let sent = send(&deep.join("file"), None);
    let uid = rustix::process::getuid().as_raw();
    let expected = root.path().join(format!(".Trash-{uid}/files/file"));
    let result = sent.map(|sent| sent.content);
    let content = std::fs::read(&expected);
    // SAFETY: the path mounted above.
    unsafe { libc::umount2(target.as_ptr(), libc::MNT_DETACH) };
    assert_eq!(result.unwrap(), expected);
    assert_eq!(content.unwrap(), b"x");
}
