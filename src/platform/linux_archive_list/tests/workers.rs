use super::*;
use std::io::Write;

fn worker() -> PathBuf {
    let executable = std::env::current_exe().unwrap().parent().unwrap().parent().unwrap().join("oxyde-archive-list-worker");
    assert!(executable.is_file(), "Build the workers before these tests: cargo build --offline --bins");
    executable
}

fn tar(names: &[&str]) -> Vec<u8> {
    let mut builder = tar::Builder::new(Vec::new());
    for name in names {
        let mut header = tar::Header::new_gnu();
        header.set_size(4);
        header.set_mode(0o644);
        header.set_cksum();
        builder.append_data(&mut header, name, &b"safe"[..]).unwrap();
    }
    builder.into_inner().unwrap()
}

#[test]
fn listings_and_indexes_come_from_the_worker_with_real_paths() {
    let directory = tempfile::tempdir().unwrap();
    let archive = directory.path().join("école -- longue.zip");
    let mut writer = zip::ZipWriter::new(std::fs::File::create(&archive).unwrap());
    for name in ["root.txt", "docs/école.txt"] {
        writer.start_file(name, zip::write::SimpleFileOptions::default()).unwrap();
        writer.write_all(b"safe").unwrap();
    }
    writer.finish().unwrap();
    let root = list_in(&worker(), &archive, "", DEADLINE).unwrap();
    assert_eq!(root.len(), 2);
    let nested = list_in(&worker(), &archive, "docs", DEADLINE).unwrap();
    assert_eq!(nested[0].path, archive.join("docs/école.txt").to_string_lossy());
    assert_eq!(nested[0].size, 4);
    let all = index_in(&worker(), &archive, DEADLINE, &|| Ok(())).unwrap();
    assert!(all.iter().any(|entry| Path::new(&entry.path) == archive.join("docs/école.txt")));
}

#[test]
fn compressed_tars_and_seven_zip_keep_their_format_in_the_sandbox() {
    let directory = tempfile::tempdir().unwrap();
    let plain = tar(&["docs/note.txt"]);
    let mut gzip = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    gzip.write_all(&plain).unwrap();
    let zst = zstd::stream::encode_all(&plain[..], 1).unwrap();
    for (name, bytes) in [("a.tar", plain.clone()), ("a.tar.gz", gzip.finish().unwrap()), ("a.tar.zst", zst)] {
        let path = directory.path().join(name);
        std::fs::write(&path, bytes).unwrap();
        assert_eq!(list_in(&worker(), &path, "docs", DEADLINE).unwrap()[0].name, "note.txt", "{name}");
    }
    let path = directory.path().join("a.7z");
    let mut writer = sevenz_rust2::ArchiveWriter::create(&path).unwrap();
    let mut entry = sevenz_rust2::ArchiveEntry::new();
    entry.name = "docs/note.txt".into();
    entry.has_stream = true;
    entry.size = 4;
    writer.push_archive_entry(entry, Some(std::io::Cursor::new(b"safe"))).unwrap();
    writer.finish().unwrap();
    assert_eq!(index_in(&worker(), &path, DEADLINE, &|| Ok(())).unwrap()[0].name, "note.txt");
}

#[test]
fn a_hostile_name_or_damaged_archive_is_refused() {
    let directory = tempfile::tempdir().unwrap();
    let archive = directory.path().join("piège.zip");
    let mut writer = zip::ZipWriter::new(std::fs::File::create(&archive).unwrap());
    writer.start_file("../outside.txt", zip::write::SimpleFileOptions::default()).unwrap();
    writer.write_all(b"safe").unwrap();
    writer.finish().unwrap();
    assert!(list_in(&worker(), &archive, "", DEADLINE).is_err());
    let damaged = directory.path().join("cassé.zip");
    std::fs::write(&damaged, b"PK not an archive").unwrap();
    assert!(list_in(&worker(), &damaged, "", DEADLINE).is_err());
}

#[test]
fn cancellation_interrupts_the_search_before_the_worker() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("a.tar");
    std::fs::write(&path, tar(&["note.txt"])).unwrap();
    let error = index_in(&worker(), &path, DEADLINE, &|| Err(io::ErrorKind::Interrupted.into())).unwrap_err();
    assert!(!error.to_string().is_empty());
}
