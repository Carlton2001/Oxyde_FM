use super::*;
use std::{ffi::OsStr, os::unix::ffi::OsStrExt};

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Runtime::new().unwrap()
}

fn listed(path: &str) -> Vec<FileEntry> {
    runtime().block_on(list_dir(path.to_owned(), Some(true), Some(true))).unwrap()
}

#[test]
fn a_name_that_is_not_utf8_is_listed_apart_from_its_lookalike() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join(OsStr::from_bytes(b"a\xffb")), b"kept").unwrap();
    std::fs::write(root.path().join("a\u{FFFD}b"), b"other").unwrap();
    let entries = listed(&root.path().to_string_lossy());
    assert_eq!(entries.len(), 2);
    let (invalid, valid): (Vec<_>, Vec<_>) = entries.iter().partition(|entry| entry.has_invalid_name());
    assert_eq!((invalid.len(), valid.len()), (1, 1));
    assert_eq!(invalid[0].name, valid[0].name);
    assert_ne!(invalid[0].path, valid[0].path);
    assert_eq!(std::fs::read(crate::utils::path_text::decode(&invalid[0].path)).unwrap(), b"kept");
}

#[test]
fn a_folder_whose_name_is_not_utf8_can_be_opened() {
    let root = tempfile::tempdir().unwrap();
    let folder = root.path().join(OsStr::from_bytes(b"dossier\xe9"));
    std::fs::create_dir(&folder).unwrap();
    std::fs::write(folder.join("inside.txt"), b"inside").unwrap();
    let folder = listed(&root.path().to_string_lossy()).remove(0);
    assert!(folder.is_dir && folder.has_invalid_name());
    let children = listed(&folder.path);
    assert_eq!(children.len(), 1);
    assert_eq!(children[0].name, "inside.txt");
    assert!(!children[0].has_invalid_name());
    assert!(crate::utils::path_text::is_encoded(&children[0].path));
}

#[test]
fn renaming_fixes_a_name_that_is_not_utf8_without_touching_its_lookalike() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join(OsStr::from_bytes(b"a\xffb")), b"kept").unwrap();
    std::fs::write(root.path().join("a\u{FFFD}b"), b"other").unwrap();
    let invalid = listed(&root.path().to_string_lossy()).into_iter().find(FileEntry::has_invalid_name).unwrap();
    let fixed = root.path().join("fixed").to_string_lossy().into_owned();
    let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    runtime().block_on(rename_item_controlled(invalid.path, fixed.clone(), "normal".into(), cancel)).unwrap();
    assert_eq!(std::fs::read(&fixed).unwrap(), b"kept");
    assert_eq!(std::fs::read(root.path().join("a\u{FFFD}b")).unwrap(), b"other");
    assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 2);
}

#[test]
fn preparing_a_transfer_of_a_name_that_is_not_utf8_is_refused() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join(OsStr::from_bytes(b"a\xffb")), b"kept").unwrap();
    let invalid = listed(&root.path().to_string_lossy()).remove(0);
    let error = runtime()
        .block_on(crate::commands::ops::check_conflicts(vec![invalid.path], root.path().to_string_lossy().into_owned()))
        .unwrap_err();
    assert!(crate::models::error::UnrepresentablePathFailure::from_message(&format!("{error:?}")).is_some());
}
