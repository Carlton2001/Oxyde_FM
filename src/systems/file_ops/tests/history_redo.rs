use super::*;
use crate::commands::ops::{redo_last_action_shown, undo_last_action};
use std::path::Path;
use std::time::Duration;

struct Fixture {
    _root: tempfile::TempDir,
    source: PathBuf,
    copied: PathBuf,
    manager: Arc<FileOperationManager>,
    history: Arc<HistoryManager>,
    runtime: tokio::runtime::Runtime,
}

fn until_final(manager: &FileOperationManager, id: &str) -> OpStatus {
    let started = std::time::Instant::now();
    loop {
        let status = manager.get_operation(id).unwrap().status;
        if FileOperationManager::is_final(&status) {
            return status;
        }
        assert!(started.elapsed() < Duration::from_secs(30), "{status:?}");
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// A folder of two files copied, the copy then undone.
fn undone_copy(size: usize) -> Fixture {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("docs");
    let target = root.path().join("target");
    std::fs::create_dir_all(source.join("nested")).unwrap();
    std::fs::write(source.join("note.txt"), b"note").unwrap();
    std::fs::write(source.join("nested/large.bin"), vec![7u8; size]).unwrap();
    std::fs::create_dir(&target).unwrap();
    let runtime = tokio::runtime::Builder::new_multi_thread().enable_all().build().unwrap();
    let manager = Arc::new(FileOperationManager::new());
    let history = Arc::new(HistoryManager::new());
    let id = manager.queue_operation_on(
        runtime.handle(),
        history.clone(),
        FileOperation::new(FileOpType::Copy, vec![source.clone()], Some(target.clone()), None),
    );
    assert_eq!(until_final(&manager, &id), OpStatus::Completed);
    let copied = target.join("docs");
    assert!(copied.join("nested/large.bin").is_file());
    runtime.block_on(undo_last_action(&history)).unwrap();
    assert!(!copied.exists());
    Fixture { _root: root, source, copied, manager, history, runtime }
}

impl Fixture {
    /// Redoes in a panel; `queued` receives the copy's id as soon as it is queued.
    fn redo(&self, queued: impl FnOnce(&FileOperationManager, &str)) -> (Result<Option<crate::models::Transaction>, crate::models::CommandError>, String) {
        let (sender, receiver) = futures::channel::oneshot::channel();
        let panel = RedoCopyPanel {
            manager: self.manager.clone(),
            runtime: self.runtime.handle().clone(),
            history: self.history.clone(),
            turbo: false,
            queued: Mutex::new(Some(sender)),
        };
        let history = self.history.clone();
        let handle = self.runtime.handle().clone();
        let redo = std::thread::spawn(move || handle.block_on(redo_last_action_shown(&history, &panel)));
        let id = futures::executor::block_on(receiver).unwrap();
        queued(&self.manager, &id);
        (redo.join().unwrap(), id)
    }
    fn stacks(&self) -> (usize, usize) {
        let state = self.history.get_state();
        (state.undo_stack.len(), state.redo_stack.len())
    }
}

#[test]
fn a_redone_copy_runs_in_the_panel_and_records_no_entry_of_its_own() {
    let fixture = undone_copy(256 * 1024);
    let (result, id) = fixture.redo(|_, _| {});
    assert!(result.unwrap().is_some());
    let operation = fixture.manager.get_operation(&id).unwrap();
    assert_eq!(operation.status, OpStatus::Completed);
    assert_eq!(operation.op_type, FileOpType::Copy);
    assert_eq!(operation.sources, [fixture.source.clone()]);
    assert_eq!((operation.processed_files, operation.total_files), (2, 2));
    assert_eq!(operation.processed_bytes, operation.total_bytes);
    assert_eq!(operation.total_bytes, 256 * 1024 + 4);
    assert_eq!(std::fs::read(fixture.copied.join("note.txt")).unwrap(), b"note");
    assert_eq!(fixture.stacks(), (1, 0));
    assert_eq!(fixture.manager.list_operations().len(), 2);
    // The entry the Redo moved undoes as before.
    fixture.runtime.block_on(undo_last_action(&fixture.history)).unwrap();
    assert!(!fixture.copied.exists());
}

#[test]
fn a_redone_copy_stopped_from_the_panel_leaves_nothing_and_stays_to_redo() {
    let fixture = undone_copy(64 * 1024 * 1024);
    let (result, id) = fixture.redo(|manager, id| {
        assert!(manager.set_paused(id, true));
        assert!(manager.cancel_operation(id));
    });
    assert!(result.is_err());
    assert_eq!(fixture.manager.get_operation(&id).unwrap().status, OpStatus::Cancelled);
    assert!(!fixture.copied.exists());
    assert_eq!(fixture.stacks(), (0, 1));
    assert!(Path::new(&fixture.source).join("nested/large.bin").is_file());
}
