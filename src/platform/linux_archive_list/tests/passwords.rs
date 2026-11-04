//! A 7z with encrypted headers lists only with its password, sent on the worker's input.
use super::*;
use crate::models::error::ArchivePasswordFailure;
use crate::utils::archive_password::{forget, remember, ArchiveSecret};

fn worker() -> PathBuf {
    let executable = std::env::current_exe().unwrap().parent().unwrap().parent().unwrap().join("oxyde-archive-list-worker");
    assert!(executable.is_file(), "Build the workers before these tests: cargo build --offline --bins");
    executable
}

fn refusal(result: Result<Vec<FileEntry>, CommandError>) -> Option<ArchivePasswordFailure> {
    ArchivePasswordFailure::from_message(&result.unwrap_err().to_string())
}

#[test]
fn encrypted_seven_zip_headers_list_with_the_remembered_password() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("source");
    std::fs::create_dir_all(source.join("docs")).unwrap();
    std::fs::write(source.join("docs/note.txt"), b"safe").unwrap();
    let archive = directory.path().join("locked.7z");
    sevenz_rust2::compress_to_path_encrypted(&source, &archive, "correct horse".into()).unwrap();
    let verified = || verify_in(&worker(), &archive, DEADLINE).err()
        .and_then(|error| ArchivePasswordFailure::from_message(&error.to_string()));
    assert_eq!(refusal(list_in(&worker(), &archive, "", DEADLINE)), Some(ArchivePasswordFailure::Required));
    assert_eq!(verified(), Some(ArchivePasswordFailure::Required));
    remember(&archive, ArchiveSecret::new("wrong".into()).unwrap());
    assert_eq!(refusal(list_in(&worker(), &archive, "", DEADLINE)), Some(ArchivePasswordFailure::Wrong));
    assert_eq!(verified(), Some(ArchivePasswordFailure::Wrong));
    remember(&archive, ArchiveSecret::new("correct horse".into()).unwrap());
    assert_eq!(verified(), None);
    let nested = list_in(&worker(), &archive, "docs", DEADLINE).unwrap();
    assert_eq!(nested[0].name, "note.txt");
    let all = index_in(&worker(), &archive, DEADLINE, &|| Ok(())).unwrap();
    assert!(all.iter().any(|entry| Path::new(&entry.path) == archive.join("docs/note.txt")));
    forget(&archive);
}
