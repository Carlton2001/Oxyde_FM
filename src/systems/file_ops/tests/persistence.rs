use super::*;

#[test]
fn interrupted_outcome_survives_restart_and_ambiguous_old_partial_becomes_unknown() {
    let dir = tempfile::tempdir_in(std::env::current_dir().unwrap().join("target")).unwrap();
    let path = dir.path().join("operation_history.json");
    let manager = FileOperationManager::with_history_path(path.clone());
    let source = PathBuf::from("file.txt");
    let mut op = FileOperation::new(FileOpType::Copy, vec![source.clone()], None, None);
    op.item_plan(&source, "target/file.txt".into(), 1, false, false);
    op.item_started(&source);
    op.status = OpStatus::Cancelled;
    let id = op.id.clone();
    manager
        .operations
        .lock()
        .unwrap()
        .insert(id.clone(), Arc::new(Mutex::new(op)));
    drop(manager);
    let manager = FileOperationManager::with_history_path(path.clone());
    assert_eq!(
        manager.get_operation(&id).unwrap().item_outcomes[&source],
        ItemOutcome::Interrupted
    );
    drop(manager);
    let mut legacy: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    legacy[0]
        .as_object_mut()
        .unwrap()
        .remove("item_outcomes_version");
    legacy[0]["item_outcomes"]["file.txt"] = serde_json::Value::String("Partial".into());
    std::fs::write(&path, serde_json::to_vec(&legacy).unwrap()).unwrap();
    let manager = FileOperationManager::with_history_path(path);
    assert_eq!(
        manager.get_operation(&id).unwrap().item_outcomes[&source],
        ItemOutcome::Unknown
    );
}

#[test]
fn receipts_encode_stably_and_legacy_records_load_as_unknown() {
    let manager = FileOperationManager::new();
    let mut op = FileOperation::new(
        FileOpType::Copy,
        vec!["a".into(), "b".into(), "c".into()],
        None,
        None,
    );
    op.status = OpStatus::Cancelled;
    let id = op.id.clone();
    manager
        .operations
        .lock()
        .unwrap()
        .insert(id.clone(), Arc::new(Mutex::new(op)));
    let bytes = encode(&manager.operations).unwrap();
    for _ in 0..16 {
        assert_eq!(encode(&manager.operations).unwrap(), bytes);
    }
    let mut legacy: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    legacy[0].as_object_mut().unwrap().remove("item_outcomes");
    let dir = tempfile::tempdir_in(std::env::current_dir().unwrap().join("target")).unwrap();
    let path = dir.path().join("operation_history.json");
    std::fs::write(&path, serde_json::to_vec(&legacy).unwrap()).unwrap();
    let restored = FileOperationManager::with_history_path(path);
    let snapshot = restored.get_operation(&id).unwrap();
    assert!(snapshot
        .item_outcomes
        .values()
        .all(|outcome| *outcome == ItemOutcome::Unknown));
}

#[test]
fn restores_history_between_sessions_and_persists_clear() {
    let dir = tempfile::tempdir_in(std::env::current_dir().unwrap().join("target")).unwrap();
    let path = dir.path().join("operation_history.json");
    let manager = FileOperationManager::with_history_path(path.clone());
    let mut op = FileOperation::new(
        FileOpType::Trash,
        vec![PathBuf::from("example.txt")],
        None,
        None,
    );
    op.sequence = 7;
    op.status = OpStatus::Completed;
    op.duration_ms = Some(123);
    op.item_complete(std::path::Path::new("example.txt"));
    let id = op.id.clone();
    manager
        .operations
        .lock()
        .unwrap()
        .insert(id.clone(), Arc::new(Mutex::new(op)));
    drop(manager);
    let bytes = std::fs::read(&path).unwrap();
    let saved: Vec<Record> = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(saved.len(), 1, "{}", String::from_utf8_lossy(&bytes));
    let manager = FileOperationManager::with_history_path(path.clone());
    let restored = manager.get_operation(&id).unwrap();
    assert_eq!(restored.status, OpStatus::Completed);
    assert_eq!(restored.duration_ms, Some(123));
    assert_eq!(
        restored.item_outcomes[std::path::Path::new("example.txt")],
        ItemOutcome::Completed
    );
    assert_eq!(restored.sources, vec![PathBuf::from("example.txt")]);
    assert_eq!(manager.next_sequence.load(Ordering::Relaxed), 8);
    assert_eq!(manager.dismiss_finished_operations(), 1);
    drop(manager);
    let manager = FileOperationManager::with_history_path(path);
    assert!(manager.list_operations().is_empty());
}

#[test]
fn keeps_ten_latest_final_operations_and_observes_dismissal() {
    let manager = FileOperationManager::new();
    for sequence in 1..=15 {
        let mut op = FileOperation::new(
            FileOpType::Copy,
            vec![PathBuf::from("source")],
            Some(PathBuf::from("target")),
            None,
        );
        op.id = sequence.to_string();
        op.sequence = sequence;
        op.status = if sequence == 15 {
            OpStatus::Running
        } else {
            OpStatus::Error("test error".into())
        };
        op.duration_ms = Some(42);
        manager
            .operations
            .lock()
            .unwrap()
            .insert(op.id.clone(), Arc::new(Mutex::new(op)));
    }
    let records: Vec<Record> =
        serde_json::from_slice(&encode(&manager.operations).unwrap()).unwrap();
    assert_eq!(records.len(), 10);
    assert_eq!(records.first().unwrap().sequence, 5);
    assert_eq!(records.last().unwrap().sequence, 14);
    assert_eq!(records[0].duration_ms, Some(42));
    assert_eq!(records[0].status, OpStatus::Error("test error".into()));
    assert!(manager.dismiss_operation("14"));
    let records: Vec<Record> =
        serde_json::from_slice(&encode(&manager.operations).unwrap()).unwrap();
    assert!(records.iter().all(|record| record.id != "14"));
    manager.dismiss_finished_operations();
    let records: Vec<Record> =
        serde_json::from_slice(&encode(&manager.operations).unwrap()).unwrap();
    assert!(records.is_empty());
}
