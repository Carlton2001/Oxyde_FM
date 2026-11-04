use super::extension_change;

#[test]
fn a_changed_or_removed_extension_is_reported() {
    assert_eq!(extension_change("notes.txt", "notes.md", false), Some(("txt".into(), Some("md".into()))));
    assert_eq!(extension_change("notes.txt", "notes", false), Some(("txt".into(), None)));
}

#[test]
fn a_kept_or_absent_extension_and_folders_are_silent() {
    assert_eq!(extension_change("notes.txt", "draft.txt", false), None);
    assert_eq!(extension_change("README", "README.md", false), None);
    assert_eq!(extension_change(".bashrc", ".zshrc", false), None);
    assert_eq!(extension_change("photos.2024", "photos", true), None);
}
