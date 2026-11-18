use super::*;

#[test]
fn an_existing_folder_in_an_archive_reports_already_exists() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("source.txt");
    std::fs::write(&source, b"source").unwrap();
    let archive = directory.path().join("test.zip");
    FileOperationManager::perform_compress(&mutation_operation("normal"), vec![source], Some(archive.clone()))
        .unwrap();
    let folder = archive.join("New folder");
    create_directory(&folder, "normal").unwrap();
    // "New folder (1)" follows only when this kind survives.
    let error = create_directory(&folder, "normal").unwrap_err();
    assert_eq!(error.kind(), std::io::ErrorKind::AlreadyExists);
}

/// The production workspace creates folders through the journal.
#[test]
fn journal_directories_report_an_existing_name_as_already_existing() {
    let directory = tempfile::tempdir().unwrap();
    let parent = crate::platform::platform().hold_archive_directory(directory.path()).unwrap();
    let folder = directory.path().join("New folder");
    crate::platform::platform().create_journal_directory(&parent, &folder, false).unwrap();
    let error = crate::platform::platform()
        .create_journal_directory(&parent, &folder, false)
        .unwrap_err();
    assert_eq!(error.kind(), std::io::ErrorKind::AlreadyExists);
}

#[test]
fn numbered_folders_in_an_archive_take_the_first_free_name() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("source.txt");
    std::fs::write(&source, b"source").unwrap();
    let archive = directory.path().join("test.zip");
    FileOperationManager::perform_compress(&mutation_operation("normal"), vec![source], Some(archive.clone()))
        .unwrap();
    create_directory(&archive.join("New folder"), "normal").unwrap();
    let mut names = (0..).map(|counter| match counter {
        0 => "New folder".to_owned(),
        counter => format!("New folder ({counter})"),
    });
    let created = create_numbered_directory(&archive, &mut names, "normal").unwrap();
    assert_eq!(created, archive.join("New folder (1)"));
    let error = create_directory(&created, "normal").unwrap_err();
    assert_eq!(error.kind(), std::io::ErrorKind::AlreadyExists);
}
