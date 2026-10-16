use super::*;

#[test]
fn finalized_graph_roundtrips_through_history_and_is_removed_on_dismissal() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("history.json");
    let source = temp.path().join("source.txt");
    let target = temp.path().join("target");
    std::fs::write(&source, b"selected").unwrap();
    std::fs::create_dir(&target).unwrap();
    let manager = FileOperationManager::with_history_path(path.clone());
    let op = Arc::new(Mutex::new(FileOperation::new(
        FileOpType::Copy,
        vec![source],
        Some(target),
        None,
    )));
    for speed in 0..75 {
        op.lock().unwrap().record_speed(speed);
    }
    assert_eq!(
        op.lock().unwrap().speed_history,
        (15..75).collect::<Vec<u64>>()
    );
    let id = op.lock().unwrap().id.clone();
    manager
        .operations
        .lock()
        .unwrap()
        .insert(id.clone(), op.clone());
    tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(FileOperationManager::execute_operation(
            Arc::new(crate::models::HistoryManager::new()),
            op,
        ));
    // The real copy may append its last speed sample. Persist exactly the
    // finalized rolling window rather than assuming no progress was emitted.
    let expected = manager.get_operation(&id).unwrap().speed_history;
    assert_eq!(expected.len(), super::super::SPEED_HISTORY_LIMIT);
    assert!(
        expected.contains(&74),
        "finalization erased earlier samples"
    );
    drop(manager);
    let manager = FileOperationManager::with_history_path(path.clone());
    let restored = manager.get_operation(&id).unwrap();
    assert_eq!(restored.status, OpStatus::Completed);
    assert_eq!(restored.speed_history, expected);
    assert_eq!(
        restored.speed_history.len(),
        super::super::SPEED_HISTORY_LIMIT
    );
    assert_eq!(restored.bytes_per_second, 0);
    assert!(manager.dismiss_operation(&id));
    drop(manager);
    let manager = FileOperationManager::with_history_path(path);
    assert!(manager.list_operations().is_empty());
}
