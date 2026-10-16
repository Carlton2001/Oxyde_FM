//! Exercise production planning, including the native archive worker path.
#![cfg(target_os = "windows")]
use oxyde_lib::{
    models::{error::TransferDestinationCollision, ConflictAction},
    systems::file_ops::{FileOpType, FileOperation, FileOperationManager},
    utils::archive_io_protocol::with_worker_executable_for_tests,
};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

fn zip(path: &Path, entries: &[(&str, &[u8])]) {
    let mut writer = zip::ZipWriter::new(fs::File::create(path).unwrap());
    for (name, bytes) in entries {
        writer
            .start_file(*name, zip::write::SimpleFileOptions::default())
            .unwrap();
        writer.write_all(bytes).unwrap();
    }
    writer.finish().unwrap();
}

fn reject(sources: Vec<PathBuf>, target: PathBuf, is_move: bool, prepared_entries: usize) {
    let resolutions = sources
        .iter()
        .map(|path| (path.to_string_lossy().into_owned(), ConflictAction::Replace))
        .collect();
    let op = Arc::new(Mutex::new(FileOperation::new(
        if is_move {
            FileOpType::Move
        } else {
            FileOpType::Copy
        },
        sources.clone(),
        Some(target.clone()),
        None,
    )));
    let error =
        FileOperationManager::perform_copy(&op, sources, Some(target), is_move, Some(resolutions))
            .unwrap_err();
    assert!(
        TransferDestinationCollision::from_message(&error).is_some(),
        "{error}"
    );
    let op = op.lock().unwrap();
    assert!(op.created_files.is_empty() && op.created_dirs.is_empty());
    assert!(op.moved_files.is_empty() && op.copied_files.is_empty());
    assert!(op.overwritten_files.is_empty());
    // Native archive preparation counts extracted workspace entries before
    // planning the transfer. None may be added for the rejected transfer.
    assert_eq!(op.processed_files, prepared_entries);
}

#[test]
fn production_transfer_rejects_before_replacing_or_removing_any_source() {
    for is_move in [false, true] {
        let root = tempfile::tempdir().unwrap();
        for name in ["a", "b", "target"] {
            fs::create_dir(root.path().join(name)).unwrap();
        }
        let a = root.path().join("a/report.txt");
        let b = root.path().join("b/report.txt");
        let target = root.path().join("target");
        fs::write(&a, b"source A").unwrap();
        fs::write(&b, b"source B").unwrap();
        fs::write(target.join("report.txt"), b"original").unwrap();
        reject(vec![a.clone(), b.clone()], target.clone(), is_move, 0);
        assert_eq!(fs::read(a).unwrap(), b"source A");
        assert_eq!(fs::read(b).unwrap(), b"source B");
        assert_eq!(fs::read(target.join("report.txt")).unwrap(), b"original");
        assert_eq!(fs::read_dir(target).unwrap().count(), 1);
    }
}

#[test]
fn production_archive_destination_stays_byte_identical_on_collision() {
    with_worker_executable_for_tests(
        Path::new(env!("CARGO_BIN_EXE_oxyde-archive-io-worker")),
        || {
            for is_move in [false, true] {
                let root = tempfile::tempdir().unwrap();
                let archive = root.path().join("target.zip");
                zip(&archive, &[("report.txt", b"original")]);
                let before = fs::read(&archive).unwrap();
                let sources: Vec<_> = ["a", "b"]
                    .into_iter()
                    .map(|parent| {
                        let dir = root.path().join(parent);
                        fs::create_dir(&dir).unwrap();
                        let source = dir.join("report.txt");
                        fs::write(&source, parent.as_bytes()).unwrap();
                        source
                    })
                    .collect();
                reject(sources.clone(), archive.clone(), is_move, 1);
                assert_eq!(fs::read(archive).unwrap(), before);
                for (source, expected) in sources.iter().zip([b"a", b"b"]) {
                    assert_eq!(fs::read(source).unwrap(), expected);
                }
            }
        },
    );
}

#[test]
fn production_archive_sources_and_real_destination_stay_unchanged() {
    with_worker_executable_for_tests(
        Path::new(env!("CARGO_BIN_EXE_oxyde-archive-io-worker")),
        || {
            for is_move in [false, true] {
                let root = tempfile::tempdir().unwrap();
                let archive = root.path().join("sources.zip");
                zip(
                    &archive,
                    &[("a/report.txt", b"first"), ("b/report.txt", b"second")],
                );
                let before = fs::read(&archive).unwrap();
                let target = root.path().join("target");
                fs::create_dir(&target).unwrap();
                fs::write(target.join("report.txt"), b"original").unwrap();
                reject(
                    vec![archive.join("a/report.txt"), archive.join("b/report.txt")],
                    target.clone(),
                    is_move,
                    2,
                );
                assert_eq!(fs::read(archive).unwrap(), before);
                assert_eq!(fs::read(target.join("report.txt")).unwrap(), b"original");
                assert_eq!(fs::read_dir(target).unwrap().count(), 1);
            }
        },
    );
}
