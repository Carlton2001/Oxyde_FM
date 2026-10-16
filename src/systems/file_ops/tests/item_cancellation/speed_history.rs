use super::*;
use crate::systems::file_ops::SPEED_HISTORY_LIMIT;

#[test]
fn finalization_keeps_speed_samples_after_success_cancellation_and_late_error() {
    for outcome in ["success", "cancel", "error"] {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("source");
        let target = temp.path().join("target");
        std::fs::create_dir(&source).unwrap();
        std::fs::create_dir(&target).unwrap();
        let sources: Vec<_> = (0..3)
            .map(|index| source.join(format!("file-{index}.txt")))
            .collect();
        for path in &sources {
            std::fs::write(path, vec![42; 4096]).unwrap();
        }
        let operation = Arc::new(Mutex::new(FileOperation::new(
            FileOpType::Copy,
            sources.clone(),
            Some(target.clone()),
            None,
        )));
        let id = operation.lock().unwrap().id.clone();
        let (arrived, wait) = mpsc::channel();
        let (resume, release) = mpsc::channel();
        PAUSES.lock().unwrap().insert(
            id.clone(),
            Pause {
                path: sources[1].clone(),
                arrived,
                resume: release,
            },
        );
        let _registration = Registration(id);
        let op = operation.clone();
        let worker = std::thread::spawn(move || {
            tokio::runtime::Runtime::new().unwrap().block_on(
                FileOperationManager::execute_operation(
                    Arc::new(crate::models::HistoryManager::new()),
                    op,
                ),
            );
        });
        wait.recv_timeout(Duration::from_secs(15))
            .expect("second file reached real transfer progress");
        assert_eq!(
            std::fs::read(target.join("file-0.txt")).unwrap(),
            vec![42; 4096]
        );
        let bytes = Arc::new(std::sync::atomic::AtomicU64::new(4096));
        let files = Arc::new(std::sync::atomic::AtomicUsize::new(1));
        let mut last_emit = std::time::Instant::now() - Duration::from_millis(600);
        FileOperationManager::emit_progress(
            &operation,
            &bytes,
            &files,
            &mut 0,
            &mut last_emit,
            &mut std::collections::VecDeque::new(),
        );
        let sample = *operation.lock().unwrap().speed_history.last().unwrap();
        assert!(sample > 0);
        if outcome == "cancel" {
            operation
                .lock()
                .unwrap()
                .cancel_flag
                .store(true, std::sync::atomic::Ordering::Relaxed);
        }
        if outcome == "error" {
            std::fs::write(target.join("file-2.txt"), b"late foreign").unwrap();
        }
        resume.send(()).unwrap();
        worker.join().unwrap();
        let operation = operation.lock().unwrap();
        match outcome {
            "success" => assert_eq!(operation.status, OpStatus::Completed),
            "cancel" => assert_eq!(operation.status, OpStatus::Cancelled),
            _ => {
                assert!(matches!(operation.status, OpStatus::Error(_)));
                assert_eq!(
                    std::fs::read(target.join("file-2.txt")).unwrap(),
                    b"late foreign"
                );
            }
        }
        assert!(
            operation.speed_history.contains(&sample),
            "{outcome}: finalization erased the graph"
        );
        assert!(operation.speed_history.len() <= SPEED_HISTORY_LIMIT);
        assert_eq!(operation.bytes_per_second, 0);
        assert!(operation.speed_sample_at.is_none());
    }
}
