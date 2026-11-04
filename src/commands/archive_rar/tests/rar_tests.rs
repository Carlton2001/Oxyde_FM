//! RAR fixtures written by `rars` itself (a test-only feature): listing, links and
//! password refusals. Decoding through the broker is covered by the worker tests.
use super::*;

pub(crate) fn fixture(password: Option<&str>, headers: bool) -> Vec<u8> {
    let mut builder = rars::Builder::new(rars::ArchiveVersion::Rar50)
        .password(password.map(|value| value.as_bytes().to_vec()))
        .header_encryption(headers);
    builder.add_directory(b"docs".to_vec(), None, None).unwrap();
    builder.add_bytes(b"docs/note.txt".to_vec(), b"secret payload".to_vec(), None, None).unwrap();
    builder.add_bytes(b"root.txt".to_vec(), vec![7; 300_000], None, None).unwrap();
    builder.to_bytes().unwrap()
}

fn write(folder: &Path, name: &str, bytes: Vec<u8>) -> std::path::PathBuf {
    let path = folder.join(name);
    std::fs::write(&path, bytes).unwrap();
    path
}

fn refusal(result: Result<Vec<FileEntry>, CommandError>) -> Option<ArchivePasswordFailure> {
    ArchivePasswordFailure::from_message(&result.unwrap_err().to_string())
}

#[test]
fn members_list_with_their_paths_sizes_and_folders() {
    let folder = tempfile::tempdir().unwrap();
    let path = write(folder.path(), "plain.rar", fixture(None, false));
    let files = indexed_file(&path, File::open(&path).unwrap(), ArchiveLimits::LISTING, &|| Ok(())).unwrap();
    let note = files.iter().find(|file| file.name == "note.txt").unwrap();
    assert_eq!(note.path, path.join("docs/note.txt").to_string_lossy());
    assert_eq!(note.size, 14);
    assert!(files.iter().any(|file| file.name == "docs" && file.is_dir));
    assert_eq!(verify(&path, File::open(&path).unwrap(), 1024), None);
}

#[test]
fn encrypted_headers_need_the_password_to_list_and_content_to_verify() {
    let folder = tempfile::tempdir().unwrap();
    let locked = write(folder.path(), "locked.rar", fixture(Some("correct horse"), true));
    let content = write(folder.path(), "content.rar", fixture(Some("correct horse"), false));
    let list = |path: &Path| indexed_file(path, File::open(path).unwrap(), ArchiveLimits::LISTING, &|| Ok(()));
    let check = |path: &Path| verify(path, File::open(path).unwrap(), 1024);
    assert_eq!(refusal(list(&locked)), Some(ArchivePasswordFailure::Required));
    assert_eq!(list(&content).unwrap().len(), 3);
    for path in [&locked, &content] {
        assert_eq!(check(path), Some(ArchivePasswordFailure::Required), "{path:?}");
        archive_password::remember(path, ArchiveSecret::new("wrong".into()).unwrap());
        assert_eq!(check(path), Some(ArchivePasswordFailure::Wrong), "{path:?}");
        archive_password::remember(path, ArchiveSecret::new("correct horse".into()).unwrap());
        assert_eq!(check(path), None, "{path:?}");
        assert_eq!(list(path).unwrap().len(), 3, "{path:?}");
        archive_password::forget(path);
    }
}

#[test]
fn links_are_refused() {
    let mut builder = rars::Builder::new(rars::ArchiveVersion::Rar50);
    builder.add_bytes(b"target.txt".to_vec(), b"x".to_vec(), None, None).unwrap();
    builder.add_unix_symlink(b"link".to_vec(), b"/etc/passwd".to_vec(), false, None, None).unwrap();
    let folder = tempfile::tempdir().unwrap();
    let path = write(folder.path(), "link.rar", builder.to_bytes().unwrap());
    assert!(indexed_file(&path, File::open(&path).unwrap(), ArchiveLimits::LISTING, &|| Ok(())).is_err());
}
