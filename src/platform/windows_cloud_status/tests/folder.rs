use super::*;
use windows::Win32::Storage::CloudFilters::{
    CF_PLACEHOLDER_STATE_IN_SYNC, CF_PLACEHOLDER_STATE_PARTIALLY_ON_DISK,
};

#[test]
fn a_nested_cloud_file_can_keep_the_parent_in_progress() {
    let root = tempfile::tempdir().unwrap();
    let child = root.path().join("child");
    std::fs::create_dir(&child).unwrap();
    std::fs::write(child.join("letter.txt"), b"fixture").unwrap();

    for evicted in [false, true] {
        let status = contents_ready(root.path(), CloudAvailability::OnlineOnly, |data| {
            if data.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY.0 != 0 {
                return CF_PLACEHOLDER_STATE(0);
            }
            let state = CF_PLACEHOLDER_STATE_PLACEHOLDER | CF_PLACEHOLDER_STATE_IN_SYNC;
            if evicted {
                state | CF_PLACEHOLDER_STATE_PARTIALLY_ON_DISK
            } else {
                state
            }
        });
        assert_eq!(status, Some(evicted));
    }
}

#[test]
fn a_junction_is_skipped_instead_of_checking_the_target() {
    let root = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    std::fs::write(outside.path().join("local.txt"), b"fixture").unwrap();
    crate::platform::windows_reparse_tests::junction(&root.path().join("link"), outside.path());
    let status = contents_ready(root.path(), CloudAvailability::OnlineOnly, |data| {
        assert_ne!(data.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT.0, 0);
        CF_PLACEHOLDER_STATE(0)
    });
    assert_eq!(status, Some(true));
}
