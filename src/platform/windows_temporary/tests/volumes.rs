use super::*;

#[test]
fn an_ntfs_temporary_folder_keeps_access_rights() {
    let directory = tempfile::tempdir().unwrap();
    assert!(keeps_access_rights(directory.path()).unwrap());
}

/// Set `OXYDE_TEST_NO_ACLS` to a folder on FAT, exFAT or a `\wsl.localhost` share.
#[test]
fn a_volume_without_access_rights_is_detected() {
    let Some(folder) = std::env::var_os("OXYDE_TEST_NO_ACLS") else { return };
    assert!(!keeps_access_rights(Path::new(&folder)).unwrap());
}
