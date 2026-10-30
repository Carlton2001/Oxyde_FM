//! The production archive path on Linux: codecs in confined workers, effects in the
//! parent. Unit tests compile the local codecs instead and do not prove this one.
#![cfg(target_os = "linux")]
use oxyde_lib::{
    systems::file_ops::{FileOpType, FileOperation, FileOperationManager},
    utils::archive_io_protocol::with_worker_executable_for_tests,
};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    sync::{atomic::Ordering, Arc, Mutex},
};
#[path = "support/iso.rs"]
mod iso;

fn confined<T>(run: impl FnOnce() -> T) -> T {
    with_worker_executable_for_tests(Path::new(env!("CARGO_BIN_EXE_oxyde-archive-io-worker")), run)
}

fn operation(kind: FileOpType) -> Arc<Mutex<FileOperation>> {
    Arc::new(Mutex::new(FileOperation::new(kind, vec![], None, None)))
}

fn zip(path: &Path, name: &str, bytes: &[u8], unsafe_tail: bool) {
    let mut zip = zip::ZipWriter::new(fs::File::create(path).unwrap());
    zip.start_file(name, zip::write::SimpleFileOptions::default()).unwrap();
    zip.write_all(bytes).unwrap();
    if unsafe_tail {
        zip.start_file("../outside", zip::write::SimpleFileOptions::default()).unwrap();
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
    tar.append_data(&mut header, "docs/école.txt", &b"safe"[..]).unwrap();
    let tar = tar.into_inner().unwrap();
    let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    gz.write_all(&tar).unwrap();
    let mut xz = xz2::write::XzEncoder::new(Vec::new(), 1);
    xz.write_all(&tar).unwrap();
    let mut bz = bzip2::write::BzEncoder::new(Vec::new(), bzip2::Compression::default());
    bz.write_all(&tar).unwrap();
    let zst = zstd::stream::encode_all(&tar[..], 1).unwrap();
    for (extension, data) in [("tar", tar), ("tar.gz", gz.finish().unwrap()), ("tar.xz", xz.finish().unwrap()),
        ("tar.bz2", bz.finish().unwrap()), ("tar.zst", zst)] {
        let path = root.join(format!("source.{extension}"));
        fs::write(&path, data).unwrap();
        fixtures.push((path, "docs/école.txt", b"safe"));
    }
    let path = root.join("source.7z");
    let mut writer = sevenz_rust2::ArchiveWriter::create(&path).unwrap();
    writer.push_archive_entry(sevenz_rust2::ArchiveEntry::new_file("docs/école.txt"), Some(std::io::Cursor::new(b"safe"))).unwrap();
    writer.finish().unwrap();
    fixtures.push((path, "docs/école.txt", b"safe"));
    let path = root.join("source.iso");
    iso::iso_fixture(&path);
    fixtures.push((path, "DOCS/README.TXT", b"ISO content"));
    fixtures
}

#[test]
fn every_family_extracts_through_the_worker_and_keeps_its_source() {
    confined(|| {
        let temp = tempfile::tempdir().unwrap();
        for (index, (source, name, expected)) in fixtures(temp.path()).into_iter().enumerate() {
            let before = fs::read(&source).unwrap();
            let target = temp.path().join(format!("output-{index}"));
            let op = operation(FileOpType::Extract);
            FileOperationManager::perform_extract(&op, vec![source.clone()], Some(target.clone())).unwrap();
            assert_eq!(fs::read(target.join(name)).unwrap(), expected, "{source:?}");
            let op = op.lock().unwrap();
            assert_eq!(op.total_files, op.processed_files);
            assert_eq!(op.total_bytes, op.processed_bytes);
            assert_eq!(fs::read(&source).unwrap(), before);
        }
    });
}

#[test]
fn truncated_archives_publish_nothing() {
    confined(|| {
        let temp = tempfile::tempdir().unwrap();
        for (index, (archive, _, _)) in fixtures(temp.path()).into_iter().enumerate() {
            let bytes = fs::read(&archive).unwrap();
            fs::write(&archive, &bytes[..32.min(bytes.len() / 2)]).unwrap();
            let target = temp.path().join(format!("output-{index}"));
            assert!(FileOperationManager::perform_extract(&operation(FileOpType::Extract), vec![archive.clone()], Some(target.clone())).is_err(), "{archive:?}");
            assert!(!target.exists(), "{archive:?}");
        }
    });
}

#[test]
fn a_rejected_entry_and_a_cancellation_remove_only_the_owned_root() {
    confined(|| {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("bad.zip");
        zip(&source, "safe.txt", b"safe", true);
        let target = temp.path().join("output");
        assert!(FileOperationManager::perform_extract(&operation(FileOpType::Extract), vec![source.clone()], Some(target.clone())).is_err());
        assert!(!target.exists());
        assert!(!temp.path().join("outside").exists());
        zip(&source, "safe.txt", b"safe", false);
        let op = operation(FileOpType::Extract);
        op.lock().unwrap().cancel_flag.store(true, Ordering::Relaxed);
        assert!(FileOperationManager::perform_extract(&op, vec![source], Some(target.clone())).is_err());
        assert!(!target.exists());
    });
}

#[test]
fn a_link_inside_a_tar_is_never_created() {
    confined(|| {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("link.tar");
        let mut tar = tar::Builder::new(fs::File::create(&source).unwrap());
        let mut header = tar::Header::new_gnu();
        header.set_entry_type(tar::EntryType::Symlink);
        header.set_size(0);
        header.set_mode(0o777);
        header.set_cksum();
        tar.append_link(&mut header, "lien", "/etc/passwd").unwrap();
        tar.finish().unwrap();
        drop(tar);
        let target = temp.path().join("output");
        assert!(FileOperationManager::perform_extract(&operation(FileOpType::Extract), vec![source], Some(target.clone())).is_err());
        assert!(fs::symlink_metadata(target.join("lien")).is_err());
    });
}

#[test]
fn a_selected_file_opens_and_exports_from_every_family() {
    confined(|| {
        let temp = tempfile::tempdir().unwrap();
        let base = temp.path().join("cache");
        fs::create_dir(&base).unwrap();
        oxyde_lib::commands::archive::with_archive_cache_base_for_tests(&base, || {
            for (index, (source, name, expected)) in fixtures(temp.path()).into_iter().enumerate() {
                let virtual_file = source.join(name);
                let cached = oxyde_lib::commands::archive::prepare_archive_file_for_open(&virtual_file).unwrap();
                assert_eq!(fs::read(&cached).unwrap(), expected, "{source:?}");
                let folder = temp.path().join(format!("export-{index}"));
                fs::create_dir(&folder).unwrap();
                let first = oxyde_lib::commands::archive::extract_archive_file_to(&virtual_file, &folder).unwrap();
                let second = oxyde_lib::commands::archive::extract_archive_file_to(&virtual_file, &folder).unwrap();
                assert_ne!(first, second);
                assert_eq!(fs::read(&first).unwrap(), expected);
                assert_eq!(fs::read_dir(&folder).unwrap().count(), 2, "staging left behind");
                assert!(oxyde_lib::commands::archive::prepare_archive_file_for_open(&source.join("absent.txt")).is_err());
            }
        });
    });
}

#[test]
fn every_writable_format_is_built_copied_into_and_read_back() {
    confined(|| {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("école");
        fs::create_dir(&source).unwrap();
        fs::write(source.join("note.txt"), b"original").unwrap();
        fs::create_dir(source.join("vide")).unwrap();
        let added = temp.path().join("added.txt");
        fs::write(&added, b"copied").unwrap();
        for extension in ["zip", "7z", "tar", "tar.zst"] {
            let archive = temp.path().join(format!("mutate.{extension}"));
            FileOperationManager::perform_compress(&operation(FileOpType::Compress), vec![source.clone()], Some(archive.clone())).unwrap();
            FileOperationManager::perform_copy(&operation(FileOpType::Copy), vec![added.clone()], Some(archive.clone()), false, None).unwrap();
            let target = temp.path().join(format!("back-{}", extension.replace('.', "-")));
            FileOperationManager::perform_extract(&operation(FileOpType::Extract), vec![archive.clone()], Some(target.clone())).unwrap();
            assert_eq!(fs::read(target.join("école/note.txt")).unwrap(), b"original", "{extension}");
            assert_eq!(fs::read(target.join("added.txt")).unwrap(), b"copied", "{extension}");
            assert!(target.join("école/vide").is_dir(), "{extension}");
        }
    });
}

/// `OXYDE_ARCHIVE_BENCH_DIR=<folder> cargo test --offline --test linux_archive_workers -- --ignored --nocapture`
#[test]
#[ignore = "measures throughput on a chosen filesystem"]
fn throughput() {
    let Some(root) = std::env::var_os("OXYDE_ARCHIVE_BENCH_DIR") else { return };
    confined(|| {
        let temp = tempfile::tempdir_in(root).unwrap();
        let source = temp.path().join("source");
        fs::create_dir(&source).unwrap();
        let mut block = vec![0u8; 1024 * 1024];
        let mut value = 1u64;
        for (index, chunk) in block.chunks_mut(8).enumerate() {
            value = value.wrapping_mul(0x9E37_79B9_7F4A_7C15).wrapping_add(index as u64);
            chunk.copy_from_slice(&(value >> (index % 32)).to_le_bytes()[..chunk.len()]);
        }
        let mut large = fs::File::create(source.join("large.bin")).unwrap();
        for _ in 0..1024 { large.write_all(&block).unwrap(); }
        for index in 0..2000 { fs::write(source.join(format!("small-{index}.txt")), &block[..4096]).unwrap(); }
        for extension in ["tar", "zip", "tar.zst"] {
            let archive = temp.path().join(format!("bench.{extension}"));
            let started = std::time::Instant::now();
            FileOperationManager::perform_compress(&operation(FileOpType::Compress), vec![source.clone()], Some(archive.clone())).unwrap();
            let encoded = started.elapsed();
            let target = temp.path().join(format!("out-{}", extension.replace('.', "-")));
            let started = std::time::Instant::now();
            FileOperationManager::perform_extract(&operation(FileOpType::Extract), vec![archive.clone()], Some(target.clone())).unwrap();
            println!("{extension}: {} MiB, encode {encoded:.2?}, decode {:.2?}", fs::metadata(&archive).unwrap().len() >> 20, started.elapsed());
            fs::remove_dir_all(&target).unwrap();
        }
    });
}
