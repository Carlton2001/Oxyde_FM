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

fn lz4(bytes: &[u8]) -> Vec<u8> {
    let mut encoder = lz4_flex::frame::FrameEncoder::new(Vec::new());
    encoder.write_all(bytes).unwrap();
    encoder.finish().unwrap()
}

fn lzma(bytes: &[u8]) -> Vec<u8> {
    let options = xz2::stream::LzmaOptions::new_preset(1).unwrap();
    let mut encoder = xz2::write::XzEncoder::new_stream(Vec::new(), xz2::stream::Stream::new_lzma_encoder(&options).unwrap());
    encoder.write_all(bytes).unwrap();
    encoder.finish().unwrap()
}

/// One compressed file per codec, holding `content`.
fn streams(content: &[u8]) -> Vec<(&'static str, Vec<u8>)> {
    let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    gz.write_all(content).unwrap();
    let mut xz = xz2::write::XzEncoder::new(Vec::new(), 1);
    xz.write_all(content).unwrap();
    let mut bz = bzip2::write::BzEncoder::new(Vec::new(), bzip2::Compression::default());
    bz.write_all(content).unwrap();
    vec![
        ("gz", gz.finish().unwrap()),
        ("xz", xz.finish().unwrap()),
        ("bz2", bz.finish().unwrap()),
        ("zst", zstd::stream::encode_all(content, 1).unwrap()),
        ("lz4", lz4(content)),
        ("lzma", lzma(content)),
    ]
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
    let lz4 = lz4(&tar);
    let lzma = lzma(&tar);
    for (extension, data) in [("tar", tar), ("tar.gz", gz.finish().unwrap()), ("tar.xz", xz.finish().unwrap()),
        ("tar.bz2", bz.finish().unwrap()), ("tar.zst", zst), ("tar.lz4", lz4), ("tar.lzma", lzma)] {
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
        for extension in ["zip", "7z", "tar", "tar.zst", "tar.gz", "tar.xz", "tar.bz2", "tar.lz4"] {
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

#[test]
fn a_single_compressed_file_extracts_beside_its_archive_and_opens() {
    confined(|| {
        let content: Vec<u8> = (0..1_500_000u32).map(|value| (value % 251) as u8).collect();
        for (extension, data) in streams(&content) {
            let temp = tempfile::tempdir().unwrap();
            let source = temp.path().join(format!("notes.txt.{extension}"));
            fs::write(&source, &data).unwrap();
            for expected in ["notes.txt", "notes (1).txt"] {
                let op = operation(FileOpType::Extract);
                FileOperationManager::perform_extract(&op, vec![source.clone()], Some(temp.path().join("notes.txt"))).unwrap();
                assert_eq!(fs::read(temp.path().join(expected)).unwrap(), content, "{extension}");
                let op = op.lock().unwrap();
                assert_eq!(op.destination.as_deref(), Some(temp.path().join(expected).as_path()), "{extension}");
                assert_eq!(op.total_bytes, content.len() as u64);
                assert_eq!(op.processed_bytes, content.len() as u64);
            }
            assert_eq!(fs::read_dir(temp.path()).unwrap().count(), 3, "{extension}: private folder left behind");
            assert_eq!(fs::read(&source).unwrap(), data);
            let base = temp.path().join("cache");
            fs::create_dir(&base).unwrap();
            oxyde_lib::commands::archive::with_archive_cache_base_for_tests(&base, || {
                let opened = oxyde_lib::commands::archive::prepare_archive_file_for_open(&source.join("notes.txt")).unwrap();
                assert_eq!(fs::read(opened).unwrap(), content, "{extension}");
                assert!(oxyde_lib::commands::archive::prepare_archive_file_for_open(&source.join("other.txt")).is_err());
            });
        }
    });
}

#[test]
fn a_foreign_or_truncated_compressed_file_publishes_nothing() {
    confined(|| {
        let content = vec![7u8; 200_000];
        for (extension, data) in streams(&content) {
            for bytes in [b"plain text, not compressed".to_vec(), data[..data.len() / 2].to_vec(), Vec::new()] {
                let temp = tempfile::tempdir().unwrap();
                let source = temp.path().join(format!("notes.txt.{extension}"));
                fs::write(&source, &bytes).unwrap();
                let result = FileOperationManager::perform_extract(&operation(FileOpType::Extract), vec![source.clone()], Some(temp.path().join("notes.txt")));
                assert!(result.is_err(), "{extension} accepted {} bytes", bytes.len());
                assert_eq!(fs::read_dir(temp.path()).unwrap().count(), 1, "{extension}");
            }
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

/// Executables grow by 5 % under Deflate: a fixed margin refused 1 GiB of them.
#[test]
#[ignore = "needs OXYDE_ARCHIVE_LARGE_FILE, about 1 GiB of data that Deflate expands"]
fn a_large_file_that_deflate_expands_joins_a_zip() {
    let Some(large) = std::env::var_os("OXYDE_ARCHIVE_LARGE_FILE") else { return };
    confined(|| {
        let temp = tempfile::tempdir_in(Path::new(&large).parent().unwrap()).unwrap();
        let archive = temp.path().join("a.zip");
        match std::env::var_os("OXYDE_ARCHIVE_TARGET") {
            Some(target) => { fs::copy(target, &archive).unwrap(); }
            None => zip(&archive, "petit.txt", b"safe", false),
        }
        let op = operation(FileOpType::Copy);
        op.lock().unwrap().compression_quality = "fast".into();
        FileOperationManager::perform_copy(&op, vec![large.into()], Some(archive.clone()), false, None).unwrap();
        println!("{} MiB", fs::metadata(&archive).unwrap().len() >> 20);
    });
}

/// Encrypted ZIP (AES-256) and 7z extract and open only with the remembered password.
#[test]
fn encrypted_archives_extract_and_open_with_their_password() {
    use oxyde_lib::models::error::ArchivePasswordFailure;
    use oxyde_lib::utils::archive_password::{forget, remember, ArchiveSecret};
    confined(|| {
        let temp = tempfile::tempdir().unwrap();
        let zip_path = temp.path().join("locked.zip");
        let mut writer = zip::ZipWriter::new(fs::File::create(&zip_path).unwrap());
        writer.start_file("note.txt", zip::write::SimpleFileOptions::default()
            .with_aes_encryption(zip::AesMode::Aes256, "correct horse")).unwrap();
        writer.write_all(b"secret").unwrap();
        writer.finish().unwrap();
        let source = temp.path().join("source");
        fs::create_dir(&source).unwrap();
        fs::write(source.join("note.txt"), b"secret").unwrap();
        let seven_path = temp.path().join("locked.7z");
        sevenz_rust2::compress_to_path_encrypted(&source, &seven_path, "correct horse".into()).unwrap();
        let base = temp.path().join("cache");
        fs::create_dir(&base).unwrap();
        for (index, archive) in [zip_path, seven_path].into_iter().enumerate() {
            let target = temp.path().join(format!("output-{index}"));
            let extract = |target: &Path| FileOperationManager::perform_extract(
                &operation(FileOpType::Extract), vec![archive.clone()], Some(target.to_path_buf()));
            let refusal = |error: String| ArchivePasswordFailure::from_message(&error);
            let error = extract(&target).unwrap_err();
            assert_eq!(refusal(error), Some(ArchivePasswordFailure::Required), "{archive:?}");
            assert!(!target.join("note.txt").exists());
            remember(&archive, ArchiveSecret::new("wrong".into()).unwrap());
            assert_eq!(refusal(extract(&target).unwrap_err()), Some(ArchivePasswordFailure::Wrong), "{archive:?}");
            remember(&archive, ArchiveSecret::new("correct horse".into()).unwrap());
            extract(&target).unwrap();
            assert_eq!(fs::read(target.join("note.txt")).unwrap(), b"secret");
            oxyde_lib::commands::archive::with_archive_cache_base_for_tests(&base, || {
                let cached = oxyde_lib::commands::archive::prepare_archive_file_for_open(&archive.join("note.txt")).unwrap();
                assert_eq!(fs::read(cached).unwrap(), b"secret");
            });
            forget(&archive);
        }
    });
}

/// A protected ZIP or 7z is created encrypted and stays encrypted when Oxyde adds to it.
#[test]
fn protected_archives_are_created_and_rebuilt_encrypted() {
    use oxyde_lib::models::error::ArchivePasswordFailure;
    use oxyde_lib::utils::archive_password::{forget, remember, ArchiveSecret};
    confined(|| {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("dossier");
        fs::create_dir(&source).unwrap();
        fs::write(source.join("note.txt"), b"original").unwrap();
        let added = temp.path().join("added.txt");
        fs::write(&added, b"copied").unwrap();
        for extension in ["zip", "7z"] {
            let archive = temp.path().join(format!("protected.{extension}"));
            remember(&archive, ArchiveSecret::new("correct horse".into()).unwrap());
            FileOperationManager::perform_compress(&operation(FileOpType::Compress), vec![source.clone()], Some(archive.clone())).unwrap();
            FileOperationManager::perform_copy(&operation(FileOpType::Copy), vec![added.clone()], Some(archive.clone()), false, None).unwrap();
            forget(&archive);
            let target = temp.path().join(format!("back-{extension}"));
            let error = FileOperationManager::perform_extract(&operation(FileOpType::Extract), vec![archive.clone()], Some(target.clone())).unwrap_err();
            assert_eq!(ArchivePasswordFailure::from_message(&error), Some(ArchivePasswordFailure::Required), "{extension}");
            remember(&archive, ArchiveSecret::new("correct horse".into()).unwrap());
            FileOperationManager::perform_extract(&operation(FileOpType::Extract), vec![archive.clone()], Some(target.clone())).unwrap();
            assert_eq!(fs::read(target.join("dossier/note.txt")).unwrap(), b"original", "{extension}");
            assert_eq!(fs::read(target.join("added.txt")).unwrap(), b"copied", "{extension}");
            forget(&archive);
        }
    });
}

fn rar(password: Option<&str>, headers: bool, solid: bool) -> Vec<u8> {
    let mut builder = rars::Builder::new(rars::ArchiveVersion::Rar50)
        .password(password.map(|value| value.as_bytes().to_vec()))
        .header_encryption(headers)
        .solid(solid);
    builder.add_directory(b"docs".to_vec(), None, None).unwrap();
    builder.add_bytes(b"docs/note.txt".to_vec(), b"secret payload".to_vec(), None, None).unwrap();
    builder.add_bytes(b"big.bin".to_vec(), (0..400_000u32).map(|value| (value % 253) as u8).collect(), None, None).unwrap();
    builder.add_bytes(b"last.txt".to_vec(), b"tail".to_vec(), None, None).unwrap();
    builder.to_bytes().unwrap()
}

/// RAR is read through `rars` in the worker: extraction, a selected file, and passwords.
#[test]
fn rar_archives_extract_and_open_with_or_without_password() {
    use oxyde_lib::models::error::ArchivePasswordFailure;
    use oxyde_lib::utils::archive_password::{forget, remember, ArchiveSecret};
    confined(|| {
        let temp = tempfile::tempdir().unwrap();
        let base = temp.path().join("cache");
        fs::create_dir(&base).unwrap();
        let cases = [
            ("plain.rar", None, false, false),
            ("solid.rar", None, false, true),
            ("content.rar", Some("correct horse"), false, true),
            ("headers.rar", Some("correct horse"), true, false),
        ];
        for (name, password, headers, solid) in cases {
            let archive = temp.path().join(name);
            fs::write(&archive, rar(password, headers, solid)).unwrap();
            let target = temp.path().join(format!("out-{name}"));
            let extract = || FileOperationManager::perform_extract(&operation(FileOpType::Extract), vec![archive.clone()], Some(target.clone()));
            if password.is_some() {
                let refusal = ArchivePasswordFailure::from_message(&extract().unwrap_err());
                assert_eq!(refusal, Some(ArchivePasswordFailure::Required), "{name}");
                assert!(!target.join("last.txt").exists(), "{name}");
                remember(&archive, ArchiveSecret::new("wrong".into()).unwrap());
                assert_eq!(ArchivePasswordFailure::from_message(&extract().unwrap_err()), Some(ArchivePasswordFailure::Wrong), "{name}");
                remember(&archive, ArchiveSecret::new("correct horse".into()).unwrap());
            }
            extract().unwrap();
            assert_eq!(fs::read(target.join("docs/note.txt")).unwrap(), b"secret payload", "{name}");
            assert_eq!(fs::read(target.join("last.txt")).unwrap(), b"tail", "{name}");
            assert_eq!(fs::read(target.join("big.bin")).unwrap().len(), 400_000, "{name}");
            oxyde_lib::commands::archive::with_archive_cache_base_for_tests(&base, || {
                for (member, expected) in [("docs/note.txt", &b"secret payload"[..]), ("last.txt", &b"tail"[..])] {
                    let cached = oxyde_lib::commands::archive::prepare_archive_file_for_open(&archive.join(member)).unwrap();
                    assert_eq!(fs::read(cached).unwrap(), expected, "{name} {member}");
                }
            });
            forget(&archive);
        }
    });
}
