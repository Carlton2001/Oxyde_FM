use super::*;
use std::sync::{Arc, Mutex};

#[test]
fn stable_paused_badges_do_not_request_repeated_main_window_notifications() {
    let manager = FileOperationManager::new();
    let mut operation =
        super::super::FileOperation::new(FileOpType::Copy, vec!["source".into()], None, None);
    operation.status = OpStatus::Paused;
    let id = operation.id.clone();
    let op = Arc::new(Mutex::new(operation));
    manager
        .operations
        .lock()
        .unwrap()
        .insert(id.clone(), op.clone());
    let mut previous = None;
    let mut notifications = 0;
    for _ in 0..1000 {
        let current = manager.operation_progress(&id).unwrap();
        notifications += usize::from(previous.as_ref() != Some(&current));
        previous = Some(current);
    }
    assert_eq!(notifications, 1);
    op.lock().unwrap().processed_bytes += 1;
    assert_ne!(previous, manager.operation_progress(&id));
    op.lock().unwrap().status = OpStatus::Running;
    assert_eq!(
        manager.operation_progress(&id).unwrap().status,
        OpStatus::Running
    );
}
