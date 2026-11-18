use super::*;
use std::{fs, os::windows::ffi::OsStrExt};

fn item(path: &Path) -> IShellItem {
    let wide: Vec<_> = path.as_os_str().encode_wide().chain([0]).collect();
    unsafe { SHCreateItemFromParsingName(PCWSTR(wide.as_ptr()), None).unwrap() }
}

/// The selection reached through a junction, as with a SUBST or mapped drive:
/// the Shell may report the root by its other path.
fn fixture(temp: &Path) -> (PathBuf, PathBuf) {
    let target = temp.join("target");
    fs::create_dir(&target).unwrap();
    fs::write(target.join("selected.txt"), b"selected").unwrap();
    let link = temp.join("link");
    let status = std::process::Command::new("cmd")
        .args(["/C", "mklink", "/J"])
        .arg(&link)
        .arg(&target)
        .output()
        .unwrap();
    assert!(status.status.success());
    (link.join("selected.txt"), target.join("selected.txt"))
}

fn sink(path: &Path, proof: HistoryFileIdentity, receipt: &Arc<Mutex<Receipt>>) -> IFileOperationProgressSink {
    DeleteSink {
        root: item(path),
        path: path.to_path_buf(),
        proof: Some(proof),
        receipt: receipt.clone(),
        control: None,
    }
    .into()
}

#[test]
fn a_root_reported_under_another_path_is_still_proven() {
    std::thread::spawn(|| {
        let _apartment = Apartment::new().unwrap();
        let temp = tempfile::tempdir().unwrap();
        let (selected, reported) = fixture(temp.path());
        let proof = HistoryFileIdentity::from_file(&fs::File::open(&selected).unwrap()).unwrap();
        let receipt = Arc::new(Mutex::new(Receipt::default()));
        let sink = sink(&selected, proof, &receipt);
        let callback_root = item(&reported);
        unsafe {
            sink.PreDeleteItem(0, &callback_root).unwrap();
            assert!(receipt.lock().unwrap().preflight);
            // Only a private rename: nothing is deleted.
            fs::rename(&reported, reported.with_extension("saved")).unwrap();
            sink.PostDeleteItem(0, &callback_root, S_OK, None).unwrap();
            sink.FinishOperations(S_OK).unwrap();
        }
        let outcome = receipt.lock().unwrap().outcome(Ok(()), false);
        assert!(outcome.completed);
        assert!(outcome.error.is_none());
    })
    .join()
    .unwrap();
}

#[test]
fn a_replaced_root_reported_under_another_path_is_refused() {
    std::thread::spawn(|| {
        let _apartment = Apartment::new().unwrap();
        let temp = tempfile::tempdir().unwrap();
        let (selected, reported) = fixture(temp.path());
        let proof = HistoryFileIdentity::from_file(&fs::File::open(&selected).unwrap()).unwrap();
        fs::remove_file(&reported).unwrap();
        fs::write(&reported, b"replacement").unwrap();
        let receipt = Arc::new(Mutex::new(Receipt::default()));
        let sink = sink(&selected, proof, &receipt);
        unsafe {
            assert!(sink.PreDeleteItem(0, &item(&reported)).is_err());
        }
        assert!(!receipt.lock().unwrap().preflight);
        assert_eq!(fs::read(&reported).unwrap(), b"replacement");
    })
    .join()
    .unwrap();
}
