//! A move to the trash journals each item that reached it, even when another fails (M54).
use super::*;
use std::os::unix::fs::PermissionsExt;

#[test]
fn undo_restores_the_items_trashed_before_a_refusal() {
    if rustix::process::geteuid().is_root() {
        return;
    }
    let data = dirs::data_dir().unwrap();
    std::fs::create_dir_all(&data).unwrap();
    let root = tempfile::tempdir_in(data).unwrap();
    let trashed = root.path().join("trashed.txt");
    let locked = root.path().join("locked");
    std::fs::create_dir(&locked).unwrap();
    std::fs::write(&trashed, b"undo me").unwrap();
    std::fs::write(locked.join("refused.txt"), b"stays").unwrap();
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o555)).unwrap();
    let history = Arc::new(HistoryManager::new());
    let op = Arc::new(Mutex::new(FileOperation::new(
        FileOpType::Trash, vec![trashed.clone(), locked.join("refused.txt")], None, None)));
    let runtime = tokio::runtime::Builder::new_multi_thread().enable_all().build().unwrap();
    runtime.block_on(FileOperationManager::execute_operation(history.clone(), op.clone()));
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert!(matches!(op.lock().unwrap().status, OpStatus::Error(_)));
    assert!(!trashed.exists());
    assert_eq!(history.get_state().undo_stack.len(), 1);
    runtime.block_on(crate::commands::ops::undo_last_action(&history)).unwrap();
    assert_eq!(std::fs::read(&trashed).unwrap(), b"undo me");
    assert_eq!(std::fs::read(locked.join("refused.txt")).unwrap(), b"stays");
}
