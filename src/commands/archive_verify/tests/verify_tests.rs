use super::*;
use std::io::{Cursor, Write};

fn refusal(path: &Path) -> Option<ArchivePasswordFailure> {
    verify_file(path, File::open(path).unwrap()).err()
        .and_then(|error| ArchivePasswordFailure::from_message(&error.to_string()))
}

#[test]
fn the_smallest_encrypted_member_decides_and_clear_archives_pass() {
    let folder = tempfile::tempdir().unwrap();
    let zip_path = folder.path().join("locked.zip");
    let mut writer = zip::ZipWriter::new(File::create(&zip_path).unwrap());
    writer.start_file("clear.txt", zip::write::SimpleFileOptions::default()).unwrap();
    writer.write_all(b"clear").unwrap();
    writer.start_file("note.txt", zip::write::SimpleFileOptions::default()
        .with_aes_encryption(zip::AesMode::Aes256, "correct horse")).unwrap();
    writer.write_all(b"secret").unwrap();
    writer.finish().unwrap();
    let source = folder.path().join("source");
    std::fs::create_dir(&source).unwrap();
    std::fs::write(source.join("note.txt"), b"secret").unwrap();
    let seven_path = folder.path().join("locked.7z");
    sevenz_rust2::compress_to_path_encrypted(&source, &seven_path, "correct horse".into()).unwrap();
    for path in [&zip_path, &seven_path] {
        assert_eq!(refusal(path), Some(ArchivePasswordFailure::Required), "{path:?}");
        archive_password::remember(path, ArchiveSecret::new("wrong".into()).unwrap());
        assert_eq!(refusal(path), Some(ArchivePasswordFailure::Wrong), "{path:?}");
        archive_password::remember(path, ArchiveSecret::new("correct horse".into()).unwrap());
        assert_eq!(refusal(path), None, "{path:?}");
        archive_password::forget(path);
    }
    let clear = folder.path().join("clear.zip");
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    writer.start_file("a.txt", zip::write::SimpleFileOptions::default()).unwrap();
    std::fs::write(&clear, writer.finish().unwrap().into_inner()).unwrap();
    assert_eq!(refusal(&clear), None);
}

#[test]
fn zipcrypto_checks_the_key_of_other_members_when_the_sample_has_no_checksum() {
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("stored.zip");
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Stored);
    let options = zip::unstable::write::FileOptionsExt::with_deprecated_encryption(options, b"correct horse").unwrap();
    let mut writer = zip::ZipWriter::new(File::create(&path).unwrap());
    // Both members exceed the sample, so no checksum is ever read.
    for (name, size) in [("a.bin", SAMPLE + 1), ("b.bin", SAMPLE + 2)] {
        writer.start_file(name, options).unwrap();
        writer.write_all(&vec![7; size as usize]).unwrap();
    }
    writer.finish().unwrap();
    let opens = |password: &str, name: &str| {
        let mut archive = zip::ZipArchive::new(File::open(&path).unwrap()).unwrap();
        let index = archive.index_for_name(name).unwrap();
        let opened = archive.by_index_decrypt(index, password.as_bytes()).is_ok();
        opened
    };
    // A wrong key passing the sampled member's one-byte check, failing the other's.
    let wrong = (0..100_000)
        .map(|attempt| format!("wrong {attempt}"))
        .find(|password| opens(password, "a.bin") && !opens(password, "b.bin"))
        .unwrap();
    archive_password::remember(&path, ArchiveSecret::new(wrong.into()).unwrap());
    assert_eq!(refusal(&path), Some(ArchivePasswordFailure::Wrong));
    archive_password::remember(&path, ArchiveSecret::new("correct horse".into()).unwrap());
    assert_eq!(refusal(&path), None);
    archive_password::forget(&path);
}
