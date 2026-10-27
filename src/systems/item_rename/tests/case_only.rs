use super::*;
use std::{
    fs,
    sync::{atomic::AtomicBool, Arc},
};

fn leaf_exists_exactly(path: &Path) -> bool {
    fs::read_dir(path.parent().unwrap())
        .unwrap()
        .any(|entry| entry.unwrap().file_name() == path.file_name().unwrap())
}

#[test]
fn public_case_only_rename_preserves_files_directories_and_unicode() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    for directory in [false, true] {
        let temp = tempfile::tempdir().unwrap();
        let old = temp.path().join("été.txt");
        let new = temp.path().join("ÉTÉ.TXT");
        if directory {
            fs::create_dir(&old).unwrap();
            fs::write(old.join("child"), b"selected").unwrap();
        } else {
            fs::write(&old, b"selected").unwrap();
        }
        let proof = runtime
            .block_on(crate::commands::io::rename_item_controlled_with_identity(
                old.to_string_lossy().into_owned(),
                new.to_string_lossy().into_owned(),
                "normal".into(),
                Arc::new(AtomicBool::new(false)),
            ))
            .unwrap()
            .expect("successful rename must retain its identity proof");
        assert!(leaf_exists_exactly(&new));
        assert!(!leaf_exists_exactly(&old));
        assert_eq!(
            fs::read(if directory {
                new.join("child")
            } else {
                new.clone()
            })
            .unwrap(),
            b"selected"
        );
        rename_from_history(&new, &old, Some(proof)).unwrap();
        assert!(leaf_exists_exactly(&old));
        assert!(!leaf_exists_exactly(&new));
        rename_from_history(&old, &new, Some(proof)).unwrap();
        assert!(leaf_exists_exactly(&new));
    }
}

#[test]
fn identical_name_and_independent_destinations_remain_rejected() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("source.txt");
    let target = temp.path().join("target.txt");
    fs::write(&source, b"selected").unwrap();
    assert!(rename(&source, &source).is_err());
    for hard_link in [false, true] {
        if hard_link {
            fs::hard_link(&source, &target).unwrap();
        } else {
            fs::write(&target, b"foreign").unwrap();
        }
        assert_eq!(
            rename(&source, &target).unwrap_err().kind(),
            io::ErrorKind::AlreadyExists
        );
        assert_eq!(fs::read(&source).unwrap(), b"selected");
        assert_eq!(
            fs::read(&target).unwrap(),
            if hard_link {
                b"selected".as_slice()
            } else {
                b"foreign".as_slice()
            }
        );
        fs::remove_file(&target).unwrap();
    }
    let other_parent = temp.path().join("other");
    fs::create_dir(&other_parent).unwrap();
    let other_link = other_parent.join("Source.txt");
    fs::hard_link(&source, &other_link).unwrap();
    assert_eq!(
        rename(&source, &other_link).unwrap_err().kind(),
        io::ErrorKind::AlreadyExists
    );
    assert!(leaf_exists_exactly(&source) && leaf_exists_exactly(&other_link));
}

#[test]
fn sensitive_directory_distinguishes_independent_case_variants_even_for_hard_links() {
    for hard_link in [false, true] {
        let temp = tempfile::tempdir().unwrap();
        platform().set_fixture_case_sensitive(temp.path()).unwrap();
        let source = temp.path().join("photo.txt");
        let target = temp.path().join("Photo.txt");
        fs::write(&source, b"selected").unwrap();
        if hard_link {
            fs::hard_link(&source, &target).unwrap();
        } else {
            fs::write(&target, b"foreign").unwrap();
        }
        assert_eq!(
            rename(&source, &target).unwrap_err().kind(),
            io::ErrorKind::AlreadyExists
        );
        assert!(leaf_exists_exactly(&source) && leaf_exists_exactly(&target));
        assert_eq!(fs::read(&source).unwrap(), b"selected");
        assert_eq!(
            fs::read(&target).unwrap(),
            if hard_link {
                b"selected".as_slice()
            } else {
                b"foreign".as_slice()
            }
        );
        fs::remove_file(&target).unwrap();
        rename(&source, &target).unwrap();
        assert!(!leaf_exists_exactly(&source) && leaf_exists_exactly(&target));
    }
}

#[test]
fn late_case_variant_in_a_sensitive_directory_is_never_replaced() {
    let temp = tempfile::tempdir().unwrap();
    platform().set_fixture_case_sensitive(temp.path()).unwrap();
    let source = temp.path().join("photo.txt");
    let target = temp.path().join("Photo.txt");
    fs::write(&source, b"selected").unwrap();
    let error = rename_with_checkpoint(&source, &target, &mut || {
        fs::write(&target, b"late foreign")
    })
    .unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
    assert_eq!(fs::read(&source).unwrap(), b"selected");
    assert_eq!(fs::read(&target).unwrap(), b"late foreign");
}

#[test]
fn case_only_junction_rename_and_history_do_not_touch_the_target() {
    let temp = tempfile::tempdir().unwrap();
    let target = temp.path().join("outside");
    fs::create_dir(&target).unwrap();
    fs::write(target.join("witness"), b"untouched").unwrap();
    let source = temp.path().join("link");
    let renamed = temp.path().join("Link");
    crate::platform::windows_reparse_tests::junction(&source, &target);
    let proof = rename_captured_with_checkpoint(&source, &renamed, None, &mut || Ok(())).unwrap();
    assert!(leaf_exists_exactly(&renamed));
    assert!(!leaf_exists_exactly(&source));
    rename_from_history(&renamed, &source, proof).unwrap();
    assert!(leaf_exists_exactly(&source));
    assert_eq!(fs::read(target.join("witness")).unwrap(), b"untouched");
    fs::remove_dir(&source).unwrap();
}

#[test]
fn cancelled_case_only_rename_keeps_the_selected_entry() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("photo.txt");
    let target = temp.path().join("Photo.txt");
    fs::write(&source, b"selected").unwrap();
    assert_eq!(
        rename_with_checkpoint(&source, &target, &mut || Err(
            io::ErrorKind::Interrupted.into()
        ))
        .unwrap_err()
        .kind(),
        io::ErrorKind::Interrupted
    );
    assert!(leaf_exists_exactly(&source));
    assert!(!leaf_exists_exactly(&target));
}

#[test]
fn case_only_rename_uses_the_same_undo_redo_commands_as_ordinary_renames() {
    use crate::models::{HistoryManager, Transaction, TransactionDetails, TransactionType};
    let runtime = tokio::runtime::Runtime::new().unwrap();
    for directory in [false, true] {
        let root = tempfile::tempdir().unwrap();
        let old = root.path().join("photo");
        let new = root.path().join("Photo");
        if directory {
            fs::create_dir(&old).unwrap();
            fs::write(old.join("child"), b"selected").unwrap();
        } else {
            fs::write(&old, b"selected").unwrap();
        }
        let proof = runtime
            .block_on(crate::commands::io::rename_item_controlled_with_identity(
                old.to_string_lossy().into_owned(),
                new.to_string_lossy().into_owned(),
                "normal".into(),
                Arc::new(AtomicBool::new(false)),
            ))
            .unwrap();
        let history = HistoryManager::new();
        history.push(Transaction::new(
            TransactionType::Rename,
            TransactionDetails {
                paths: vec![old.to_string_lossy().into_owned()],
                target_dir: None,
                old_path: Some(old.to_string_lossy().into_owned()),
                new_path: Some(new.to_string_lossy().into_owned()),
                created_links: None,
                moved_links: None,
                directory_attributes: None,
                renamed_dirs: None,
                renamed_item_identity: proof,
                new_folder_identity: None,
                created_files: None,
                created_dirs: None,
                overwritten_files: None,
                moved_files: None,
                moved_file_identities: None,
                moved_directory_identities: None,
                trash_metadata_backups: None,
                moved_dirs: None,
                copied_files: None,
                copied_file_hashes: None,
                copied_file_identities: None,
                copied_directory_identities: None,
                deleted_items: None,
            },
        ));
        assert!(runtime
            .block_on(crate::commands::ops::undo_last_action(&history))
            .unwrap()
            .is_some());
        assert!(leaf_exists_exactly(&old) && !leaf_exists_exactly(&new));
        assert_eq!(history.summary().redo, Some(TransactionType::Rename));
        assert!(runtime
            .block_on(crate::commands::ops::redo_last_action(&history))
            .unwrap()
            .is_some());
        assert!(leaf_exists_exactly(&new) && !leaf_exists_exactly(&old));
        assert_eq!(history.summary().undo, Some(TransactionType::Rename));
        assert_eq!(
            fs::read(if directory { new.join("child") } else { new }).unwrap(),
            b"selected"
        );
    }
}

#[test]
fn case_only_source_stays_pinned_between_validation_and_native_rename() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("photo.txt");
    let target = root.path().join("Photo.txt");
    let displaced = root.path().join("displaced.txt");
    fs::write(&source, b"selected").unwrap();
    rename_with_checkpoint(&source, &target, &mut || {
        assert!(fs::rename(&source, &displaced).is_err());
        assert!(fs::write(&target, b"foreign").is_err());
        Ok(())
    })
    .unwrap();
    assert!(leaf_exists_exactly(&target) && !leaf_exists_exactly(&source));
    assert!(!displaced.exists());
    assert_eq!(fs::read(&target).unwrap(), b"selected");
}
