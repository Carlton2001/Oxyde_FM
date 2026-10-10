use super::*;
use std::io::Write;

#[test]
fn copy_from_iso_preserves_image_and_extracts_selected_entry() {
    let temp = tempfile::tempdir().unwrap();
    let image = temp.path().join("disc.iso");
    crate::commands::archive::test_iso_image(&image);
    let original = std::fs::read(&image).unwrap();
    let target = temp.path().join("output");
    std::fs::create_dir(&target).unwrap();
    let source = image.join("DOCS/README.TXT");
    let operation = Arc::new(Mutex::new(FileOperation::new(
        FileOpType::Copy,
        vec![source.clone()],
        Some(target.clone()),
        None,
    )));
    FileOperationManager::perform_copy(
        &operation,
        vec![source],
        Some(target.clone()),
        false,
        None,
    )
    .unwrap();
    assert_eq!(
        std::fs::read(target.join("README.TXT")).unwrap(),
        b"ISO content"
    );
    assert_eq!(std::fs::read(image).unwrap(), original);
}

#[test]
fn archive_roundtrip_tracks_progress_and_preserves_existing_destinations() {
    for format in ["zip", "7z", "tar", "zst"] {
        for quality in ["fast", "normal", "best"] {
            let temp = tempfile::tempdir().unwrap();
            let source = temp.path().join("folder");
            std::fs::create_dir(&source).unwrap();
            std::fs::create_dir(source.join("empty")).unwrap();
            std::fs::write(source.join("hello.txt"), b"archive roundtrip").unwrap();
            let archive_path = temp.path().join(format!("folder.{format}"));
            std::fs::write(&archive_path, b"existing archive").unwrap();
            let compress = Arc::new(Mutex::new(FileOperation::new(
                FileOpType::Compress,
                vec![source.clone()],
                Some(archive_path.clone()),
                None,
            )));
            compress.lock().unwrap().compression_quality = quality.into();
            FileOperationManager::perform_compress(
                &compress,
                vec![source],
                Some(archive_path.clone()),
            )
            .unwrap();
            let compressed = compress.lock().unwrap().snapshot();
            let produced = compressed.destination.unwrap();
            assert_ne!(produced, archive_path);
            assert_eq!(std::fs::read(&archive_path).unwrap(), b"existing archive");
            assert_eq!(compressed.processed_files, compressed.total_files);
            assert_eq!(compressed.processed_bytes, compressed.total_bytes);

            let extraction = Arc::new(Mutex::new(FileOperation::new(
                FileOpType::Extract,
                vec![produced.clone()],
                Some(temp.path().join("folder")),
                None,
            )));
            FileOperationManager::perform_extract(
                &extraction,
                vec![produced],
                Some(temp.path().join("folder")),
            )
            .unwrap();
            let extracted = extraction.lock().unwrap().snapshot();
            assert_eq!(
                std::fs::read(
                    extracted
                        .destination
                        .as_ref()
                        .unwrap()
                        .join("folder/hello.txt")
                )
                .unwrap(),
                b"archive roundtrip"
            );
            assert!(extracted
                .destination
                .as_ref()
                .unwrap()
                .join("folder/empty")
                .is_dir());
            assert_eq!(extracted.processed_bytes, extracted.total_bytes);
        }
    }
}

#[test]
fn removing_archive_items_keeps_other_entries_and_cleans_temporary_files() {
    use crate::systems::archive_workspace::{mutation_operation, ArchiveWorkspace};

    for extension in ["zip", "7z", "tar", "tar.zst"] {
        let directory = tempfile::tempdir().unwrap();
        let sources = tempfile::tempdir().unwrap();
        std::fs::write(sources.path().join("keep.txt"), b"keep").unwrap();
        std::fs::write(sources.path().join("remove.txt"), b"remove").unwrap();
        std::fs::create_dir_all(sources.path().join("remove/nested")).unwrap();
        std::fs::write(sources.path().join("remove/nested/file.txt"), b"remove").unwrap();
        std::fs::create_dir(sources.path().join("remove-neighbor")).unwrap();
        std::fs::write(sources.path().join("remove-neighbor/file.txt"), b"neighbor").unwrap();
        let archive = directory.path().join(format!("sample.{extension}"));
        FileOperationManager::perform_compress(
            &mutation_operation("normal"),
            vec![
                sources.path().join("keep.txt"),
                sources.path().join("remove.txt"),
                sources.path().join("remove"),
                sources.path().join("remove-neighbor"),
            ],
            Some(archive.clone()),
        )
        .unwrap();
        let operation = Arc::new(Mutex::new(FileOperation::new(
            FileOpType::Delete,
            vec![],
            None,
            None,
        )));
        FileOperationManager::perform_delete(
            &operation,
            vec![archive.join("remove.txt"), archive.join("remove")],
        )
        .unwrap();

        let workspace = ArchiveWorkspace::open(archive).unwrap();
        assert_eq!(
            std::fs::read(workspace.root().join("keep.txt")).unwrap(),
            b"keep"
        );
        assert_eq!(
            std::fs::read(workspace.root().join("remove-neighbor/file.txt")).unwrap(),
            b"neighbor"
        );
        assert!(!workspace.root().join("remove.txt").exists());
        assert!(!workspace.root().join("remove").exists());
        assert_eq!(std::fs::read_dir(workspace.root()).unwrap().count(), 2);
        drop(workspace);
        let remaining = std::fs::read_dir(directory.path())
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect::<Vec<_>>();
        assert_eq!(remaining.len(), 1, "{extension}: residual files {remaining:?}");
    }
}

#[test]
fn compression_does_not_overwrite_an_existing_archive() {
    use crate::systems::archive_workspace::{mutation_operation, ArchiveWorkspace};

    for extension in ["zip", "7z", "tar", "tar.zst"] {
        let directory = tempfile::tempdir().unwrap();
        let sources = tempfile::tempdir().unwrap();
        let source = sources.path().join("source.txt");
        std::fs::write(&source, b"new payload").unwrap();
        let requested = directory.path().join(format!("existing.{extension}"));
        std::fs::write(&requested, b"original").unwrap();
        let operation = mutation_operation("fast");
        FileOperationManager::perform_compress(
            &operation,
            vec![source],
            Some(requested.clone()),
        )
        .unwrap();

        assert_eq!(std::fs::read(&requested).unwrap(), b"original");
        let published = operation.lock().unwrap().destination.clone().unwrap();
        assert_eq!(
            published,
            directory.path().join(format!("existing (1).{extension}"))
        );
        let workspace = ArchiveWorkspace::open(published).unwrap();
        assert_eq!(
            std::fs::read(workspace.root().join("source.txt")).unwrap(),
            b"new payload"
        );
        drop(workspace);
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 2);
    }
}

#[test]
fn archive_file_operations_work_inside_and_between_all_created_formats() {
    for extension in ["zip", "7z", "tar", "tar.zst", "7Z"] {
        let directory = tempfile::tempdir().unwrap();
        let file = directory.path().join("original.txt");
        std::fs::write(&file, b"original").unwrap();
        let archive = directory.path().join(format!("editable.{extension}"));
        let make_op = || {
            Arc::new(Mutex::new(FileOperation::new(
                FileOpType::Copy,
                vec![],
                None,
                None,
            )))
        };
        FileOperationManager::perform_compress(
            &make_op(),
            vec![file.clone()],
            Some(archive.clone()),
        )
        .unwrap();
        let folder = archive.join("nested");
        crate::commands::archive::create_archive_directory_with_quality(&folder, "normal")
            .unwrap();
        let added = directory.path().join("added.txt");
        std::fs::write(&added, b"added").unwrap();
        FileOperationManager::perform_copy(
            &make_op(),
            vec![added.clone()],
            Some(folder.clone()),
            false,
            None,
        )
        .unwrap();
        assert!(added.exists());
        let output = directory.path().join("output");
        std::fs::create_dir(&output).unwrap();
        FileOperationManager::perform_copy(
            &make_op(),
            vec![folder.join("added.txt")],
            Some(output.clone()),
            false,
            None,
        )
        .unwrap();
        assert_eq!(std::fs::read(output.join("added.txt")).unwrap(), b"added");
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime
            .block_on(crate::commands::io::rename_item_controlled(
                folder.join("added.txt").to_string_lossy().into_owned(),
                folder.join("renamed.txt").to_string_lossy().into_owned(),
                "normal".into(),
                Arc::new(AtomicBool::new(false)),
            ))
            .unwrap();
        FileOperationManager::perform_copy(
            &make_op(),
            vec![folder.join("renamed.txt")],
            Some(archive.clone()),
            true,
            None,
        )
        .unwrap();
        let nested = crate::commands::archive::list_archive_contents(
            archive.to_string_lossy().into_owned(),
            "nested".into(),
        )
        .unwrap();
        assert!(nested.is_empty());
        FileOperationManager::perform_copy(
            &make_op(),
            vec![archive.join("renamed.txt")],
            Some(output.clone()),
            true,
            None,
        )
        .unwrap();
        assert_eq!(std::fs::read(output.join("renamed.txt")).unwrap(), b"added");
        let second_archive = directory.path().join(format!("second.{extension}"));
        FileOperationManager::perform_compress(
            &make_op(),
            vec![],
            Some(second_archive.clone()),
        )
        .unwrap();
        FileOperationManager::perform_copy(
            &make_op(),
            vec![archive.join("original.txt")],
            Some(second_archive.clone()),
            true,
            None,
        )
        .unwrap();
        assert!(crate::commands::archive::list_archive_contents(
            second_archive.to_string_lossy().into_owned(),
            "".into()
        )
        .unwrap()
        .iter()
        .any(|entry| entry.name == "original.txt"));
        FileOperationManager::perform_delete(&make_op(), vec![folder]).unwrap();
        assert!(crate::commands::archive::list_archive_contents(
            archive.to_string_lossy().into_owned(),
            "".into()
        )
        .unwrap()
        .is_empty());
    }
}

#[test]
fn archive_move_preserves_skipped_files_and_reports_conflicts() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("source");
    std::fs::create_dir(&source).unwrap();
    std::fs::write(source.join("keep.txt"), b"keep").unwrap();
    std::fs::write(source.join("move.txt"), b"move").unwrap();
    let archive = directory.path().join("test.7z");
    let op = Arc::new(Mutex::new(FileOperation::new(
        FileOpType::Copy,
        vec![],
        None,
        None,
    )));
    FileOperationManager::perform_compress(&op, vec![source.clone()], Some(archive.clone()))
        .unwrap();
    let target = directory.path().join("target");
    std::fs::create_dir_all(target.join("source")).unwrap();
    std::fs::write(target.join("source/keep.txt"), b"existing").unwrap();
    let virtual_folder = archive.join("source");
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let conflicts = runtime
        .block_on(crate::commands::ops::check_conflicts(
            vec![virtual_folder.to_string_lossy().into_owned()],
            target.to_string_lossy().into_owned(),
        ))
        .unwrap();
    assert!(conflicts
        .iter()
        .any(|conflict| conflict.source.path
            == virtual_folder.join("keep.txt").to_string_lossy()));
    let resolutions = HashMap::from([(
        virtual_folder
            .join("keep.txt")
            .to_string_lossy()
            .into_owned(),
        ConflictAction::Skip,
    )]);
    let op = Arc::new(Mutex::new(FileOperation::new(
        FileOpType::Move,
        vec![],
        None,
        None,
    )));
    FileOperationManager::perform_copy(
        &op,
        vec![virtual_folder],
        Some(target.clone()),
        true,
        Some(resolutions),
    )
    .unwrap();
    let remaining = crate::commands::archive::list_archive_contents(
        archive.to_string_lossy().into_owned(),
        "source".into(),
    )
    .unwrap();
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].name, "keep.txt");
    assert_eq!(
        std::fs::read(target.join("source/keep.txt")).unwrap(),
        b"existing"
    );
    assert_eq!(
        std::fs::read(target.join("source/move.txt")).unwrap(),
        b"move"
    );
    let resolutions = HashMap::from([(
        target
            .join("source/keep.txt")
            .to_string_lossy()
            .into_owned(),
        ConflictAction::Replace,
    )]);
    FileOperationManager::perform_copy(
        &Arc::new(Mutex::new(FileOperation::new(
            FileOpType::Copy,
            vec![],
            None,
            None,
        ))),
        vec![target.join("source")],
        Some(archive.clone()),
        false,
        Some(resolutions),
    )
    .unwrap();
    let files = crate::commands::archive::list_archive_contents(
        archive.to_string_lossy().into_owned(),
        "source".into(),
    )
    .unwrap();
    assert_eq!(files.len(), 2);
}

#[test]
fn unsafe_zip_entry_is_rejected_and_partial_output_removed() {
    let temp = tempfile::tempdir().unwrap();
    let archive_path = temp.path().join("unsafe.zip");
    let mut writer = zip::ZipWriter::new(std::fs::File::create(&archive_path).unwrap());
    writer
        .start_file("../escape.txt", zip::write::SimpleFileOptions::default())
        .unwrap();
    writer.write_all(b"escape").unwrap();
    writer.finish().unwrap();
    let output = temp.path().join("unpacked");
    let extraction = Arc::new(Mutex::new(FileOperation::new(
        FileOpType::Extract,
        vec![archive_path.clone()],
        Some(output.clone()),
        None,
    )));
    assert!(FileOperationManager::perform_extract(
        &extraction,
        vec![archive_path],
        Some(output.clone())
    )
    .is_err());
    assert!(!output.exists());
    assert!(!temp.path().join("escape.txt").exists());
}
