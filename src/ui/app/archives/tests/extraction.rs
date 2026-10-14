use super::extraction_jobs;
use std::path::PathBuf;

#[test]
fn compression_includes_selected_archives_but_excludes_virtual_entries() {
    let sources = super::compression_sources(vec![
        "folder/second.zip".into(), "folder/readme.txt".into(),
        "folder/first.zip".into(), "folder/first.zip/nested.txt".into(),
    ]);
    assert_eq!(sources, vec![
        PathBuf::from("folder/first.zip"), PathBuf::from("folder/readme.txt"),
        PathBuf::from("folder/second.zip"),
    ]);
}

#[test]
fn multiple_zip_archives_each_get_their_own_extraction_job() {
    let jobs = extraction_jobs(vec!["folder/first.zip".into(), "folder/second.zip".into()]);
    assert_eq!(jobs, vec![
        (PathBuf::from("folder/first.zip"), PathBuf::from("folder/first")),
        (PathBuf::from("folder/second.zip"), PathBuf::from("folder/second")),
    ]);
}

#[test]
fn mixed_selection_only_extracts_supported_archives_in_their_own_directories() {
    let jobs = extraction_jobs(vec![
        "first/data.zip".into(), "second/backup.tar.gz".into(),
        "first/readme.txt".into(), "first/unsupported.rar".into(),
        "first/container.zip/nested.zip".into(),
    ]);
    assert_eq!(jobs, vec![
        (PathBuf::from("first/data.zip"), PathBuf::from("first/data")),
        (PathBuf::from("second/backup.tar.gz"), PathBuf::from("second/backup")),
    ]);
}
