use super::*;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap()
}

fn rename(runtime: &tokio::runtime::Runtime, history: &HistoryManager, old: &std::path::Path, new: &std::path::Path) {
    let (old, new) = (old.to_string_lossy().into_owned(), new.to_string_lossy().into_owned());
    let proof = runtime
        .block_on(crate::commands::io::rename_item_controlled_with_identity(
            old.clone(), new.clone(), "normal".into(), Arc::new(AtomicBool::new(false)),
        ))
        .unwrap()
        .unwrap();
    crate::commands::io::record_captured_rename(history, &old, &new, proof);
}

#[test]
fn a_rename_is_undone_and_redone() {
    let root = tempfile::tempdir().unwrap();
    let old = root.path().join("report.txt");
    let new = root.path().join("Rapport final.txt");
    std::fs::write(&old, b"content").unwrap();
    let history = HistoryManager::new();
    let runtime = runtime();
    rename(&runtime, &history, &old, &new);
    assert_eq!(history.summary().undo, Some(TransactionType::Rename));

    runtime.block_on(undo_last_action(&history)).unwrap().unwrap();
    assert!(old.exists() && !new.exists());
    assert_eq!(history.summary().redo, Some(TransactionType::Rename));

    runtime.block_on(redo_last_action(&history)).unwrap().unwrap();
    assert!(!old.exists() && new.exists());
    assert_eq!(std::fs::read(&new).unwrap(), b"content");
}

#[test]
fn undo_refuses_an_item_replaced_since_the_rename() {
    let root = tempfile::tempdir().unwrap();
    let old = root.path().join("a");
    let new = root.path().join("b");
    std::fs::write(&old, b"first").unwrap();
    let history = HistoryManager::new();
    let runtime = runtime();
    rename(&runtime, &history, &old, &new);
    std::fs::remove_file(&new).unwrap();
    std::fs::write(&new, b"another").unwrap();

    assert!(runtime.block_on(undo_last_action(&history)).is_err());
    assert!(!old.exists());
    assert_eq!(std::fs::read(&new).unwrap(), b"another");
}
