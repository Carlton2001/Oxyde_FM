use super::*;

#[test]
fn watchers_follow_real_directories_and_the_resolved_archive_parent() {
    let root = tempfile::tempdir().unwrap();
    let folder = root.path().join("ordinary.zip");
    std::fs::create_dir(&folder).unwrap();
    let mut tab = TabState::new("test".into(), folder.to_string_lossy().into_owned());
    assert_eq!(tab.watched_path(), tab.path);
    let archive = folder.join("inner.7z");
    std::fs::write(&archive, b"fixture").unwrap();
    tab.path = archive.join("child").to_string_lossy().into_owned();
    tab.archive_root = Some(archive.clone());
    assert_eq!(tab.watched_path(), folder.to_string_lossy());
    std::fs::remove_file(archive).unwrap();
    // Preserve the containing-directory subscription while the archive is absent.
    assert_eq!(tab.watched_path(), folder.to_string_lossy());
}
