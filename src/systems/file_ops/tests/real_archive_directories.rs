use super::*;
use std::io::Write;

#[test]
fn real_archive_named_directories_support_copy_move_delete_listing_and_creation() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    for name in ["ordinary.zip", "ordinary.7z", "ordinary.tar.gz"] {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join(name);
        let copied = root.path().join("copied");
        let moved = root.path().join("moved");
        for folder in [&source, &copied, &moved] {
            std::fs::create_dir(folder).unwrap();
        }
        std::fs::write(source.join("plain.txt"), b"preserve real bytes").unwrap();
        crate::commands::io::create_local_folder(&source.join("child")).unwrap();
        let entries = runtime
            .block_on(crate::commands::io::list_dir(
                source.to_string_lossy().into_owned(),
                None,
                None,
            ))
            .unwrap();
        assert_eq!(entries.len(), 2);
        assert!(entries.iter().all(|entry| entry.archive_path.is_none()));
        FileOperationManager::perform_copy(
            &operation(),
            vec![source.clone()],
            Some(copied.clone()),
            false,
            None,
        )
        .unwrap();
        assert_eq!(
            std::fs::read(copied.join(name).join("plain.txt")).unwrap(),
            b"preserve real bytes"
        );
        FileOperationManager::perform_copy(
            &operation(),
            vec![source.clone()],
            Some(moved.clone()),
            true,
            None,
        )
        .unwrap();
        assert!(!source.exists());
        let file = moved.join(name).join("plain.txt");
        let renamed = moved.join(name).join("renamed.txt");
        runtime
            .block_on(crate::commands::io::rename_item_controlled(
                file.to_string_lossy().into_owned(),
                renamed.to_string_lossy().into_owned(),
                "fast".into(),
                Arc::new(AtomicBool::new(false)),
            ))
            .unwrap();
        assert_eq!(std::fs::read(&renamed).unwrap(), b"preserve real bytes");
        FileOperationManager::perform_delete(&operation(), vec![renamed.clone()]).unwrap();
        assert!(!renamed.exists());
        assert!(moved.join(name).join("child").is_dir());
        assert_eq!(
            std::fs::read(copied.join(name).join("plain.txt")).unwrap(),
            b"preserve real bytes"
        );
    }
}

#[cfg(target_os = "windows")]
#[test]
fn trash_below_archive_named_directories_keeps_a_working_undo_receipt() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    for name in ["ordinary.zip", "ordinary.7z", "ordinary.tar.gz"] {
        let root = tempfile::tempdir().unwrap();
        let folder = root.path().join(name);
        std::fs::create_dir(&folder).unwrap();
        let source = folder.join("private-trash-fixture.txt");
        std::fs::write(&source, b"undo must restore this fixture").unwrap();
        let history = Arc::new(HistoryManager::new());
        let op = Arc::new(Mutex::new(FileOperation::new(
            FileOpType::Trash,
            vec![source.clone()],
            None,
            None,
        )));
        runtime.block_on(FileOperationManager::execute_operation(
            history.clone(),
            op.clone(),
        ));
        let status = op.lock().unwrap().status.clone();
        let removed = !source.exists();
        // Restore our own fixture before assertions, even after a partial failure.
        let undo = runtime.block_on(crate::commands::ops::undo_last_action(&history));
        assert!(matches!(status, OpStatus::Completed), "{status:?}");
        assert!(removed);
        assert!(undo.unwrap().is_some());
        assert_eq!(
            std::fs::read(&source).unwrap(),
            b"undo must restore this fixture"
        );
        assert!(folder.is_dir());
    }
}

#[test]
fn nested_real_archives_still_list_and_delete_their_virtual_members() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    for name in ["ordinary.zip", "ordinary.7z", "ordinary.tar.gz"] {
        for trash in [false, true] {
            let root = tempfile::tempdir().unwrap();
            let folder = root.path().join(name);
            std::fs::create_dir(&folder).unwrap();
            let archive = folder.join("inside.zip");
            let mut writer = zip::ZipWriter::new(std::fs::File::create(&archive).unwrap());
            for file in ["child/remove.txt", "child/keep.txt", "nested.zip"] {
                writer
                    .start_file(file, zip::write::SimpleFileOptions::default())
                    .unwrap();
                writer.write_all(file.as_bytes()).unwrap();
            }
            writer.finish().unwrap();
            let files = runtime
                .block_on(crate::commands::io::list_dir(
                    archive.join("child").to_string_lossy().into_owned(),
                    None,
                    None,
                ))
                .unwrap();
            assert_eq!(files.len(), 2);
            assert!(files
                .iter()
                .all(|file| file.archive_path.as_deref() == Some(&archive)));
            let sources = vec![archive.join("child/remove.txt")];
            if trash {
                FileOperationManager::perform_trash(&operation(), sources).unwrap();
            } else {
                FileOperationManager::perform_delete(&operation(), sources).unwrap();
            }
            let mut result = zip::ZipArchive::new(std::fs::File::open(&archive).unwrap()).unwrap();
            assert!(result.by_name("child/remove.txt").is_err());
            assert_eq!(result.by_name("child/keep.txt").unwrap().size(), 14);
            assert_eq!(result.by_name("nested.zip").unwrap().size(), 10);
            assert!(folder.is_dir());
        }
    }
}
