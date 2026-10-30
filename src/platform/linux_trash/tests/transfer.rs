use super::*;

struct Fixture {
    _root: tempfile::TempDir,
    content: PathBuf,
    record: PathBuf,
    item: trash::TrashItem,
}

fn fixture() -> Fixture {
    let root = tempfile::tempdir().unwrap();
    let trash = root.path().join("Trash");
    std::fs::create_dir_all(trash.join("files")).unwrap();
    std::fs::create_dir_all(trash.join("info")).unwrap();
    let content = trash.join("files/report.2.txt");
    let record = trash.join("info/report.2.txt.trashinfo");
    std::fs::write(&content, b"content").unwrap();
    std::fs::write(&record, b"[Trash Info]\nPath=/somewhere/report.txt\n").unwrap();
    let item = trash::TrashItem {
        id: record.clone().into_os_string(),
        name: "report.txt".into(),
        original_parent: PathBuf::from("/somewhere"),
        time_deleted: 0,
    };
    Fixture { _root: root, content, record, item }
}

#[test]
fn the_entry_gives_the_original_name_of_a_renamed_item() {
    let fixture = fixture();
    let entry = entry(&fixture.content, &fixture.item).unwrap();
    assert_eq!(entry.original_path, Path::new("/somewhere/report.txt"));
    assert_eq!(entry.metadata_path, fixture.record);
}

#[test]
fn an_item_still_in_the_trash_keeps_its_record() {
    let fixture = fixture();
    let entry = entry(&fixture.content, &fixture.item).unwrap();
    assert!(detach(&entry).unwrap().is_none());
    assert!(fixture.record.exists());
}

#[test]
fn the_record_of_an_item_that_left_is_kept_aside_for_undo() {
    let fixture = fixture();
    let entry = entry(&fixture.content, &fixture.item).unwrap();
    std::fs::remove_file(&fixture.content).unwrap();
    let detached = detach(&entry).unwrap().unwrap();
    assert!(!fixture.record.exists());
    assert_eq!(detached.source, fixture.record);
    assert_eq!(std::fs::read(&detached.backup).unwrap(), b"[Trash Info]\nPath=/somewhere/report.txt\n");
    assert!(!detached.backup.starts_with(fixture.record.parent().unwrap()), "listings read every file in info");
}

#[test]
fn a_record_changed_during_the_transfer_stays() {
    let fixture = fixture();
    let entry = entry(&fixture.content, &fixture.item).unwrap();
    std::fs::remove_file(&fixture.content).unwrap();
    std::fs::write(&fixture.record, b"[Trash Info]\nPath=/elsewhere/report.txt\n").unwrap();
    assert!(detach(&entry).is_err());
    assert!(fixture.record.exists());
}
