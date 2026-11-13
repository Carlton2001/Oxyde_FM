use super::*;

#[::core::prelude::v1::test]
fn directory_type_takes_precedence_over_extension() {
    let temp = tempfile::tempdir().unwrap();
    let folder = temp.path().join("folder.zip");
    std::fs::create_dir(&folder).unwrap();
    assert_eq!(resolve(&folder, None).unwrap().1, IconType::Folder);
}

#[::core::prelude::v1::test]
fn moved_directory_uses_destination_identity() {
    let temp = tempfile::tempdir().unwrap();
    let destination = temp.path().join("moved-folder");
    std::fs::create_dir(&destination).unwrap();
    let (path, icon) = resolve(&temp.path().join("missing-source"), Some(&destination)).unwrap();
    assert_eq!(path, destination.to_string_lossy());
    assert_eq!(icon, IconType::Folder);
}

#[::core::prelude::v1::test]
fn existing_source_is_preferred_to_destination() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("source.txt");
    std::fs::write(&source, b"text").unwrap();
    let (path, icon) = resolve(&source, Some(temp.path())).unwrap();
    assert_eq!(path, source.to_string_lossy());
    assert_eq!(icon, IconType::FileText);
}
