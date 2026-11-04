use super::icon_file;

#[test]
fn an_icon_file_is_kept_only_when_it_exists() {
    let folder = tempfile::tempdir().unwrap();
    let image = folder.path().join("app.png");
    std::fs::write(&image, b"png").unwrap();
    assert_eq!(icon_file(&image.to_string_lossy()), Some(image));
    assert_eq!(icon_file(&folder.path().join("missing.png").to_string_lossy()), None);
}

#[test]
fn serialized_icons_other_than_themed_names_are_ignored() {
    assert_eq!(icon_file(". GEmblemedIcon x"), None);
    assert_eq!(icon_file("../escape"), None);
}

