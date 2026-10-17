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
