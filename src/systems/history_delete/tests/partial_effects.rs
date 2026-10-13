use super::*;
use std::fs;

#[test]
fn initial_failed_folder_delete_journals_only_confirmed_children() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("selected");
    fs::create_dir(&root).unwrap();
    let child = root.join("child");
    fs::write(&child, b"child").unwrap();
    let proof = HistoryFileIdentity::from_file(&File::open(&child).unwrap()).unwrap();
    let prepared = PreparedDelete::new(&root).unwrap();
    let mut journal = Vec::new();
    let result = prepared.recycle_reporting(
        &mut |_, _| {
            fs::rename(&child, temp.path().join("saved")).unwrap();
            crate::platform::ShellDeleteOutcome {
                completed: false,
                error: Some("root failed".into()),
                effects: vec![HistoryDeletedItem::new(
                    child.to_string_lossy().into_owned(),
                    Some(proof),
                )],
            }
        },
        &mut |entry| journal.push(entry),
    );
    assert!(result.is_err());
    assert_eq!(journal.len(), 1);
    assert_eq!(journal[0].path, child.to_string_lossy());
    assert!(root.exists());
}

#[test]
fn failed_redo_keeps_child_receipts_in_shared_journal_through_root_retry() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("selected");
    fs::create_dir(&root).unwrap();
    let child = root.join("child");
    fs::write(&child, b"child").unwrap();
    let prepared = PreparedDelete::new(&root).unwrap();
    let entry = HistoryDeletedItem::new(root.to_string_lossy().into_owned(), prepared.identity);
    entry.set_restored(true);
    let cloned = entry.clone();
    let proof = HistoryFileIdentity::from_file(&File::open(&child).unwrap()).unwrap();
    assert!(redo_reporting(&[entry.clone()], &mut |_, _| {
        fs::rename(&child, temp.path().join("saved-child")).unwrap();
        crate::platform::ShellDeleteOutcome {
            completed: false,
            error: Some("root failed".into()),
            effects: vec![HistoryDeletedItem::new(
                child.to_string_lossy().into_owned(),
                Some(proof),
            )],
        }
    })
    .is_err());
    assert!(cloned.is_restored());
    assert_eq!(cloned.partial()[0].path, child.to_string_lossy());
    drop(prepared);
    redo_reporting(&[entry], &mut |path, _| {
        fs::rename(path, temp.path().join("saved-root")).unwrap();
        crate::platform::ShellDeleteOutcome::from_result(Ok(()))
    })
    .unwrap();
    assert!(!cloned.is_restored());
    assert_eq!(cloned.partial().len(), 1);
}

#[test]
fn undo_restores_separate_root_and_child_payloads_in_order_and_keeps_retry_state() {
    use std::os::windows::ffi::OsStrExt;
    let temp = tempfile::tempdir().unwrap();
    let bin = temp.path().join("$Recycle.Bin/test-user");
    fs::create_dir_all(&bin).unwrap();
    let original = temp.path().join("restored");
    let payload_root = bin.join("$Rroot");
    let payload_child = bin.join("$Rchild");
    fs::create_dir(&payload_root).unwrap();
    fs::write(&payload_child, b"selected child").unwrap();
    let make = |payload: &Path, target: &Path| {
        let parents = HeldParents::new(payload).unwrap();
        let held = platform()
            .track_source_item(
                parents.directory(),
                &parents.resolved_path(payload).unwrap(),
            )
            .unwrap();
        let entry = HistoryDeletedItem::new(
            target.to_string_lossy().into_owned(),
            Some(HistoryFileIdentity::from_file(&held).unwrap()),
        );
        let units: Vec<_> = target.as_os_str().encode_wide().chain([0]).collect();
        let mut metadata = 2u64.to_le_bytes().to_vec();
        metadata.extend_from_slice(&held.metadata().unwrap().len().to_le_bytes());
        metadata.extend_from_slice(&0u64.to_le_bytes());
        metadata.extend_from_slice(&(units.len() as u32).to_le_bytes());
        for unit in units {
            metadata.extend_from_slice(&unit.to_le_bytes());
        }
        let name = payload.file_name().unwrap().to_string_lossy();
        fs::write(bin.join(name.replacen("$R", "$I", 1)), metadata).unwrap();
        let item = trash::TrashItem {
            id: payload.as_os_str().to_owned(),
            name: target.file_name().unwrap().to_owned(),
            original_parent: target.parent().unwrap().to_path_buf(),
            time_deleted: 0,
        };
        (entry, item)
    };
    let (root, root_item) = make(&payload_root, &original);
    let (child, child_item) = make(&payload_child, &original.join("child"));
    root.remember_partial(vec![child.clone()]);
    let items = vec![root_item, child_item];
    let mut first = true;
    assert!(restore_entries(&[root.clone()], &mut |journal| {
        platform().restore_trash_history(items.clone(), journal)?;
        if first {
            first = false;
            fs::write(original.join("child"), b"foreign conflict").unwrap();
        }
        Ok(())
    })
    .is_err());
    assert!(root.is_restored() && !child.is_restored());
    assert_eq!(fs::read(&payload_child).unwrap(), b"selected child");
    assert_eq!(
        fs::read(original.join("child")).unwrap(),
        b"foreign conflict"
    );
    fs::remove_file(original.join("child")).unwrap();
    restore_entries(&[root], &mut |journal| {
        platform().restore_trash_history(items.clone(), journal)
    })
    .unwrap();
    assert!(child.is_restored());
    assert_eq!(fs::read(original.join("child")).unwrap(), b"selected child");
    assert!(!payload_root.exists() && !payload_child.exists());
}
