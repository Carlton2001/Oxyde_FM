use super::*;
use std::{fs, os::windows::ffi::OsStrExt};
use windows::Win32::Foundation::E_FAIL;

fn item(path: &Path) -> IShellItem {
    let wide: Vec<_> = path.as_os_str().encode_wide().chain([0]).collect();
    unsafe { SHCreateItemFromParsingName(PCWSTR(wide.as_ptr()), None).unwrap() }
}
#[test]
fn failed_root_keeps_confirmed_descendant_and_excludes_skipped_and_foreign_items() {
    std::thread::spawn(|| {
        let _apartment = Apartment::new().unwrap();
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("selected");
        fs::create_dir(&path).unwrap();
        let child = path.join("école.txt");
        let skipped = path.join("skipped.txt");
        let other = temp.path().join("foreign.txt");
        for file in [&child, &skipped, &other] {
            fs::write(file, b"witness").unwrap();
        }
        let root = item(&path);
        let child_item = item(&child);
        let skip_item = item(&skipped);
        let foreign = item(&other);
        let receipt = Arc::new(Mutex::new(Receipt::default()));
        let sink: IFileOperationProgressSink = DeleteSink {
            root: root.clone(),
            path: path.clone(),
            proof: None,
            receipt: receipt.clone(),
        }
        .into();
        unsafe {
            sink.PreDeleteItem(0, &root).unwrap();
            sink.PreDeleteItem(0, &child_item).unwrap();
            let saved = temp.path().join("saved");
            fs::rename(&child, &saved).unwrap();
            sink.PostDeleteItem(0, &child_item, S_OK, None).unwrap();
            sink.PreDeleteItem(0, &skip_item).unwrap();
            sink.PostDeleteItem(0, &skip_item, COPYENGINE_S_USER_IGNORED, None)
                .unwrap();
            sink.PreDeleteItem(0, &foreign).unwrap();
            sink.PostDeleteItem(0, &foreign, S_OK, None).unwrap();
            sink.PostDeleteItem(0, &root, E_FAIL, None).unwrap();
            sink.FinishOperations(E_FAIL).unwrap();
            let result = receipt
                .lock()
                .unwrap()
                .outcome(Err(Error::from_hresult(E_FAIL)), false);
            assert!(!result.completed && result.error.is_some());
            assert_eq!(result.effects.len(), 1);
            assert_eq!(result.effects[0].path, child.to_string_lossy());
            assert!(result.effects[0]
                .identity
                .unwrap()
                .matches(&fs::File::open(saved).unwrap())
                .unwrap());
        }
        assert_eq!(fs::read(other).unwrap(), b"witness");
        assert_eq!(fs::read(skipped).unwrap(), b"witness");
    })
    .join()
    .unwrap();
}

#[test]
fn native_partial_permanent_delete_reports_every_observed_missing_child_on_private_fixture() {
    use std::os::windows::fs::OpenOptionsExt;
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("selected");
    fs::create_dir(&root).unwrap();
    let a = root.join("a-first.txt");
    let z = root.join("z-locked.txt");
    fs::write(&a, b"first").unwrap();
    fs::write(&z, b"locked").unwrap();
    let _held = fs::OpenOptions::new()
        .read(true)
        .share_mode(1)
        .open(&z)
        .unwrap();
    let result = run(&root, false, None);
    assert!(!result.completed && result.error.is_some());
    assert_eq!(fs::read(&z).unwrap(), b"locked");
    if !a.exists() {
        assert!(
            result
                .effects
                .iter()
                .any(|effect| effect.path == a.to_string_lossy()),
            "Native Shell deleted a child without preserving its completion receipt: {result:?}"
        );
    }
}

#[test]
fn root_success_preserves_a_child_recycled_into_a_separate_payload() {
    std::thread::spawn(|| {
        let _apartment = Apartment::new().unwrap();
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("selected");
        fs::create_dir(&path).unwrap();
        let child = path.join("child");
        fs::write(&child, b"child").unwrap();
        let root = item(&path);
        let child_item = item(&child);
        let receipt = Arc::new(Mutex::new(Receipt::default()));
        let sink: IFileOperationProgressSink = DeleteSink {
            root: root.clone(),
            path: path.clone(),
            proof: None,
            receipt: receipt.clone(),
        }
        .into();
        unsafe {
            sink.PreDeleteItem(0, &root).unwrap();
            sink.PreDeleteItem(0, &child_item).unwrap();
            let child_payload = temp.path().join("separate-child-payload");
            fs::rename(&child, &child_payload).unwrap();
            sink.PostDeleteItem(0, &child_item, S_OK, &item(&child_payload))
                .unwrap();
            let root_payload = temp.path().join("root-payload");
            fs::rename(&path, &root_payload).unwrap();
            sink.PostDeleteItem(0, &root, S_OK, &item(&root_payload))
                .unwrap();
        }
        let result = receipt.lock().unwrap().outcome(Ok(()), false);
        assert!(result.completed && result.error.is_none());
        assert_eq!(result.effects.len(), 1);
        assert_eq!(result.effects[0].path, child.to_string_lossy());
    })
    .join()
    .unwrap();
}
