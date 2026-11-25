use super::{icon_file, IconFolders};

#[test]
fn an_icon_file_is_kept_only_when_it_exists() {
    let folder = tempfile::tempdir().unwrap();
    let image = folder.path().join("app.png");
    std::fs::write(&image, b"png").unwrap();
    assert_eq!(icon_file(&image.to_string_lossy(), &IconFolders(Vec::new())), Some(image));
    assert_eq!(icon_file(&folder.path().join("missing.png").to_string_lossy(), &IconFolders(Vec::new())), None);
}

#[test]
fn serialized_icons_other_than_themed_names_are_ignored() {
    assert_eq!(icon_file(". GEmblemedIcon x", &IconFolders(Vec::new())), None);
    assert_eq!(icon_file("../escape", &IconFolders(Vec::new())), None);
}


#[test]
fn themed_names_are_found_in_folder_order_then_by_extension() {
    let root = tempfile::tempdir().unwrap();
    let (large, small) = (root.path().join("256x256"), root.path().join("48x48"));
    std::fs::create_dir(&large).unwrap();
    std::fs::create_dir(&small).unwrap();
    std::fs::write(large.join("app.png"), b"png").unwrap();
    std::fs::write(small.join("app.svg"), b"svg").unwrap();
    std::fs::write(small.join("other.png"), b"png").unwrap();
    let listed = |folder: &std::path::Path| (folder.to_path_buf(), std::fs::read_dir(folder).unwrap().map(|entry| entry.unwrap().file_name()).collect());
    let folders = IconFolders(vec![listed(&large), listed(&small)]);
    assert_eq!(icon_file(". GThemedIcon missing app", &folders), Some(large.join("app.png")));
    assert_eq!(icon_file("other", &folders), Some(small.join("other.png")));
    // A file gone since the folder was read is passed over.
    std::fs::remove_file(large.join("app.png")).unwrap();
    assert_eq!(icon_file("app", &folders), Some(small.join("app.svg")));
}
