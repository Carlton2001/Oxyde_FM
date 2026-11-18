use super::*;
use crate::models::transaction::HistoryDeletedItem;

fn folder() -> tempfile::TempDir {
    let data = dirs::data_dir().unwrap();
    std::fs::create_dir_all(&data).unwrap();
    tempfile::tempdir_in(data).unwrap()
}

fn trashed(path: &Path) -> HistoryDeletedItem {
    let outcome = platform().shell_delete_tracked(path, None);
    assert!(outcome.completed, "{:?}", outcome.error);
    outcome.effects.into_iter().next().unwrap()
}

fn run<T>(future: impl std::future::Future<Output = T>) -> T {
    tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap().block_on(future)
}

/// Removes an entry a test left in the trash.
fn discard(entry: &HistoryDeletedItem) {
    let Some(content) = entry.content() else { return };
    let trash = content.parent().unwrap().parent().unwrap();
    let mut record = content.file_name().unwrap().to_os_string();
    record.push(".trashinfo");
    let _ = std::fs::remove_file(&content);
    let _ = std::fs::remove_file(trash.join("info").join(record));
}

#[test]
fn undo_and_redo_follow_the_entry_of_each_round() {
    let root = folder();
    let path = root.path().join("doc.txt");
    std::fs::write(&path, b"mine").unwrap();
    let entry = trashed(&path);
    let first = entry.content().unwrap();
    let entries = vec![entry];
    run(undo(&entries)).unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), b"mine");
    assert_eq!(entries[0].content(), None);
    redo(&entries).unwrap();
    assert!(!path.exists());
    assert!(entries[0].content().is_some_and(|content| content.exists()));
    run(undo(&entries)).unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), b"mine");
    assert!(!first.exists());
}

#[test]
fn undo_leaves_another_item_trashed_from_the_same_place() {
    let root = folder();
    let path = root.path().join("doc.txt");
    std::fs::write(&path, b"mine").unwrap();
    let entries = vec![trashed(&path)];
    // The trash is emptied elsewhere, then another program trashes a new doc.txt.
    let content = entries[0].content().unwrap();
    let mut record = content.file_name().unwrap().to_os_string();
    record.push(".trashinfo");
    std::fs::remove_file(&content).unwrap();
    std::fs::remove_file(content.parent().unwrap().parent().unwrap().join("info").join(record)).unwrap();
    std::fs::write(&path, b"foreign").unwrap();
    let foreign = trashed(&path);
    let error = run(undo(&entries)).unwrap_err();
    assert!(error.to_string().contains("doc.txt"), "{error}");
    assert!(!path.exists());
    assert!(foreign.content().is_some_and(|content| content.exists()));
    discard(&foreign);
}

#[test]
fn redo_keeps_a_replacement_of_the_restored_item() {
    let root = folder();
    let path = root.path().join("doc.txt");
    std::fs::write(&path, b"mine").unwrap();
    let entries = vec![trashed(&path)];
    run(undo(&entries)).unwrap();
    std::fs::rename(&path, root.path().join("moved")).unwrap();
    std::fs::write(&path, b"replacement").unwrap();
    assert!(redo(&entries).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), b"replacement");
}
