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
#[path = "support/seven_crc.rs"]
mod seven_crc;

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

#[test]
fn production_selected_cache_and_permanent_export_cover_all_eight_families() {
    native(|| {
        let temp = tempfile::tempdir().unwrap();
        let base = temp.path().join("cache");
        fs::create_dir(&base).unwrap();
        oxyde_lib::commands::archive::with_archive_cache_base_for_tests(&base, || {
            for (index, (source, name, expected)) in fixtures(temp.path()).into_iter().enumerate() {
                let provenance = b"[ZoneTransfer]\r\nZoneId=3\r\n";
                oxyde_lib::platform::platform()
                    .write_file_provenance(&source, provenance)
                    .unwrap();
                let before = fs::read(&source).unwrap();
                let virtual_file = source.join(name);
                let cached =
                    oxyde_lib::commands::archive::prepare_archive_file_for_open(&virtual_file)
                        .unwrap();
                assert_eq!(fs::read(&cached).unwrap(), expected);
                let payload = cached
                    .ancestors()
                    .find(|path| path.file_name().is_some_and(|name| name == "data"))
                    .unwrap();
                let files: Vec<_> = walkdir::WalkDir::new(payload)
                    .into_iter()
                    .map(Result::unwrap)
                    .filter(|entry| entry.file_type().is_file())
                    .collect();
                assert_eq!(files.len(), 1);
                let folder = temp.path().join(format!("export-{index}"));
                fs::create_dir(&folder).unwrap();
                let first =
                    oxyde_lib::commands::archive::extract_archive_file_to(&virtual_file, &folder)
                        .unwrap();
                let second =
                    oxyde_lib::commands::archive::extract_archive_file_to(&virtual_file, &folder)
                        .unwrap();
                assert_ne!(first, second);
                assert_eq!(fs::read(&first).unwrap(), expected);
                assert_eq!(fs::read(&second).unwrap(), expected);
                assert_eq!(fs::read_dir(&folder).unwrap().count(), 2);
                for path in [&cached, &first, &second] {
                    assert_eq!(
                        oxyde_lib::platform::platform()
                            .read_file_provenance(path)
                            .unwrap()
                            .as_deref(),
                        Some(&provenance[..])
                    );
                }
                assert_eq!(fs::read(source).unwrap(), before);
            }
        });
    });
}

#[test]
fn selected_failure_keeps_cache_and_export_clean_and_refuses_missing_or_directory_members() {
    native(|| {
        let temp = tempfile::tempdir().unwrap();
        let base = temp.path().join("cache");
        fs::create_dir(&base).unwrap();
        oxyde_lib::commands::archive::with_archive_cache_base_for_tests(&base, || {
            let source = temp.path().join("bad.zip");
            zip(&source, "docs/safe", b"safe", true);
            let export = temp.path().join("export");
            fs::create_dir(&export).unwrap();
            for name in ["docs/safe", "missing", "docs"] {
                assert!(oxyde_lib::commands::archive::prepare_archive_file_for_open(
                    &source.join(name)
                )
                .is_err());
                assert!(oxyde_lib::commands::archive::extract_archive_file_to(
                    &source.join(name),
                    &export
                )
                .is_err());
                assert_eq!(fs::read_dir(&export).unwrap().count(), 0);
                assert_eq!(
                    fs::read_dir(base.join("oxyde-archive-open/v1"))
                        .unwrap()
                        .count(),
                    1
                ); // only the cache lease
            }
            assert!(!temp.path().join("outside").exists());
        });
    });
}

#[test]
fn selected_seven_wrapper_verifies_unselected_block_crc_tail_before_publication() {
    native(|| {
        let temp = tempfile::tempdir().unwrap();
        let base = temp.path().join("cache");
        fs::create_dir(&base).unwrap();
        oxyde_lib::commands::archive::with_archive_cache_base_for_tests(&base, || {
            let source = temp.path().join("block.7z");
            let folder = temp.path().join("export");
            fs::create_dir(&folder).unwrap();
            for corrupt in [true, false] {
                seven_crc::block_crc_fixture_with_method(&source, corrupt, 0);
                let virtual_file = source.join("selected.txt");
                let cached =
                    oxyde_lib::commands::archive::prepare_archive_file_for_open(&virtual_file);
                let exported =
                    oxyde_lib::commands::archive::extract_archive_file_to(&virtual_file, &folder);
                if corrupt {
                    assert!(cached.is_err());
                    assert!(exported.is_err());
                    assert_eq!(fs::read_dir(&folder).unwrap().count(), 0);
                    assert_eq!(
                        fs::read_dir(base.join("oxyde-archive-open/v1"))
                            .unwrap()
                            .count(),
                        1
                    );
                } else {
                    assert_eq!(fs::read(cached.unwrap()).unwrap(), b"safe");
                    assert_eq!(fs::read(exported.unwrap()).unwrap(), b"safe");
                }
            }
        });
    });
}

#[test]
fn production_encoder_roundtrips_all_formats_qualities_dates_and_incompressible_sources() {
    native(|| {
        let temp = tempfile::tempdir().unwrap();
        let folder = temp.path().join("école_日本語");
        fs::create_dir(&folder).unwrap();
        fs::create_dir(folder.join("empty-dir")).unwrap();
        fs::write(folder.join("empty.txt"), []).unwrap();
        let path = folder.join("data.bin");
        let mut random = 0xdeadbeefu32;
        let content: Vec<_> = (0..(1024 * 1024 + 13))
            .map(|_| {
                random ^= random << 13;
                random ^= random >> 17;
                random ^= random << 5;
                random as u8
            })
            .collect();
        fs::write(&path, &content).unwrap();
        let modified = std::time::UNIX_EPOCH + Duration::new(1234567890, 123456700);
        fs::File::options()
            .write(true)
            .open(&path)
            .unwrap()
            .set_times(fs::FileTimes::new().set_modified(modified))
            .unwrap();
        let other = temp.path().join("other.txt");
        fs::write(&other, b"second selection").unwrap();
        for extension in ["zip", "7z", "tar", "tar.zst"] {
            for quality in ["fast", "normal", "best"] {
                let archive = temp.path().join(format!("roundtrip-{quality}.{extension}"));
                let op = operation(FileOpType::Compress);
                op.lock().unwrap().compression_quality = quality.into();
                FileOperationManager::perform_compress(
                    &op,
                    vec![folder.clone(), other.clone()],
                    Some(archive.clone()),
                )
                .unwrap();
                {
                    let op = op.lock().unwrap();
                    assert_eq!(op.total_bytes, op.processed_bytes);
                    assert_eq!(op.total_files, op.processed_files);
                }
                let target = temp.path().join(format!("decoded-{quality}-{extension}"));
                FileOperationManager::perform_extract(
                    &operation(FileOpType::Extract),
                    vec![archive],
                    Some(target.clone()),
                )
                .unwrap();
                let data = target.join("école_日本語/data.bin");
                assert_eq!(fs::read(&data).unwrap(), content);
                assert_eq!(
                    fs::read(target.join("other.txt")).unwrap(),
                    b"second selection"
                );
                assert!(target.join("école_日本語/empty-dir").is_dir());
                assert_eq!(
                    fs::metadata(target.join("école_日本語/empty.txt"))
                        .unwrap()
                        .len(),
                    0
                );
                if extension != "zip" {
                    let actual = fs::metadata(&data).unwrap().modified().unwrap();
                    if extension == "7z" {
                        assert_eq!(actual, modified);
                    } else {
                        assert_eq!(
                            actual
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap()
                                .as_secs(),
                            1234567890
                        );
                    }
                }
            }
        }
        assert_eq!(fs::read(path).unwrap(), content);
    });
}

#[test]
fn production_compression_boundaries_fail_before_worker_or_staging_and_preserve_collision() {
    use oxyde_lib::{
        models::error::{ArchiveLimitFailure, ArchiveLimitKind},
        utils::ArchiveLimits,
    };
    native(|| {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("source");
        fs::write(&source, b"1234").unwrap();
        let target = temp.path().join("archive.zip");
        let limits = ArchiveLimits {
            max_entries: 1,
            max_bytes: 4,
            max_depth: 1,
            ..ArchiveLimits::EXTRACTION
        };
        let error =
            with_worker_executable_for_tests(&temp.path().join("missing-worker.exe"), || {
                FileOperationManager::perform_compress_with_limits_for_tests(
                    &operation(FileOpType::Compress),
                    vec![source.clone()],
                    Some(target.clone()),
                    ArchiveLimits {
                        max_bytes: 3,
                        ..limits
                    },
                )
            })
            .unwrap_err();
        assert_eq!(
            ArchiveLimitFailure::from_message(&error),
            Some(ArchiveLimitFailure {
                kind: ArchiveLimitKind::SourceBytes,
                limit: 3
            })
        );
        assert_eq!(fs::read_dir(temp.path()).unwrap().count(), 1);
        fs::write(&target, b"existing").unwrap();
        let op = operation(FileOpType::Compress);
        FileOperationManager::perform_compress_with_limits_for_tests(
            &op,
            vec![source.clone()],
            Some(target.clone()),
            limits,
        )
        .unwrap();
        assert_eq!(fs::read(&target).unwrap(), b"existing");
        assert_ne!(op.lock().unwrap().destination.as_ref(), Some(&target));
        let folder = temp.path().join("tree");
        fs::create_dir(&folder).unwrap();
        fs::write(folder.join("file"), b"x").unwrap();
        for (limits, expected) in [
            (
                ArchiveLimits {
                    max_entries: 1,
                    max_depth: 2,
                    ..limits
                },
                ArchiveLimitKind::Entries,
            ),
            (
                ArchiveLimits {
                    max_entries: 2,
                    ..limits
                },
                ArchiveLimitKind::Depth,
            ),
            (
                ArchiveLimits {
                    max_duration: Duration::ZERO,
                    ..limits
                },
                ArchiveLimitKind::Seconds,
            ),
        ] {
            let rejected = temp.path().join("rejected.zip");
            let error = FileOperationManager::perform_compress_with_limits_for_tests(
                &operation(FileOpType::Compress),
                vec![folder.clone()],
                Some(rejected.clone()),
                limits,
            )
            .unwrap_err();
            assert_eq!(
                ArchiveLimitFailure::from_message(&error).unwrap().kind,
                expected
            );
            assert!(!rejected.exists());
        }
        assert_eq!(fs::read(source).unwrap(), b"1234");
    });
}

#[test]
fn production_turbo_changes_new_zip_entry_level_and_empty_encoders_finish() {
    native(|| {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("source");
        fs::write(&source, vec![b'x'; 1024 * 1024]).unwrap();
        let mut sizes = Vec::new();
        for turbo in [false, true] {
            let archive = temp.path().join(format!("turbo-{turbo}.zip"));
            let op = operation(FileOpType::Compress);
            {
                let op = op.lock().unwrap();
                op.turbo_flag.store(turbo, Ordering::Relaxed);
            }
            op.lock().unwrap().compression_quality = "best".into();
            FileOperationManager::perform_compress(
                &op,
                vec![source.clone()],
                Some(archive.clone()),
            )
            .unwrap();
            let mut zip = zip::ZipArchive::new(fs::File::open(archive).unwrap()).unwrap();
            sizes.push(zip.by_index(0).unwrap().compressed_size());
        }
        assert!(sizes[0] < sizes[1]);
        for extension in ["zip", "7z", "tar", "tar.zst"] {
            let archive = temp.path().join(format!("empty.{extension}"));
            FileOperationManager::perform_compress(
                &operation(FileOpType::Compress),
                vec![],
                Some(archive.clone()),
            )
            .unwrap();
            let target = temp.path().join(format!("empty-{extension}"));
            FileOperationManager::perform_extract(
                &operation(FileOpType::Extract),
                vec![archive],
                Some(target.clone()),
            )
            .unwrap();
            assert_eq!(fs::read_dir(target).unwrap().count(), 0);
        }
    });
}

#[test]
fn production_encoding_pause_cancel_and_launch_failure_never_publish() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("source");
    fs::write(&source, vec![42; 16 * 1024 * 1024]).unwrap();
    let archive = temp.path().join("result.zip");
    let op = operation(FileOpType::Compress);
    let (work, input, target) = (op.clone(), source.clone(), archive.clone());
    let child = std::thread::spawn(move || {
        native(|| FileOperationManager::perform_compress(&work, vec![input], Some(target)))
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
    let bytes = op.lock().unwrap().processed_bytes;
    std::thread::sleep(Duration::from_millis(150));
    assert_eq!(op.lock().unwrap().processed_bytes, bytes);
    assert!(fs::File::options().write(true).open(&source).is_err());
    op.lock()
        .unwrap()
        .cancel_flag
        .store(true, Ordering::Relaxed);
    assert!(child.join().unwrap().is_err());
    assert!(!archive.exists());
    assert_eq!(fs::read_dir(temp.path()).unwrap().count(), 1);
    let error = with_worker_executable_for_tests(&temp.path().join("absent.exe"), || {
        FileOperationManager::perform_compress(
            &operation(FileOpType::Compress),
            vec![source.clone()],
            Some(archive.clone()),
        )
    });
    assert!(error.is_err());
    assert!(!archive.exists());
    assert_eq!(fs::read_dir(temp.path()).unwrap().count(), 1);
    native(|| {
        FileOperationManager::perform_compress(
            &operation(FileOpType::Compress),
            vec![source],
            Some(archive),
        )
        .unwrap()
    });
}
