use super::*;
use crate::systems::file_ops::FileOperation;
use std::sync::{Arc, Mutex};

#[test]
fn main_window_queries_exclude_final_history_and_preserve_active_order_and_progress() {
    let manager = FileOperationManager::new();
    let mut active_ids = Vec::new();
    for (sequence, status) in [
        OpStatus::Completed,
        OpStatus::Running,
        OpStatus::Cancelled,
        OpStatus::Paused,
        OpStatus::Error("failure".into()),
        OpStatus::Queued,
        OpStatus::Calculating,
    ]
    .into_iter()
    .enumerate()
    {
        let mut operation = FileOperation::new(
            FileOpType::Copy,
            (0..1000)
                .map(|index| format!("file-{index}").into())
                .collect(),
            None,
            None,
        );
        operation.sequence = sequence as u64;
        operation.status = status.clone();
        operation.total_bytes = 200;
        operation.processed_bytes = sequence as u64;
        operation.total_files = 1000;
        operation.processed_files = sequence;
        if !FileOperationManager::is_final(&status) {
            active_ids.push(operation.id.clone());
        }
        assert!(manager.operation_state(&operation.id).is_none());
        let id = operation.id.clone();
        manager
            .operations
            .lock()
            .unwrap()
            .insert(id.clone(), Arc::new(Mutex::new(operation)));
        assert_eq!(manager.operation_state(&id).unwrap().1, status);
    }
    assert_eq!(manager.operation_counts(), (7, 4));
    let progress = manager.list_active_progress();
    assert_eq!(
        progress
            .iter()
            .map(|operation| operation.id.clone())
            .collect::<Vec<_>>(),
        active_ids
    );
    assert!(progress
        .iter()
        .all(|operation| operation.total_bytes == 200 && operation.total_files == 1000));
    assert_eq!(progress[0].processed_bytes, 1);
    assert_eq!(progress[0].processed_files, 1);
    assert_eq!(manager.dismiss_finished_operations(), 3);
    assert_eq!(manager.operation_counts(), (4, 4));
}

#[test]
fn an_idle_history_returns_no_progress_without_removing_file_details() {
    let manager = FileOperationManager::new();
    let source = std::path::PathBuf::from("source");
    let mut operation = FileOperation::new(FileOpType::Trash, vec![source.clone()], None, None);
    operation.item_complete(&source);
    operation.status = OpStatus::Cancelled;
    let id = operation.id.clone();
    manager
        .operations
        .lock()
        .unwrap()
        .insert(id.clone(), Arc::new(Mutex::new(operation)));
    assert_eq!(manager.operation_counts(), (1, 0));
    assert!(manager.list_active_progress().is_empty());
    assert_eq!(
        manager.get_operation(&id).unwrap().sources,
        vec![source.clone()]
    );
    assert_eq!(
        manager.get_operation(&id).unwrap().item_outcomes[&source],
        crate::systems::file_ops::ItemOutcome::Completed
    );
}

#[test]
fn the_operations_window_reads_details_states_and_items_apart() {
    let manager = FileOperationManager::new();
    let sources: Vec<std::path::PathBuf> = ["b", "a", "c"].iter().map(std::path::PathBuf::from).collect();
    let mut operation = FileOperation::new(FileOpType::Copy, sources.clone(), Some("target".into()), None);
    operation.item_plan(&sources[1], "target/a (2)".into(), 1, false, false);
    operation.item_done(&sources[1]);
    operation.item_started(&sources[2]);
    operation.status = OpStatus::Running;
    operation.speed_history = vec![1, 2];
    let id = operation.id.clone();
    manager
        .operations
        .lock()
        .unwrap()
        .insert(id.clone(), Arc::new(Mutex::new(operation)));

    let detail = manager.operation_detail(&id).unwrap();
    assert_eq!(detail.source_count, 3);
    assert_eq!(detail.first_source, Some("a".into()));
    assert_eq!(detail.destination, Some("target".into()));
    assert_eq!(detail.speed_history, [1, 2]);

    // In the order of the sources, as the window's rows.
    let states = manager.operation_item_states(&id).unwrap();
    use crate::systems::file_ops::ItemOutcome;
    assert_eq!(states.outcomes, [ItemOutcome::Pending, ItemOutcome::Completed, ItemOutcome::InProgress]);
    assert!(!states.settled);

    let items = manager.operation_items(&id).unwrap();
    assert_eq!(items.sources, sources);
    assert_eq!(items.destinations[&sources[1]], std::path::PathBuf::from("target/a (2)"));

    manager.operations.lock().unwrap()[&id].lock().unwrap().status = OpStatus::Cancelled;
    let states = manager.operation_item_states(&id).unwrap();
    assert!(states.settled);
    assert_eq!(states.outcomes[0], ItemOutcome::NotProcessed);
    assert!(manager.operation_item_states("missing").is_none());
}
