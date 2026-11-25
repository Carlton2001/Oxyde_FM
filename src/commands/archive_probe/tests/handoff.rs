use super::*;
use std::io::Write;

fn archive(dir: &Path) -> PathBuf {
    let path = dir.join("probe.zip");
    let mut zip = zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
    zip.start_file("a.txt", zip::write::SimpleFileOptions::default()).unwrap();
    zip.write_all(b"a").unwrap();
    zip.finish().unwrap();
    path
}

// One test: the kept listing is process-wide.
#[test]
fn a_probed_root_is_taken_once_and_only_while_unchanged() {
    let dir = tempfile::tempdir().unwrap();
    let path = archive(dir.path());
    let text = path.to_string_lossy().into_owned();

    probe(text.clone()).unwrap();
    assert!(take(&dir.path().join("other.zip")).is_none());
    let entries = take(&path).unwrap();
    assert_eq!(entries.iter().map(|entry| entry.name.as_str()).collect::<Vec<_>>(), ["a.txt"]);
    assert!(take(&path).is_none());

    probe(text.clone()).unwrap();
    forget();
    assert!(take(&path).is_none());

    probe(text).unwrap();
    std::fs::OpenOptions::new().append(true).open(&path).unwrap().write_all(b"!").unwrap();
    assert!(take(&path).is_none());
}
