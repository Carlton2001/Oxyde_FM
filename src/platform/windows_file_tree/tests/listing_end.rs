use super::*;
use std::os::windows::fs::OpenOptionsExt;

#[test]
fn an_empty_folder_on_a_redirector_ends_its_listing() {
    assert!(end_of_listing(0x8000_0006_u32 as i32, false));
    assert!(end_of_listing(0x8000_0006_u32 as i32, true));
    // STATUS_NO_SUCH_FILE: an empty first answer, an error afterwards.
    assert!(end_of_listing(0xC000_000F_u32 as i32, true));
    assert!(!end_of_listing(0xC000_000F_u32 as i32, false));
    // STATUS_OBJECT_NAME_NOT_FOUND also maps to "file not found" but is no end.
    assert!(!end_of_listing(0xC000_0034_u32 as i32, true));
    assert!(!end_of_listing(0, true));
}

#[test]
fn an_empty_folder_lists_nothing() {
    let root = tempfile::tempdir().unwrap();
    let directory = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(0x02000000)
        .open(root.path())
        .unwrap();
    let mut names = Vec::new();
    enumerate(&directory, &mut |name| {
        names.push(name);
        Ok(())
    })
    .unwrap();
    assert!(names.is_empty());
}

#[test]
fn a_read_only_file_does_not_stop_the_removal_of_its_folder() {
    let root = tempfile::tempdir().unwrap();
    let tree = root.path().join("repo");
    std::fs::create_dir_all(tree.join("objects")).unwrap();
    for name in ["a", "objects/b"] {
        let file = tree.join(name);
        std::fs::write(&file, b"x").unwrap();
        let mut permissions = std::fs::metadata(&file).unwrap().permissions();
        permissions.set_readonly(true);
        std::fs::set_permissions(&file, permissions).unwrap();
    }
    let parent = std::fs::OpenOptions::new().read(true).custom_flags(0x02000000).open(root.path()).unwrap();
    remove(&parent, &tree, 1, &|| Ok(())).unwrap();
    assert!(!tree.exists());
}
