use super::*;

#[test]
fn same_volume_move_preserves_file_identity_without_copying() {
    let temp = tempfile::tempdir().unwrap();
    let source_dir = temp.path().join("source");
    let target_dir = temp.path().join("target");
    std::fs::create_dir(&source_dir).unwrap();
    std::fs::create_dir(&target_dir).unwrap();
    let source = source_dir.join("file.bin");
    let contents = vec![0x5a; 2 * 1024 * 1024];
    std::fs::write(&source, &contents).unwrap();
    let alias = temp.path().join("alias.bin");
    std::fs::hard_link(&source, &alias).unwrap();
    let op = operation();

    FileOperationManager::perform_copy(
        &op,
        vec![source.clone()],
        Some(target_dir.clone()),
        true,
        None,
    )
    .unwrap();

    assert!(!source.exists());
    assert_eq!(
        std::fs::read(target_dir.join("file.bin")).unwrap(),
        contents
    );
    std::fs::write(target_dir.join("file.bin"), b"changed").unwrap();
    assert_eq!(std::fs::read(alias).unwrap(), b"changed");
    let locked = op.lock().unwrap();
    assert_eq!(locked.moved_files.len(), 1);
    assert_eq!(locked.moved_file_identities.len(), 1);
    assert!(locked.moved_file_identities[0].1.snapshot().unwrap()
        .matches(&std::fs::File::open(target_dir.join("file.bin")).unwrap())
        .unwrap());
    assert!(locked.copied_files.is_empty());
    assert!(locked.copied_file_hashes.is_empty());
}

#[cfg(target_os = "windows")]
#[test]
fn real_directory_delete_preserves_junction_targets_and_reports_completion() {
    let root = tempfile::tempdir().unwrap();
    let selected = root.path().join("selected");
    let outside = root.path().join("outside");
    std::fs::create_dir_all(selected.join("nested")).unwrap();
    std::fs::create_dir(&outside).unwrap();
    std::fs::write(selected.join("nested/content"), b"selected").unwrap();
    std::fs::write(outside.join("witness"), b"outside witness").unwrap();
    crate::platform::windows_reparse_tests::junction(&selected.join("link"), &outside);
    let op = Arc::new(Mutex::new(FileOperation::new(FileOpType::Delete, vec![selected.clone()], None, None)));
    FileOperationManager::perform_delete(&op, vec![selected.clone()]).unwrap();
    assert!(!selected.exists());
    assert_eq!(std::fs::read(outside.join("witness")).unwrap(), b"outside witness");
    let locked = op.lock().unwrap();
    assert_eq!(locked.total_files, 1);
    assert_eq!(locked.processed_files, 1);
}

#[cfg(target_os = "windows")]
#[test]
fn directory_copy_refuses_a_nested_destination_junction_before_creation() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source");
    let target = root.path().join("target");
    let outside = root.path().join("outside");
    std::fs::create_dir_all(source.join("nested/empty")).unwrap();
    std::fs::create_dir(&target).unwrap();
    std::fs::create_dir(&outside).unwrap();
    std::fs::write(outside.join("witness"), b"outside witness").unwrap();
    crate::platform::windows_reparse_tests::junction(&target.join("source"), &outside);
    let op = operation();
    let result = FileOperationManager::perform_copy(&op, vec![source.clone()], Some(target.clone()), false, None);
    let created_outside = outside.join("nested").exists();
    std::fs::remove_dir(target.join("source")).unwrap();
    assert!(result.is_err(), "directory creation accepted a nested destination junction");
    assert!(!created_outside, "directory creation escaped into a junction target");
    assert_eq!(std::fs::read(outside.join("witness")).unwrap(), b"outside witness");
    assert!(source.join("nested/empty").is_dir());
    assert!(op.lock().unwrap().created_dirs.is_empty());
}

#[test]
fn copy_skips_existing_destination_without_resolution() {
    let temp = tempfile::tempdir().unwrap();
    let source_dir = temp.path().join("source");
    let target_dir = temp.path().join("target");
    std::fs::create_dir(&source_dir).unwrap();
    std::fs::create_dir(&target_dir).unwrap();
    let source = source_dir.join("file.txt");
    let destination = target_dir.join("file.txt");
    std::fs::write(&source, b"source").unwrap();
    std::fs::write(&destination, b"existing").unwrap();

    FileOperationManager::perform_copy(
        &operation(),
        vec![source.clone()],
        Some(target_dir),
        false,
        None,
    )
    .unwrap();

    assert_eq!(std::fs::read(&source).unwrap(), b"source");
    assert_eq!(std::fs::read(destination).unwrap(), b"existing");
}

#[test]
fn copy_processes_every_selected_source() {
    let temp = tempfile::tempdir().unwrap();
    let source_dir = temp.path().join("source");
    let target_dir = temp.path().join("target");
    std::fs::create_dir(&source_dir).unwrap();
    std::fs::create_dir(&target_dir).unwrap();
    let first = source_dir.join("first.txt");
    let second = source_dir.join("second.txt");
    std::fs::write(&first, b"first").unwrap();
    std::fs::write(&second, b"second").unwrap();

    FileOperationManager::perform_copy(
        &operation(),
        vec![first, second],
        Some(target_dir.clone()),
        false,
        None,
    )
    .unwrap();

    assert_eq!(
        std::fs::read(target_dir.join("first.txt")).unwrap(),
        b"first"
    );
    assert_eq!(
        std::fs::read(target_dir.join("second.txt")).unwrap(),
        b"second"
    );
}

#[test]
fn explicit_replace_preserves_overwrite_policy_without_a_backup() {
    let temp = tempfile::tempdir().unwrap();
    let source_dir = temp.path().join("source");
    let target_dir = temp.path().join("target");
    std::fs::create_dir(&source_dir).unwrap();
    std::fs::create_dir(&target_dir).unwrap();
    let source = source_dir.join("file.txt");
    let destination = target_dir.join("file.txt");
    std::fs::write(&source, b"new contents").unwrap();
    std::fs::write(&destination, b"old contents that are longer").unwrap();
    let resolutions = HashMap::from([(
        source.to_string_lossy().into_owned(),
        ConflictAction::Replace,
    )]);
    let op = operation();

    FileOperationManager::perform_copy(
        &op,
        vec![source.clone()],
        Some(target_dir.clone()),
        false,
        Some(resolutions),
    )
    .unwrap();

    assert_eq!(std::fs::read(&destination).unwrap(), b"new contents");
    let operation = op.lock().unwrap();
    assert_eq!(
        operation.overwritten_files,
        vec![std::fs::canonicalize(&destination).unwrap()]
    );
    assert!(operation.created_files.is_empty());
    assert_eq!(std::fs::read_dir(&target_dir).unwrap().count(), 1);
    assert_eq!(operation.copied_file_hashes.len(), 1);
    assert_eq!(
        operation.copied_file_hashes[0].1,
        blake3::hash(b"new contents").to_hex().to_string()
    );
}

#[cfg(target_os = "windows")]
#[test]
fn native_initial_copy_preserves_streams_attributes_and_modified_time() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source.txt");
    let target_dir = root.path().join("target");
    std::fs::create_dir(&target_dir).unwrap();
    std::fs::write(&source, b"main content").unwrap();
    std::fs::write(
        source.with_file_name("source.txt:oxyde-test"),
        b"alternate content",
    )
    .unwrap();
    let modified = std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_700_000_000);
    std::fs::File::options()
        .write(true)
        .open(&source)
        .unwrap()
        .set_times(std::fs::FileTimes::new().set_modified(modified))
        .unwrap();
    platform().set_hidden_attribute(&source, true).unwrap();
    FileOperationManager::perform_copy(
        &operation(),
        vec![source],
        Some(target_dir.clone()),
        false,
        None,
    )
    .unwrap();
    let destination = target_dir.join("source.txt");
    assert_eq!(std::fs::read(&destination).unwrap(), b"main content");
    assert_eq!(
        std::fs::read(target_dir.join("source.txt:oxyde-test")).unwrap(),
        b"alternate content"
    );
    let metadata = std::fs::metadata(&destination).unwrap();
    assert_eq!(metadata.modified().unwrap(), modified);
    assert!(platform().file_attributes(&metadata, "source.txt").0);
}
