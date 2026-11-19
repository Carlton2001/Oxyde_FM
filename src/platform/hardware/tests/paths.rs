use super::*;

#[test]
fn verbatim_drive_and_share_prefixes_keep_their_storage_kind() {
    assert_eq!(plain_path(Path::new(r"\\?\C:\Users")), r"C:\Users");
    assert_eq!(plain_path(Path::new(r"\\?\UNC\host\share\x")), r"\\host\share\x");
    assert_eq!(plain_path(Path::new(r"D:\data")), r"D:\data");
    assert!(!is_network_path(Path::new(r"\\?\C:\Users")));
    assert!(is_network_path(Path::new(r"\\?\UNC\host\share")));
    assert_eq!(is_ssd(Path::new(r"\\?\C:\Users")), is_ssd(Path::new(r"C:\Users")));
}

#[test]
fn non_ascii_first_character_is_not_a_drive_letter() {
    assert_eq!(drive_letter(r"c:\x"), Some('C'));
    assert_eq!(drive_letter(r"é:\x"), None);
    assert_eq!(drive_letter("1:"), None);
    assert!(!is_network_path(Path::new(r"é:\x")));
    assert!(!is_ssd(Path::new(r"é:\x")));
    assert_eq!(get_physical_disk_id(Path::new(r"é:\x")), 0);
}
