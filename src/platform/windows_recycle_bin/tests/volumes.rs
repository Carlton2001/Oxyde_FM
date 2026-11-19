use super::*;

#[test]
fn only_fixed_drives_have_a_recycle_bin() {
    // DRIVE_REMOVABLE, DRIVE_REMOTE, DRIVE_CDROM, DRIVE_RAMDISK.
    for drive_type in [2, 4, 5, 6] {
        assert!(!recycles(drive_type), "{drive_type}");
    }
    assert!(recycles(DRIVE_FIXED));
}

#[test]
fn shares_have_no_recycle_bin_and_the_system_drive_has_one() {
    let temp = tempfile::tempdir().unwrap();
    assert!(available(temp.path()));
    let share = Path::new(r"\\localhost\C$\Windows");
    if std::fs::metadata(share).is_ok() {
        assert!(!available(share));
    }
}

#[test]
fn drive_letters_are_read_from_roots_only() {
    assert_eq!(drive_letter(Path::new(r"Z:\")), Some('Z'));
    assert_eq!(drive_letter(Path::new(r"\\?\Z:\")), Some('Z'));
    assert_eq!(drive_letter(Path::new(r"C:\Data\")), None);
    assert_eq!(drive_letter(Path::new(r"\\server\share\")), None);
}

#[test]
fn path_letters_are_read_from_any_drive_path() {
    assert_eq!(path_letter(Path::new(r"Y:\boot.wim")), Some('Y'));
    assert_eq!(path_letter(Path::new(r"\\?\Y:\data")), Some('Y'));
    assert_eq!(path_letter(Path::new(r"\\server\share\x")), None);
}

#[test]
fn subst_drives_have_no_recycle_bin() {
    // Set up by hand: `subst Y: <folder>`, including a folder of a mounted ISO.
    if super::super::windows_final_path::dos_device('Y').is_some_and(|device| device.starts_with(r"\??\")) {
        assert!(!available(Path::new(r"Y:\")));
    }
}
