use super::*;
use std::fs;
use std::sync::atomic::AtomicBool;

fn control() -> ShellDeleteControl {
    ShellDeleteControl {
        cancel: Arc::new(AtomicBool::new(false)),
        pause: Arc::new(AtomicBool::new(false)),
        turbo: Arc::new(AtomicBool::new(true)),
    }
}

fn files(root: &Path) -> Vec<(PathBuf, bool, HistoryFileIdentity)> {
    files_with_count(root, 3)
}

fn files_with_count(root: &Path, count: usize) -> Vec<(PathBuf, bool, HistoryFileIdentity)> {
    (0..count)
        .map(|index| {
            let path = root.join(format!("fixture-{index}.txt"));
            fs::write(&path, b"test fixture").unwrap();
            let file = fs::File::open(&path).unwrap();
            let proof = HistoryFileIdentity::from_file(&file).unwrap();
            (path, false, proof) // Permanent deletion of private fixtures only, never the user's trash.
        })
        .collect()
}

#[test]
fn native_file_batch_returns_one_confirmed_receipt_per_fixture() {
    for count in [3, 256] {
        let root = tempfile::tempdir().unwrap();
        let files = files_with_count(root.path(), count);
        let outcomes = run_files(files.clone(), control());
        assert_eq!(outcomes.len(), files.len());
        for ((path, _, _), outcome) in files.iter().zip(outcomes) {
            assert!(outcome.completed, "{path:?}: {:?}", outcome.error);
            assert!(outcome.error.is_none(), "{:?}", outcome.error);
            assert!(!path.exists());
        }
    }
}

#[test]
fn cancellation_before_a_paused_batch_preserves_every_fixture() {
    let root = tempfile::tempdir().unwrap();
    let files = files(root.path());
    let control = control();
    control.pause.store(true, Ordering::Relaxed);
    control.cancel.store(true, Ordering::Relaxed);
    let outcomes = run_files(files.clone(), control);
    assert_eq!(outcomes.len(), files.len());
    assert!(outcomes
        .iter()
        .all(|outcome| !outcome.completed && outcome.error.is_some()));
    assert!(files.iter().all(|(path, _, _)| path.exists()));
}

#[test]
fn wrong_identity_prevents_deleting_the_replaced_item_and_reports_other_receipts() {
    let root = tempfile::tempdir().unwrap();
    let mut files = files(root.path());
    files[0].2 = files[1].2;
    let outcomes = run_files(files.clone(), control());
    assert_eq!(outcomes.len(), files.len());
    assert!(files[0].0.exists());
    assert!(!outcomes[0].completed);
    assert!(outcomes[0].error.is_some());
    for ((path, _, _), outcome) in files.iter().zip(outcomes) {
        assert_eq!(outcome.completed, !path.exists());
    }
}

#[test]
fn callback_cancellation_keeps_an_earlier_success_receipt() {
    std::thread::spawn(|| {
        let _apartment = Apartment::new().unwrap();
        let root = tempfile::tempdir().unwrap();
        let files = files(root.path());
        let buffer =
            crate::platform::windows_shell_delete::source_buffer(&[files[0].0.clone()]).unwrap();
        let item: IShellItem =
            unsafe { SHCreateItemFromParsingName(PCWSTR(buffer.as_ptr()), None).unwrap() };
        let receipt = Arc::new(Mutex::new(Receipt::default()));
        let control = control();
        let sink: IFileOperationProgressSink = DeleteSink {
            root: item.clone(),
            path: files[0].0.clone(),
            proof: Some(files[0].2),
            receipt: receipt.clone(),
            control: Some(control.clone()),
        }
        .into();
        unsafe {
            sink.PreDeleteItem(0, &item).unwrap();
            fs::rename(&files[0].0, root.path().join("saved")).unwrap();
            sink.PostDeleteItem(0, &item, S_OK, None).unwrap();
            control.cancel.store(true, Ordering::Relaxed);
            assert!(sink.UpdateProgress(3, 1).is_err());
        }
        let outcome = receipt
            .lock()
            .unwrap()
            .outcome(Err(Error::from_hresult(E_ABORT)), true);
        assert!(outcome.completed);
        assert!(outcome.error.is_some());
    })
    .join()
    .unwrap();
}
