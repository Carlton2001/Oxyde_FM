//! Pause and cancellation reach a permanent deletion inside the selected folder.
use super::*;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

fn folders(path: &Path) -> usize {
    std::fs::read_dir(path).map_or(0, |entries| entries.count())
}

#[test]
fn a_selected_folder_pauses_and_cancels_between_its_items() {
    let root = tempfile::tempdir().unwrap();
    let selected = root.path().join("tree");
    for index in 0..40 {
        let folder = selected.join(format!("d{index}"));
        std::fs::create_dir_all(&folder).unwrap();
        for file in 0..1000 {
            std::fs::File::create(folder.join(file.to_string())).unwrap();
        }
    }
    let op = Arc::new(Mutex::new(FileOperation::new(FileOpType::Delete, vec![selected.clone()], None, None)));
    let (pause, cancel) = {
        let locked = op.lock().unwrap();
        (locked.pause_flag.clone(), locked.cancel_flag.clone())
    };
    let worker = {
        let (op, selected) = (op.clone(), selected.clone());
        std::thread::spawn(move || FileOperationManager::perform_delete(&op, vec![selected]))
    };
    let deadline = Instant::now() + Duration::from_secs(20);
    while folders(&selected) == 40 {
        assert!(Instant::now() < deadline, "deletion never started");
        std::thread::sleep(Duration::from_millis(1));
    }
    pause.store(true, Ordering::Relaxed);
    std::thread::sleep(Duration::from_millis(100));
    let paused = walk(&selected);
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(walk(&selected), paused, "deletion went on while paused");
    cancel.store(true, Ordering::Relaxed);
    worker.join().unwrap().unwrap();
    assert!(paused > 0 && walk(&selected) == paused, "{paused}");
}

fn walk(path: &Path) -> usize {
    std::fs::read_dir(path)
        .unwrap()
        .map(|entry| {
            let entry = entry.unwrap();
            1 + if entry.file_type().unwrap().is_dir() { walk(&entry.path()) } else { 0 }
        })
        .sum()
}
