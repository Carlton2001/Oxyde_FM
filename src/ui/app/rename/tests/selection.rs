use super::initial_selection_len;

#[test]
fn archive_selection_excludes_the_complete_extension() {
    for extension in ["zip", "7z", "tar", "tar.zst"] {
        let name = format!("rapport.v2.{extension}");
        assert_eq!(&name[..initial_selection_len(&name, false, Some(extension))], "rapport.v2");
    }
    let name = "été.tar.zst";
    assert_eq!(&name[..initial_selection_len(name, false, Some("tar.zst"))], "été");
}

#[test]
fn ordinary_rename_selection_preserves_file_and_folder_behavior() {
    assert_eq!(initial_selection_len("report.txt", false, None), 6);
    assert_eq!(initial_selection_len("folder.name", true, None), 11);
    assert_eq!(initial_selection_len("README", false, None), 6);
}
