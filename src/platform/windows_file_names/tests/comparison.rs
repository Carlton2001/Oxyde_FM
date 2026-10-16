use super::*;
use crate::platform::platform;
use std::os::windows::fs::OpenOptionsExt;
use windows::Win32::Storage::FileSystem::{
    SetFileInformationByHandle, FILE_FLAG_BACKUP_SEMANTICS, FILE_READ_ATTRIBUTES,
    FILE_WRITE_ATTRIBUTES,
};

#[test]
fn directory_rules_use_native_unicode_case_without_expanding_names() {
    let root = tempfile::tempdir().unwrap();
    let directory = platform()
        .hold_resolved_operation_directory(root.path())
        .unwrap();
    let rules = platform().file_name_rules(&directory).unwrap();
    assert_eq!(
        rules.key(OsStr::new("École.txt")),
        rules.key(OsStr::new("école.txt"))
    );
    assert_eq!(
        rules.key(OsStr::new("Report.txt")),
        rules.key(OsStr::new("report.txt"))
    );
    assert_ne!(
        rules.key(OsStr::new("straße.txt")),
        rules.key(OsStr::new("strasse.txt"))
    );
}

#[test]
fn case_sensitive_destination_preserves_distinct_incoming_names() {
    use crate::systems::file_ops::{FileOpType, FileOperation, FileOperationManager};
    use std::sync::{Arc, Mutex};
    let root = tempfile::tempdir().unwrap();
    let target = root.path().join("target");
    std::fs::create_dir(&target).unwrap();
    let handle = std::fs::OpenOptions::new()
        .access_mode(FILE_WRITE_ATTRIBUTES.0 | FILE_READ_ATTRIBUTES.0)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS.0)
        .open(&target)
        .unwrap();
    let info = FILE_CASE_SENSITIVE_INFO { Flags: 1 };
    unsafe {
        SetFileInformationByHandle(
            HANDLE(handle.as_raw_handle()),
            FileCaseSensitiveInfo,
            (&info as *const FILE_CASE_SENSITIVE_INFO).cast(),
            std::mem::size_of_val(&info) as u32,
        )
    }
    .expect("Fixture requires NTFS per-directory case sensitivity");
    let rules = platform().file_name_rules(&handle).unwrap();
    assert_ne!(
        rules.key(OsStr::new("Report.txt")),
        rules.key(OsStr::new("report.txt"))
    );
    drop(handle);
    for (name, bytes) in [
        ("Report.txt", b"first".as_slice()),
        ("report.txt", b"second".as_slice()),
    ] {
        // Sources have separate parents, since the fixture root remains insensitive.
        let parent = root
            .path()
            .join(if name.starts_with('R') { "a" } else { "b" });
        std::fs::create_dir(&parent).unwrap();
        let source = parent.join(name);
        std::fs::write(&source, bytes).unwrap();
    }
    let sources = vec![
        root.path().join("a/Report.txt"),
        root.path().join("b/report.txt"),
    ];
    let op = Arc::new(Mutex::new(FileOperation::new(
        FileOpType::Copy,
        sources.clone(),
        Some(target.clone()),
        None,
    )));
    FileOperationManager::perform_copy(&op, sources, Some(target.clone()), false, None).unwrap();
    assert_eq!(std::fs::read(target.join("Report.txt")).unwrap(), b"first");
    assert_eq!(std::fs::read(target.join("report.txt")).unwrap(), b"second");
    assert_eq!(std::fs::read_dir(target).unwrap().count(), 2);
}
