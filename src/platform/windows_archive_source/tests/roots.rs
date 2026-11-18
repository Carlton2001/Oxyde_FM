use super::*;

fn open_directory(path: &Path) -> File {
    OpenOptions::new()
        .access_mode(0x80) // FILE_READ_ATTRIBUTES
        .share_mode(7)
        .custom_flags(0x0200_0000) // FILE_FLAG_BACKUP_SEMANTICS
        .open(path)
        .unwrap()
}

/// Set `OXYDE_TEST_FAT_ROOT` to the root of a FAT32 or exFAT volume, e.g. `G:\`.
#[test]
fn a_fat_volume_root_is_identified_by_its_volume() {
    let Some(root) = std::env::var_os("OXYDE_TEST_FAT_ROOT") else { return };
    let root = Path::new(&root);
    let first = identity(&open_directory(root)).unwrap();
    assert_eq!(first, identity(&open_directory(root)).unwrap());
    let folder = tempfile::tempdir_in(root).unwrap();
    assert_ne!(first, identity(&open_directory(folder.path())).unwrap());
}

#[test]
fn an_ordinary_folder_is_not_taken_for_a_volume_root() {
    let folder = tempfile::tempdir().unwrap();
    assert!(!crate::platform::windows_final_path::is_volume_root(&open_directory(folder.path())).unwrap());
    let system = std::env::var_os("SystemDrive").unwrap();
    let root = format!("{}\\", system.to_string_lossy());
    assert!(crate::platform::windows_final_path::is_volume_root(&open_directory(Path::new(&root))).unwrap());
}
