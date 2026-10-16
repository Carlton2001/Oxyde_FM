use super::*;

#[test]
fn overlapping_paths_are_serialized_without_blocking_independent_paths() {
    let temp = tempfile::tempdir().unwrap();
    let destination = temp.path().join("target");
    let first = FileOperation::new(
        FileOpType::Copy,
        vec![temp.path().join("source-a")],
        Some(destination.clone()),
        None,
    );
    let same_destination = FileOperation::new(
        FileOpType::Move,
        vec![temp.path().join("source-b")],
        Some(destination.join("source-a/nested")),
        None,
    );
    let independent = FileOperation::new(
        FileOpType::Copy,
        vec![temp.path().join("source-c")],
        Some(destination),
        None,
    );
    assert!(FileOperationManager::paths_overlap(
        &FileOperationManager::reserved_paths(&first),
        &FileOperationManager::reserved_paths(&same_destination)
    ));
    assert!(!FileOperationManager::paths_overlap(
        &FileOperationManager::reserved_paths(&first),
        &FileOperationManager::reserved_paths(&independent)
    ));
}

#[test]
fn controls_and_dismissal_target_only_one_operation() {
    let manager = FileOperationManager::new();
    let mut first = FileOperation::new(FileOpType::Copy, Vec::new(), None, None);
    let mut second = FileOperation::new(FileOpType::Move, Vec::new(), None, None);
    first.sequence = 1;
    second.sequence = 2;
    let first_id = first.id.clone();
    let second_id = second.id.clone();
    {
        let mut ops = manager.operations.lock().unwrap();
        ops.insert(first_id.clone(), Arc::new(Mutex::new(first)));
        ops.insert(second_id.clone(), Arc::new(Mutex::new(second)));
    }
    assert!(manager.set_paused(&first_id, true));
    manager.set_turbo(&first_id, true);
    assert_eq!(
        manager.get_operation(&first_id).unwrap().status,
        OpStatus::Paused
    );
    assert!(manager.get_operation(&first_id).unwrap().turbo);
    assert!(manager.get_operation(&first_id).unwrap().turbo_flag.load(Ordering::Relaxed));
    assert_eq!(
        manager.get_operation(&second_id).unwrap().status,
        OpStatus::Queued
    );
    assert!(!manager.get_operation(&second_id).unwrap().turbo);
    assert!(!manager.get_operation(&second_id).unwrap().turbo_flag.load(Ordering::Relaxed));
    assert!(manager.cancel_operation(&second_id));
    assert_eq!(
        manager.get_operation(&second_id).unwrap().status,
        OpStatus::Cancelled
    );
    assert!(manager.dismiss_operation(&second_id));
    assert_eq!(manager.list_operations().len(), 1);
    assert!(!manager.dismiss_operation(&first_id));
}

#[test]
fn dismiss_finished_operations_preserves_running_operations() {
    let manager = FileOperationManager::new();
    let mut completed = FileOperation::new(FileOpType::Copy, Vec::new(), None, None);
    let mut running = FileOperation::new(FileOpType::Move, Vec::new(), None, None);
    completed.status = OpStatus::Completed;
    running.status = OpStatus::Running;
    let completed_id = completed.id.clone();
    let running_id = running.id.clone();
    {
        let mut operations = manager.operations.lock().unwrap();
        operations.insert(completed_id.clone(), Arc::new(Mutex::new(completed)));
        operations.insert(running_id.clone(), Arc::new(Mutex::new(running)));
    }

    assert_eq!(manager.dismiss_finished_operations(), 1);
    assert!(manager.get_operation(&completed_id).is_none());
    assert_eq!(
        manager.get_operation(&running_id).unwrap().status,
        OpStatus::Running
    );
}

#[test]
fn destination_created_while_queued_requires_a_new_conflict_decision() {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("item.txt");
        let target = temp.path().join("target");
        std::fs::create_dir(&target).unwrap();
        std::fs::write(&source, b"source").unwrap();
        let manager = FileOperationManager::new();
        let held_slots = manager
            .scheduler
            .slots
            .clone()
            .acquire_many_owned(2)
            .await
            .unwrap();
        let id = manager.queue_operation_on(
            &tokio::runtime::Handle::current(),
            Arc::new(HistoryManager::new()),
            FileOperation::new(
                FileOpType::Copy,
                vec![source.clone()],
                Some(target.clone()),
                None,
            ),
        );
        std::fs::write(target.join("item.txt"), b"existing").unwrap();
        drop(held_slots);
        let status = tokio::time::timeout(std::time::Duration::from_secs(5), async {
            loop {
                let status = manager.get_operation(&id).unwrap().status;
                if FileOperationManager::is_final(&status) {
                    break status;
                }
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
        assert!(matches!(status, OpStatus::Error(_)));
        assert_eq!(std::fs::read(&source).unwrap(), b"source");
        assert_eq!(std::fs::read(target.join("item.txt")).unwrap(), b"existing");
    });
}

#[test]
fn paused_operations_do_not_hold_capacity_and_queued_launch_stays_queued_until_started() {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        let temp = tempfile::tempdir().unwrap();
        let manager = FileOperationManager::new();
        let history = Arc::new(HistoryManager::new());
        let mut ids = Vec::new();
        for index in 0..3 {
            let source_dir = temp.path().join(format!("source-{index}"));
            let target_dir = temp.path().join(format!("target-{index}"));
            std::fs::create_dir(&source_dir).unwrap();
            std::fs::create_dir(&target_dir).unwrap();
            let source = source_dir.join("item.txt");
            std::fs::write(&source, b"copy me").unwrap();
            let op = FileOperation::new(FileOpType::Copy, vec![source], Some(target_dir), None);
            op.pause_flag.store(true, Ordering::Relaxed);
            ids.push(manager.queue_operation_on(
                &tokio::runtime::Handle::current(),
                history.clone(),
                op,
            ));
        }
        tokio::time::timeout(std::time::Duration::from_secs(5), async {
            loop {
                let statuses: Vec<_> = ids
                    .iter()
                    .map(|id| manager.get_operation(id).unwrap().status)
                    .collect();
                if statuses.iter().all(|status| *status == OpStatus::Paused) {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
        let held_slots = manager
            .scheduler
            .slots
            .clone()
            .acquire_many_owned(2)
            .await
            .unwrap();
        assert!(manager.set_paused(&ids[2], false));
        assert_eq!(
            manager.get_operation(&ids[2]).unwrap().status,
            OpStatus::Queued
        );
        drop(held_slots);
        tokio::time::timeout(std::time::Duration::from_secs(5), async {
            loop {
                if manager.get_operation(&ids[2]).unwrap().status == OpStatus::Completed {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
        assert_eq!(
            manager.get_operation(&ids[0]).unwrap().status,
            OpStatus::Paused
        );
        assert_eq!(
            manager.get_operation(&ids[1]).unwrap().status,
            OpStatus::Paused
        );
        for id in &ids {
            manager.cancel_operation(id);
        }
    });
}

async fn wait_for_status(manager: &FileOperationManager, id: &str, expected: OpStatus) {
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            let status = manager.get_operation(id).unwrap().status;
            if status == expected {
                break;
            }
            assert!(
                !matches!(status, OpStatus::Error(_)),
                "Unexpected failure: {status:?}"
            );
            tokio::time::sleep(std::time::Duration::from_millis(1)).await;
        }
    })
    .await
    .unwrap();
}

#[test]
fn pausing_an_active_copy_starts_the_next_copy_but_protects_the_paused_paths() {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("large.bin");
        let file = std::fs::File::create(&source).unwrap();
        file.set_len(32 * 1024 * 1024).unwrap();
        drop(file);
        let other = temp.path().join("small.txt");
        std::fs::write(&other, b"next operation").unwrap();
        let target = temp.path().join("target");
        std::fs::create_dir(&target).unwrap();
        let manager = FileOperationManager::new();
        let history = Arc::new(HistoryManager::new());
        // Leave only one execution slot, so advancing the queue requires releasing it.
        let held = manager
            .scheduler
            .slots
            .clone()
            .acquire_owned()
            .await
            .unwrap();
        let first = manager.queue_operation_on(
            &tokio::runtime::Handle::current(),
            history.clone(),
            FileOperation::new(
                FileOpType::Copy,
                vec![source.clone()],
                Some(target.clone()),
                None,
            ),
        );
        wait_for_status(&manager, &first, OpStatus::Running).await;
        assert!(manager.set_paused(&first, true));
        let next = manager.queue_operation_on(
            &tokio::runtime::Handle::current(),
            history.clone(),
            FileOperation::new(FileOpType::Copy, vec![other], Some(target.clone()), None),
        );
        let overlapping = manager.queue_operation_on(
            &tokio::runtime::Handle::current(),
            history.clone(),
            FileOperation::new(FileOpType::Copy, vec![source], Some(target.clone()), None),
        );
        wait_for_status(&manager, &next, OpStatus::Completed).await;
        assert_eq!(
            std::fs::read(target.join("small.txt")).unwrap(),
            b"next operation"
        );
        assert_eq!(
            manager.get_operation(&first).unwrap().status,
            OpStatus::Paused
        );
        assert_eq!(
            manager.get_operation(&overlapping).unwrap().status,
            OpStatus::Queued
        );
        manager.cancel_operation(&overlapping);
        assert!(manager.set_paused(&first, false));
        wait_for_status(&manager, &first, OpStatus::Completed).await;
        assert_eq!(
            std::fs::metadata(target.join("large.bin")).unwrap().len(),
            32 * 1024 * 1024
        );
        drop(held);
    });
}

#[test]
fn manual_launch_takes_priority_over_earlier_waiting_operations() {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        let manager = FileOperationManager::new();
        let mut ids = Vec::new();
        for sequence in 1..=3 {
            let mut operation = FileOperation::new(FileOpType::Copy, Vec::new(), None, None);
            operation.sequence = sequence;
            let id = operation.id.clone();
            let operation = Arc::new(Mutex::new(operation));
            manager
                .operations
                .lock()
                .unwrap()
                .insert(id.clone(), operation.clone());
            manager.scheduler.enqueue(operation, Vec::new());
            ids.push(id);
        }
        let held = manager
            .scheduler
            .slots
            .clone()
            .acquire_owned()
            .await
            .unwrap();
        assert!(manager.set_paused(&ids[2], false));
        assert_eq!(
            manager.get_operation(&ids[2]).unwrap().status,
            OpStatus::Queued
        );
        let launched = tokio::time::timeout(
            std::time::Duration::from_secs(1),
            manager.scheduler.acquire(&ids[2]),
        )
        .await
        .unwrap()
        .unwrap();
        assert_eq!(
            manager.get_operation(&ids[0]).unwrap().status,
            OpStatus::Queued
        );
        assert_eq!(
            manager.get_operation(&ids[1]).unwrap().status,
            OpStatus::Queued
        );
        drop(launched);
        drop(held);
    });
}
