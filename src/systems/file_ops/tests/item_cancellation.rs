use super::*;
use std::sync::{mpsc, Arc, LazyLock, Mutex};
use std::time::Duration;

struct Pause {
    path: PathBuf,
    arrived: mpsc::Sender<()>,
    resume: mpsc::Receiver<()>,
}

static PAUSES: LazyLock<Mutex<HashMap<String, Pause>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

pub(super) fn pause_copy(id: &str, path: &Path, bytes: u64) {
    if bytes == 0 {
        return;
    }
    let pause = {
        let mut pauses = PAUSES.lock().unwrap();
        if pauses.get(id).is_some_and(|pause| pause.path == path) {
            pauses.remove(id)
        } else {
            None
        }
    };
    if let Some(pause) = pause {
        let _ = pause.arrived.send(());
        let _ = pause.resume.recv_timeout(Duration::from_secs(30));
    }
}

struct Registration(String);
impl Drop for Registration {
    fn drop(&mut self) {
        PAUSES.lock().unwrap().remove(&self.0);
    }
}

#[path = "item_cancellation/speed_history.rs"]
mod speed_history_tests;

#[test]
fn cancelled_second_file_is_interrupted_after_staging_cleanup_not_partial() {
    let temp = tempfile::tempdir_in(std::env::current_dir().unwrap().join("target")).unwrap();
    let source = temp.path().join("source");
    let target = temp.path().join("target");
    std::fs::create_dir(&source).unwrap();
    std::fs::create_dir(&target).unwrap();
    let sources: Vec<_> = (0..5)
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
    let cancel = operation.lock().unwrap().cancel_flag.clone();
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
    let worker_op = operation.clone();
    let worker_sources = sources.clone();
    let worker_target = target.clone();
    let worker = std::thread::spawn(move || {
        FileOperationManager::perform_copy(
            &worker_op,
            worker_sources,
            Some(worker_target),
            false,
            None,
        )
    });
    wait.recv_timeout(Duration::from_secs(30))
        .expect("second copy must report transferred bytes");
    assert!(target.join("file-0.txt").exists());
    cancel.store(true, std::sync::atomic::Ordering::Relaxed);
    resume.send(()).unwrap();
    let result = worker.join().unwrap();
    let mut op = operation.lock().unwrap();
    FileOperationManager::finish_status(
        &mut op,
        match result {
            Ok(()) => OpStatus::Completed,
            Err(error) => OpStatus::Error(error),
        },
    );
    assert_eq!(op.status, OpStatus::Cancelled);
    let outcomes = op.snapshot_item_outcomes();
    assert_eq!(outcomes[&sources[0]], ItemOutcome::Completed);
    assert_eq!(outcomes[&sources[1]], ItemOutcome::Interrupted);
    for path in &sources[2..] {
        assert_eq!(outcomes[path], ItemOutcome::NotProcessed);
    }
    assert!(!target.join("file-1.txt").exists());
    assert_eq!(
        std::fs::read_dir(&target).unwrap().count(),
        1,
        "only the completed file remains; staging is cleaned"
    );
}
