#![cfg(target_os = "windows")]
use oxyde_lib::{
    systems::file_ops::{FileOpType, FileOperation, FileOperationManager},
    utils::archive_io_protocol::with_worker_executable_for_tests,
};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    sync::{atomic::Ordering, Arc, Mutex},
    time::{Duration, Instant},
};
#[path = "support/iso.rs"]
mod iso;

fn native<T>(run: impl FnOnce() -> T) -> T {
    with_worker_executable_for_tests(Path::new(env!("CARGO_BIN_EXE_oxyde")), run)
}
fn operation(kind: FileOpType) -> Arc<Mutex<FileOperation>> {
    Arc::new(Mutex::new(FileOperation::new(kind, vec![], None, None)))
}
fn zip(path: &Path, name: &str, bytes: &[u8], unsafe_tail: bool) {
    let mut zip = zip::ZipWriter::new(fs::File::create(path).unwrap());
    zip.start_file(name, zip::write::SimpleFileOptions::default())
        .unwrap();
    zip.write_all(bytes).unwrap();
    if unsafe_tail {
        zip.start_file("../outside", zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(b"evil").unwrap();
    }
    zip.finish().unwrap();
}
fn fixtures(root: &Path) -> Vec<(PathBuf, &'static str, &'static [u8])> {
    let path = root.join("source.zip");
    zip(&path, "docs/école.txt", b"safe", false);
    let mut fixtures = vec![(path, "docs/école.txt", &b"safe"[..])];
    let mut tar = tar::Builder::new(Vec::new());
    let mut header = tar::Header::new_gnu();
    header.set_size(4);
    header.set_mode(0o644);
    header.set_mtime(1234567890);
    header.set_cksum();
    tar.append_data(&mut header, "docs/école.txt", &b"safe"[..])
        .unwrap();
    let tar = tar.into_inner().unwrap();
    let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    gz.write_all(&tar).unwrap();
    let mut xz = xz2::write::XzEncoder::new(Vec::new(), 1);
    xz.write_all(&tar).unwrap();
    let mut bz = bzip2::write::BzEncoder::new(Vec::new(), bzip2::Compression::default());
    bz.write_all(&tar).unwrap();
    let zst = zstd::stream::encode_all(&tar[..], 1).unwrap();
    for (extension, data) in [
        ("tar", tar),
        ("tar.gz", gz.finish().unwrap()),
        ("tar.xz", xz.finish().unwrap()),
        ("tar.bz2", bz.finish().unwrap()),
        ("tar.zst", zst),
    ] {
        let path = root.join(format!("source.{extension}"));
        fs::write(&path, data).unwrap();
        fixtures.push((path, "docs/école.txt", b"safe"));
    }
    let path = root.join("source.7z");
    let mut writer = sevenz_rust2::ArchiveWriter::create(&path).unwrap();
    writer
        .push_archive_entry(
            sevenz_rust2::ArchiveEntry::new_file("docs/école.txt"),
            Some(std::io::Cursor::new(b"safe")),
        )
        .unwrap();
    writer.finish().unwrap();
    fixtures.push((path, "docs/école.txt", b"safe"));
    let path = root.join("source.iso");
    iso::iso_fixture(&path);
    fixtures.push((path, "DOCS/README.TXT", b"ISO content"));
    fixtures
}

#[test]
fn production_full_extraction_uses_the_worker_for_all_eight_families() {
    native(|| {
        let temp = tempfile::tempdir().unwrap();
        for (index, (source, name, expected)) in fixtures(temp.path()).into_iter().enumerate() {
            let before = fs::read(&source).unwrap();
            let provenance = b"[ZoneTransfer]\r\nZoneId=3\r\n";
            oxyde_lib::platform::platform()
                .write_file_provenance(&source, provenance)
                .unwrap();
            let target = temp.path().join(format!("output-{index}"));
            let op = operation(FileOpType::Extract);
            FileOperationManager::perform_extract(&op, vec![source.clone()], Some(target.clone()))
                .unwrap();
            assert_eq!(fs::read(target.join(name)).unwrap(), expected);
            assert_eq!(
                oxyde_lib::platform::platform()
                    .read_file_provenance(&target.join(name))
                    .unwrap()
                    .as_deref(),
                Some(&provenance[..])
            );
            let op = op.lock().unwrap();
            assert_eq!(op.total_files, op.processed_files);
            assert_eq!(op.total_bytes, op.processed_bytes);
            assert!(op.processed_files > 0);
            assert_eq!(fs::read(&source).unwrap(), before);
            assert!(fs::File::options().write(true).open(&source).is_ok());
        }
    });
}

#[test]
fn rejected_index_and_cancelled_admission_remove_only_the_owned_empty_root() {
    native(|| {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("bad.zip");
        zip(&source, "safe.txt", b"safe", true);
        let target = temp.path().join("output");
        assert!(FileOperationManager::perform_extract(
            &operation(FileOpType::Extract),
            vec![source.clone()],
            Some(target.clone())
        )
        .is_err());
        assert!(!target.exists());
        assert!(!temp.path().join("outside").exists());
        zip(&source, "safe.txt", b"safe", false);
        let op = operation(FileOpType::Extract);
        op.lock()
            .unwrap()
            .cancel_flag
            .store(true, Ordering::Relaxed);
        assert!(
            FileOperationManager::perform_extract(&op, vec![source], Some(target.clone())).is_err()
        );
        assert!(!target.exists());
    });
}

#[test]
fn production_pause_cancel_preserves_foreign_children_and_source_snapshot() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("large.zip");
    zip(&source, "docs/owned", &vec![42; 16 * 1024 * 1024], false);
    let before = fs::read(&source).unwrap();
    let target = temp.path().join("output");
    let op = operation(FileOpType::Extract);
    let (operation, archive, destination) = (op.clone(), source.clone(), target.clone());
    let worker = std::thread::spawn(move || {
        native(|| {
            FileOperationManager::perform_extract(&operation, vec![archive], Some(destination))
        })
    });
    let started = Instant::now();
    loop {
        if op.lock().unwrap().processed_bytes >= 65536 {
            break;
        }
        assert!(started.elapsed() < Duration::from_secs(15));
        std::thread::sleep(Duration::from_millis(2));
    }
    op.lock().unwrap().pause_flag.store(true, Ordering::Relaxed);
    std::thread::sleep(Duration::from_millis(200));
    let paused_bytes = op.lock().unwrap().processed_bytes;
    std::thread::sleep(Duration::from_millis(150));
    assert_eq!(op.lock().unwrap().processed_bytes, paused_bytes);
    assert!(fs::File::options().write(true).open(&source).is_err());
    assert!(fs::rename(&source, temp.path().join("renamed.zip")).is_err());
    fs::write(target.join("docs/foreign"), b"keep").unwrap();
    op.lock()
        .unwrap()
        .cancel_flag
        .store(true, Ordering::Relaxed);
    assert!(worker.join().unwrap().is_err());
    assert_eq!(fs::read(target.join("docs/foreign")).unwrap(), b"keep");
    assert!(!target.join("docs/owned").exists());
    assert_eq!(fs::read(source).unwrap(), before);
}

#[test]
fn workspace_decode_uses_native_worker_before_parent_owned_reconstruction() {
    native(|| {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("source.zip");
        zip(&path, "docs/original", b"safe", false);
        oxyde_lib::commands::archive::create_archive_directory_with_quality(
            &path.join("docs/new"),
            "fast",
        )
        .unwrap();
        let mut archive = zip::ZipArchive::new(fs::File::open(path).unwrap()).unwrap();
        assert_eq!(archive.by_name("docs/original").unwrap().size(), 4);
        assert!(archive.by_name("docs/new/").unwrap().is_dir());
    });
}
