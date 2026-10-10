use super::*;

#[test]
#[cfg(target_os = "windows")]
fn archive_move_publication_conflict_preserves_held_real_sources() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source.txt");
    let archive = root.path().join("target.zip");
    std::fs::write(&source, b"source data").unwrap();
    FileOperationManager::perform_compress(&operation(), vec![], Some(archive.clone())).unwrap();
    let op = operation();
    let result = FileOperationManager::perform_archive_transfer_with(
        &op, vec![source.clone()], Some(archive.clone()), true, None,
        &mut || {
            assert!(std::fs::OpenOptions::new().write(true).open(&source).is_err());
            assert!(std::fs::remove_file(&source).is_err());
            let output = op.lock().unwrap().copied_files[0].1.clone();
            assert!(std::fs::OpenOptions::new().write(true).open(&output).is_err());
            assert!(std::fs::remove_file(&output).is_err());
            std::fs::write(&archive, b"foreign archive edit").map_err(|error| error.to_string())
        },
    );
    assert!(result.is_err());
    assert_eq!(std::fs::read(&source).unwrap(), b"source data");
    assert_eq!(std::fs::read(&archive).unwrap(), b"foreign archive edit");
    assert!(std::fs::OpenOptions::new().write(true).open(&source).is_ok());
    assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 2);
}

#[test]
#[cfg(target_os = "windows")]
fn archive_move_locks_copied_files_but_preserves_editable_skipped_conflicts() {
    use crate::systems::archive_workspace::ArchiveWorkspace;
    let root = tempfile::tempdir().unwrap();
    let seed = root.path().join("seed/source");
    let source = root.path().join("source");
    let archive = root.path().join("target.zip");
    std::fs::create_dir_all(&seed).unwrap();
    std::fs::create_dir(&source).unwrap();
    std::fs::write(seed.join("keep"), b"existing archive entry").unwrap();
    std::fs::write(source.join("keep"), b"skipped source").unwrap();
    std::fs::write(source.join("move"), b"moved source").unwrap();
    FileOperationManager::perform_compress(&operation(), vec![seed], Some(archive.clone())).unwrap();
    let resolutions = HashMap::from([(source.join("keep").to_string_lossy().into_owned(), ConflictAction::Skip)]);
    FileOperationManager::perform_archive_transfer_with(
        &operation(), vec![source.clone()], Some(archive.clone()), true, Some(resolutions),
        &mut || {
            assert!(std::fs::OpenOptions::new().write(true).open(source.join("move")).is_err());
            std::fs::write(source.join("keep"), b"edited skipped source").map_err(|error| error.to_string())
        },
    ).unwrap();
    assert_eq!(std::fs::read(source.join("keep")).unwrap(), b"edited skipped source");
    assert!(!source.join("move").exists());
    let workspace = ArchiveWorkspace::open(archive).unwrap();
    assert_eq!(std::fs::read(workspace.root().join("source/keep")).unwrap(), b"existing archive entry");
    assert_eq!(std::fs::read(workspace.root().join("source/move")).unwrap(), b"moved source");
}

#[test]
#[cfg(target_os = "windows")]
fn archive_directory_move_preserves_a_replacement_after_copy() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source");
    let original = root.path().join("relocated");
    let archive = root.path().join("target.zip");
    std::fs::create_dir(&source).unwrap();
    FileOperationManager::perform_compress(&operation(), vec![], Some(archive.clone())).unwrap();
    let result = FileOperationManager::perform_archive_transfer_with(
        &operation(), vec![source.clone()], Some(archive), true, None,
        &mut || {
            std::fs::rename(&source, &original).map_err(|error| error.to_string())?;
            std::fs::create_dir(&source).map_err(|error| error.to_string())
        },
    );
    assert!(source.is_dir(), "archive transfer deleted a replacement directory");
    assert!(original.is_dir());
    assert!(result.unwrap_err().contains("replacement was preserved"));
}

#[test]
#[cfg(target_os = "windows")]
fn archive_directory_move_keeps_the_initial_parent_after_junction_retargeting() {
    use crate::platform::windows_reparse_tests::{junction, Retarget};
    let root = tempfile::tempdir().unwrap();
    let chosen = root.path().join("chosen");
    let outside = root.path().join("outside");
    let link = root.path().join("link");
    let template = root.path().join("template");
    let archive = root.path().join("target.zip");
    std::fs::create_dir_all(chosen.join("source")).unwrap();
    std::fs::create_dir_all(outside.join("source")).unwrap();
    std::fs::write(outside.join("witness"), b"foreign witness").unwrap();
    junction(&link, &chosen);
    junction(&template, &outside);
    let retarget = Retarget::new(&link, &template);
    FileOperationManager::perform_compress(&operation(), vec![], Some(archive.clone())).unwrap();
    FileOperationManager::perform_archive_transfer_with(
        &operation(), vec![link.join("source")], Some(archive), true, None,
        &mut || { retarget.apply(); Ok(()) },
    ).unwrap();
    assert!(outside.join("source").is_dir());
    assert!(!chosen.join("source").exists());
    assert_eq!(std::fs::read(outside.join("witness")).unwrap(), b"foreign witness");
    drop(retarget);
    std::fs::remove_dir(&link).unwrap();
    std::fs::remove_dir(&template).unwrap();
}

#[test]
#[cfg(target_os = "windows")]
fn archive_directory_move_preserves_a_new_empty_child_absent_from_the_plan() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source");
    let archive = root.path().join("target.zip");
    std::fs::create_dir(&source).unwrap();
    FileOperationManager::perform_compress(&operation(), vec![], Some(archive.clone())).unwrap();
    FileOperationManager::perform_archive_transfer_with(
        &operation(), vec![source.clone()], Some(archive), true, None,
        &mut || std::fs::create_dir(source.join("new")).map_err(|error| error.to_string()),
    ).unwrap();
    assert!(source.join("new").is_dir());
}

#[test]
#[cfg(target_os = "windows")]
fn archive_directory_move_rejects_a_changed_destination_before_publication() {
    use crate::platform::windows_reparse_tests::{junction, Retarget};
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source");
    let outside = root.path().join("outside");
    let template = root.path().join("template");
    let archive = root.path().join("target.zip");
    std::fs::create_dir(&source).unwrap();
    std::fs::create_dir(&outside).unwrap();
    junction(&template, &outside);
    FileOperationManager::perform_compress(&operation(), vec![], Some(archive.clone())).unwrap();
    let original = std::fs::read(&archive).unwrap();
    let op = operation();
    let mut retarget = None;
    let result = FileOperationManager::perform_archive_transfer_with(
        &op, vec![source.clone()], Some(archive.clone()), true, None,
        &mut || {
            let destination = op.lock().unwrap().created_dirs.last().unwrap().clone();
            let changed = Retarget::new(&destination, &template);
            changed.apply();
            retarget = Some(changed);
            Ok(())
        },
    );
    // Closing the writer completes any deletion queued by workspace cleanup.
    drop(retarget.take());
    for (workspace, _) in &op.lock().unwrap().archive_path_mappings {
        if workspace.exists() {
            std::fs::remove_dir(workspace).unwrap();
        }
    }
    std::fs::remove_dir(&template).unwrap();
    assert!(result.unwrap_err().contains("destination directory"));
    assert!(source.is_dir());
    assert!(outside.is_dir());
    assert_eq!(std::fs::read(&archive).unwrap(), original);
}

#[test]
#[cfg(target_os = "windows")]
fn archive_directory_move_roundtrips_nested_empty_directories_in_all_writable_formats() {
    use crate::systems::archive_workspace::ArchiveWorkspace;
    for extension in ["zip", "7z", "tar", "tar.zst"] {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source");
        let output = root.path().join("output");
        let archive = root.path().join(format!("target.{extension}"));
        std::fs::create_dir_all(source.join("écriture/空")).unwrap();
        std::fs::create_dir(&output).unwrap();
        FileOperationManager::perform_compress(&operation(), vec![], Some(archive.clone())).unwrap();
        FileOperationManager::perform_copy(&operation(), vec![source.clone()], Some(archive.clone()), true, None).unwrap();
        assert!(!source.exists(), "{extension}");
        FileOperationManager::perform_copy(&operation(), vec![archive.join("source")], Some(output.clone()), true, None).unwrap();
        assert!(output.join("source/écriture/空").is_dir(), "{extension}");
        let workspace = ArchiveWorkspace::open(archive).unwrap();
        assert!(!workspace.root().join("source").exists(), "{extension}");
    }
}

#[test]
#[cfg(target_os = "windows")]
fn archive_directory_move_cancellation_keeps_the_original_archive() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source");
    let output = root.path().join("output");
    let archive = root.path().join("source.zip");
    std::fs::create_dir_all(source.join("nested/empty")).unwrap();
    std::fs::create_dir(&output).unwrap();
    FileOperationManager::perform_compress(&operation(), vec![source], Some(archive.clone())).unwrap();
    let original = std::fs::read(&archive).unwrap();
    let op = operation();
    let result = FileOperationManager::perform_archive_transfer_with(
        &op, vec![archive.join("source")], Some(output), true, None,
        &mut || { op.lock().unwrap().cancel_flag.store(true, Ordering::Relaxed); Ok(()) },
    );
    assert!(result.is_err());
    assert_eq!(std::fs::read(archive).unwrap(), original);
}

#[test]
#[cfg(target_os = "windows")]
fn archive_directory_move_keeps_an_entire_subtree_skipped_for_a_file_conflict() {
    use crate::systems::archive_workspace::ArchiveWorkspace;
    let root = tempfile::tempdir().unwrap();
    let seed = root.path().join("seed/source");
    let source = root.path().join("source");
    let archive = root.path().join("target.zip");
    std::fs::create_dir_all(&seed).unwrap();
    std::fs::write(seed.join("blocked"), b"existing archive file").unwrap();
    std::fs::create_dir_all(source.join("blocked/nested")).unwrap();
    std::fs::write(source.join("blocked/nested/keep"), b"skipped source data").unwrap();
    std::fs::create_dir(source.join("empty")).unwrap();
    FileOperationManager::perform_compress(&operation(), vec![seed], Some(archive.clone())).unwrap();
    FileOperationManager::perform_copy(&operation(), vec![source.clone()], Some(archive.clone()), true, None).unwrap();
    assert_eq!(std::fs::read(source.join("blocked/nested/keep")).unwrap(), b"skipped source data");
    assert!(!source.join("empty").exists());
    let workspace = ArchiveWorkspace::open(archive).unwrap();
    assert_eq!(std::fs::read(workspace.root().join("source/blocked")).unwrap(), b"existing archive file");
    assert!(workspace.root().join("source/empty").is_dir());
}
