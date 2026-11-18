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
