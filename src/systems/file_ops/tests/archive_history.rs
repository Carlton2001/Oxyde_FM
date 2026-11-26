use super::*;
use crate::commands::ops::undo_last_action;
use crate::models::transaction::{HistoryReach, UndoBlock};
use std::path::Path;
use std::time::Duration;

struct Fixture {
    root: tempfile::TempDir,
    archive: PathBuf,
    manager: FileOperationManager,
    history: Arc<HistoryManager>,
    runtime: tokio::runtime::Runtime,
}

/// `docs/note.txt` and `docs/nested/deep.txt` in a ZIP.
fn fixture() -> Fixture {
    let root = tempfile::tempdir().unwrap();
    let docs = root.path().join("docs");
    std::fs::create_dir_all(docs.join("nested")).unwrap();
    std::fs::write(docs.join("note.txt"), b"note").unwrap();
    std::fs::write(docs.join("nested/deep.txt"), b"deep").unwrap();
    let archive = root.path().join("bundle.zip");
    let compress = Arc::new(Mutex::new(FileOperation::new(FileOpType::Compress, vec![], None, None)));
    FileOperationManager::perform_compress(&compress, vec![docs.clone()], Some(archive.clone())).unwrap();
    std::fs::remove_dir_all(&docs).unwrap();
    Fixture {
        root,
        archive,
        manager: FileOperationManager::new(),
        history: Arc::new(HistoryManager::new()),
        runtime: tokio::runtime::Builder::new_multi_thread().enable_all().build().unwrap(),
    }
}

impl Fixture {
    fn run(&self, op_type: FileOpType, sources: Vec<PathBuf>, destination: &Path) {
        let id = self.manager.queue_operation_on(
            self.runtime.handle(),
            self.history.clone(),
            FileOperation::new(op_type, sources, Some(destination.to_path_buf()), None),
        );
        let started = std::time::Instant::now();
        loop {
            let status = self.manager.get_operation(&id).unwrap().status;
            if FileOperationManager::is_final(&status) {
                assert_eq!(status, OpStatus::Completed);
                return;
            }
            assert!(started.elapsed() < Duration::from_secs(30));
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    fn folder(&self, name: &str) -> PathBuf {
        let folder = self.root.path().join(name);
        std::fs::create_dir(&folder).unwrap();
        folder
    }
}

#[test]
fn a_copy_out_of_an_archive_is_undone_and_cannot_be_redone() {
    let fixture = fixture();
    let archive = std::fs::read(&fixture.archive).unwrap();
    let output = fixture.folder("output");
    fixture.run(FileOpType::Copy, vec![fixture.archive.join("docs")], &output);
    assert_eq!(std::fs::read(output.join("docs/nested/deep.txt")).unwrap(), b"deep");
    let state = fixture.history.get_state();
    assert_eq!(state.undo_stack.len(), 1);
    assert_eq!(state.undo_stack[0].reach, HistoryReach::UndoOnly);
    assert_eq!(fixture.history.summary().undo_blocked, None);
    fixture.runtime.block_on(undo_last_action(&fixture.history)).unwrap();
    assert_eq!(std::fs::read_dir(&output).unwrap().count(), 0);
    let state = fixture.history.get_state();
    assert_eq!((state.undo_stack.len(), state.redo_stack.len()), (0, 0));
    assert_eq!(std::fs::read(&fixture.archive).unwrap(), archive);
}

#[test]
fn undo_after_an_archive_operation_stops_there_instead_of_reaching_an_older_action() {
    let fixture = fixture();
    let first = fixture.folder("first");
    let added = fixture.root.path().join("added.txt");
    std::fs::write(&added, b"added").unwrap();
    fixture.run(FileOpType::Copy, vec![added.clone()], &first);
    // Into the archive, then out of it with a move: neither is reversed by history.
    fixture.run(FileOpType::Copy, vec![added.clone()], &fixture.archive.join("docs"));
    let moved = fixture.folder("moved");
    fixture.run(FileOpType::Move, vec![fixture.archive.join("docs/note.txt")], &moved);
    assert_eq!(std::fs::read(moved.join("note.txt")).unwrap(), b"note");
    let state = fixture.history.get_state();
    assert_eq!(state.undo_stack.iter().map(|tx| tx.reach).collect::<Vec<_>>(), [HistoryReach::UndoAndRedo, HistoryReach::Barrier, HistoryReach::Barrier]);
    assert_eq!(fixture.history.summary().undo_blocked, Some(UndoBlock::Archive));
    assert!(fixture.runtime.block_on(undo_last_action(&fixture.history)).is_err());
    assert_eq!(std::fs::read(first.join("added.txt")).unwrap(), b"added");
    assert_eq!(std::fs::read(moved.join("note.txt")).unwrap(), b"note");
    assert_eq!(fixture.history.get_state().undo_stack.len(), 3);
}
