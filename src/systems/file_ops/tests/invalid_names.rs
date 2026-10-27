use super::*;
use std::{ffi::OsStr, os::unix::ffi::OsStrExt, path::Path};

fn queued(op_type: FileOpType, source: &Path, destination: Option<&Path>) -> OpStatus {
    let runtime = tokio::runtime::Builder::new_multi_thread().enable_all().build().unwrap();
    runtime.block_on(async {
        let manager = FileOperationManager::new();
        let text = crate::utils::path_text::encode(source);
        let id = manager.queue_operation_on(
            &tokio::runtime::Handle::current(),
            Arc::new(HistoryManager::new()),
            FileOperation::new(op_type, vec![PathBuf::from(text)], destination.map(Path::to_path_buf), None),
        );
        tokio::time::timeout(std::time::Duration::from_secs(20), async {
            loop {
                let status = manager.get_operation(&id).unwrap().status;
                if FileOperationManager::is_final(&status) {
                    return status;
                }
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap()
    })
}

#[::core::prelude::v1::test]
fn operations_on_a_name_that_is_not_utf8_are_refused_before_any_effect() {
    let root = tempfile::tempdir().unwrap();
    let invalid = root.path().join(OsStr::from_bytes(b"a\xffb"));
    let lookalike = root.path().join("a\u{FFFD}b");
    let target = root.path().join("target");
    std::fs::write(&invalid, b"kept").unwrap();
    std::fs::write(&lookalike, b"other").unwrap();
    std::fs::create_dir(&target).unwrap();
    for (op_type, destination) in [
        (FileOpType::Copy, Some(target.as_path())),
        (FileOpType::Move, Some(target.as_path())),
        (FileOpType::Delete, None),
        (FileOpType::Trash, None),
    ] {
        let status = queued(op_type, &invalid, destination);
        let OpStatus::Error(error) = status else { panic!("{status:?}") };
        assert_eq!(
            crate::models::error::UnrepresentablePathFailure::from_message(&error),
            Some(lookalike.to_string_lossy().into_owned())
        );
        assert_eq!(std::fs::read(&invalid).unwrap(), b"kept");
        assert_eq!(std::fs::read(&lookalike).unwrap(), b"other");
        assert_eq!(std::fs::read_dir(&target).unwrap().count(), 0);
    }
}
