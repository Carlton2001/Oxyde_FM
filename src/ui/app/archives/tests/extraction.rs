use super::extraction_jobs;
use oxyde_lib::models::FileEntry;
use std::path::PathBuf;

fn real(path: &str) -> FileEntry {
    FileEntry {
        path: path.into(),
        ..Default::default()
    }
}

fn member(path: &str, archive: &str) -> FileEntry {
    FileEntry {
        archive_path: Some(std::sync::Arc::new(archive.into())),
        ..real(path)
    }
}

#[test]
fn compression_includes_selected_archives_but_excludes_virtual_entries() {
    let sources = super::compression_sources(vec![
        real("folder/second.zip"),
        real("folder/readme.txt"),
        real("folder/first.zip"),
        member("folder/first.zip/nested.txt", "folder/first.zip"),
    ]);
    assert_eq!(
        sources,
        vec![
            PathBuf::from("folder/first.zip"),
            PathBuf::from("folder/readme.txt"),
            PathBuf::from("folder/second.zip"),
        ]
    );
}

#[test]
fn multiple_zip_archives_each_get_their_own_extraction_job() {
    let jobs = extraction_jobs(vec![real("folder/first.zip"), real("folder/second.zip")]);
    assert_eq!(
        jobs,
        vec![
            (
                PathBuf::from("folder/first.zip"),
                PathBuf::from("folder/first")
            ),
            (
                PathBuf::from("folder/second.zip"),
                PathBuf::from("folder/second")
            ),
        ]
    );
}

#[test]
fn mixed_selection_only_extracts_supported_archives_in_their_own_directories() {
    let jobs = extraction_jobs(vec![
        real("first/data.zip"),
        real("second/backup.tar.gz"),
        real("first/readme.txt"),
        real("first/unsupported.rar"),
        member("first/container.zip/nested.zip", "first/container.zip"),
    ]);
    assert_eq!(
        jobs,
        vec![
            (PathBuf::from("first/data.zip"), PathBuf::from("first/data")),
            (
                PathBuf::from("second/backup.tar.gz"),
                PathBuf::from("second/backup")
            ),
        ]
    );
}

#[test]
fn real_archive_named_directories_and_their_archives_keep_their_actions() {
    for name in ["ordinary.zip", "ordinary.7z", "ordinary.tar.gz", "ordinary.iso", "ordinary.img"] {
        let folder = FileEntry {
            is_dir: true,
            ..real(name)
        };
        let archive = real(&format!("{name}/inner.zip"));
        let archived = member(
            &format!("{name}/inner.zip/nested.zip"),
            &format!("{name}/inner.zip"),
        );
        assert_eq!(
            super::compression_sources(vec![folder.clone(), archive.clone(), archived.clone()])
                .len(),
            2
        );
        assert_eq!(
            extraction_jobs(vec![folder, archive.clone(), archived]),
            vec![(
                PathBuf::from(&archive.path),
                PathBuf::from(name).join("inner")
            ),]
        );
    }
}
