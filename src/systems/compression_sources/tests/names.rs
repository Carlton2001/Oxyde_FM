use super::*;

#[cfg(target_os = "windows")]
#[test]
fn a_drive_root_is_named_by_its_letter() {
    assert_eq!(drive_letter(Path::new(r"E:\")), Some("E".into()));
    assert_eq!(drive_letter(Path::new(r"\\?\E:\")), Some("E".into()));
    assert_eq!(drive_letter(Path::new(r"\\server\share")), None);
}

#[test]
fn names_differing_by_case_collide_only_on_windows() {
    assert_eq!(name_key("README") == name_key("readme"), cfg!(target_os = "windows"));
}
