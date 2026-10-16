#![cfg(target_os = "windows")]
use oxyde_lib::{
    models::get_file_entry_from_path,
    platform::worker_test_support::archive_listing,
    systems::file_ops::{FileOpType, FileOperation, FileOperationManager},
    utils::archive_io_protocol::with_worker_executable_for_tests,
};
use std::{
    fs,
    io::Write,
    path::Path,
    sync::{Arc, Mutex},
    time::Duration,
};

fn native(run: impl FnOnce()) {
    with_worker_executable_for_tests(
        Path::new(env!("CARGO_BIN_EXE_oxyde-archive-io-worker")),
        run,
    );
}
fn operation(kind: FileOpType) -> Arc<Mutex<FileOperation>> {
    Arc::new(Mutex::new(FileOperation::new(kind, Vec::new(), None, None)))
}
fn zip(path: &Path, files: &[&str]) {
    let mut writer = zip::ZipWriter::new(fs::File::create(path).unwrap());
    for file in files {
        writer
            .start_file(*file, zip::write::SimpleFileOptions::default())
            .unwrap();
        writer.write_all(file.as_bytes()).unwrap();
    }
    writer.finish().unwrap();
}

#[test]
fn production_compression_and_extraction_work_under_real_archive_named_directories() {
    native(|| {
        for suffix in ["zip", "7z", "tar.gz"] {
            let root = tempfile::tempdir().unwrap();
            let folder = root.path().join(format!("ordinary.{suffix}"));
            fs::create_dir(&folder).unwrap();
            let source = folder.join("plain.txt");
            fs::write(&source, b"real filesystem payload").unwrap();
            let archive = folder.join("packed.zip");
            FileOperationManager::perform_compress(
                &operation(FileOpType::Compress),
                vec![source.clone()],
                Some(archive.clone()),
            )
            .unwrap();
            let output = folder.join("unpacked");
            FileOperationManager::perform_extract(
                &operation(FileOpType::Extract),
                vec![archive],
                Some(output.clone()),
            )
            .unwrap();
            assert_eq!(
                fs::read(output.join("plain.txt")).unwrap(),
                b"real filesystem payload"
            );
            assert_eq!(fs::read(source).unwrap(), b"real filesystem payload");
        }
    });
}

#[test]
fn production_virtual_transfers_and_deletion_select_the_actual_inner_archive() {
    native(|| {
        for suffix in ["zip", "7z", "tar.gz"] {
            let root = tempfile::tempdir().unwrap();
            let folder = root.path().join(format!("ordinary.{suffix}"));
            fs::create_dir(&folder).unwrap();
            fs::write(folder.join("keep.txt"), b"ordinary directory").unwrap();
            let archive = folder.join("inside.zip");
            zip(&archive, &["copy.txt", "move.txt", "delete.txt"]);
            let before = fs::read(&archive).unwrap();
            let output = folder.join("outputs");
            fs::create_dir(&output).unwrap();
            FileOperationManager::perform_copy(
                &operation(FileOpType::Copy),
                vec![archive.join("copy.txt")],
                Some(output.clone()),
                false,
                None,
            )
            .unwrap();
            assert_eq!(fs::read(&archive).unwrap(), before);
            FileOperationManager::perform_copy(
                &operation(FileOpType::Move),
                vec![archive.join("move.txt")],
                Some(output.clone()),
                true,
                None,
            )
            .unwrap();
            FileOperationManager::perform_delete(
                &operation(FileOpType::Delete),
                vec![archive.join("delete.txt")],
            )
            .unwrap();
            let mut result = zip::ZipArchive::new(fs::File::open(&archive).unwrap()).unwrap();
            assert!(result.by_name("move.txt").is_err());
            assert!(result.by_name("delete.txt").is_err());
            assert_eq!(result.by_name("copy.txt").unwrap().size(), 8);
            assert_eq!(fs::read(output.join("copy.txt")).unwrap(), b"copy.txt");
            assert_eq!(fs::read(output.join("move.txt")).unwrap(), b"move.txt");
            assert_eq!(
                fs::read(folder.join("keep.txt")).unwrap(),
                b"ordinary directory"
            );
        }
    });
}

#[test]
fn production_listing_supplies_origin_for_ui_actions_without_nested_archive_navigation() {
    let root = tempfile::tempdir().unwrap();
    for suffix in ["zip", "7z", "tar.gz"] {
        let folder = root.path().join(format!("ordinary.{suffix}"));
        fs::create_dir(&folder).unwrap();
        let archive = folder.join("inside.zip");
        zip(&archive, &["nested.zip", "disk.vhdx"]);
        let entries = archive_listing::run_in(
            Path::new(env!("CARGO_BIN_EXE_oxyde-archive-list-worker")),
            &archive,
            "",
            Duration::from_secs(15),
        )
        .unwrap();
        assert_eq!(entries.len(), 2);
        assert!(entries
            .iter()
            .all(|entry| entry.archive_path.as_deref() == Some(&archive)));
        assert!(Arc::ptr_eq(
            entries[0].archive_path.as_ref().unwrap(),
            entries[1].archive_path.as_ref().unwrap()
        ));
        assert!(entries.iter().all(|entry| !entry.is_navigable_archive()
            && !entry.is_writable_archive()
            && !entry.is_disk_image()));
        let real = get_file_entry_from_path(&archive).unwrap();
        assert!(
            real.archive_path.is_none()
                && real.is_navigable_archive()
                && real.is_writable_archive()
        );
        let ordinary = get_file_entry_from_path(&folder).unwrap();
        assert!(!ordinary.is_navigable_archive() && !ordinary.is_writable_archive());
    }
}
