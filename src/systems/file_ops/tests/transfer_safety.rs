use super::*;

#[test]
fn overlapping_source_selections_are_refused_before_destination_changes() {
    let root = tempfile::tempdir().unwrap();
    let folder = root.path().join("source");
    let file = folder.join("file.txt");
    let target = root.path().join("destination");
    std::fs::create_dir(&folder).unwrap();
    std::fs::create_dir(&target).unwrap();
    std::fs::write(&file, b"keep").unwrap();
    for sources in [vec![folder.clone(), file.clone()], vec![file.clone(), folder.clone()], vec![file.clone(), file.clone()]] {
        let result = FileOperationManager::perform_copy(&operation(), sources, Some(target.clone()), false, None);
        assert!(result.unwrap_err().contains("Overlapping source paths"));
        assert_eq!(std::fs::read_dir(&target).unwrap().count(), 0);
        assert_eq!(std::fs::read(&file).unwrap(), b"keep");
    }
}

#[test]
fn move_merge_keeps_skipped_source_files() {
    let temp = tempfile::tempdir().unwrap();
    let source_root = temp.path().join("source");
    let target_root = temp.path().join("target");
    let source = source_root.join("folder");
    let destination = target_root.join("folder");
    std::fs::create_dir_all(&source).unwrap();
    std::fs::create_dir_all(&destination).unwrap();
    std::fs::write(source.join("conflict.txt"), b"source version").unwrap();
    std::fs::write(destination.join("conflict.txt"), b"existing version").unwrap();
    std::fs::write(source.join("new.txt"), b"new file").unwrap();

    FileOperationManager::perform_copy(
        &operation(),
        vec![source.clone()],
        Some(target_root),
        true,
        None,
    )
    .unwrap();

    assert_eq!(
        std::fs::read(source.join("conflict.txt")).unwrap(),
        b"source version"
    );
    assert!(!source.join("new.txt").exists());
    assert_eq!(
        std::fs::read(destination.join("new.txt")).unwrap(),
        b"new file"
    );
}

#[test]
fn move_rejects_destination_inside_source() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("source");
    let target = source.join("nested");
    std::fs::create_dir_all(&target).unwrap();
    std::fs::write(source.join("file.txt"), b"keep").unwrap();

    let result = FileOperationManager::perform_copy(
        &operation(),
        vec![source.clone()],
        Some(target),
        true,
        None,
    );

    assert!(result.is_err());
    assert_eq!(std::fs::read(source.join("file.txt")).unwrap(), b"keep");
}

#[test]
fn self_transfers_preserve_the_original_even_with_parent_components_and_replace_resolution() {
    let temp = tempfile::tempdir().unwrap();
    let folder = temp.path().join("folder");
    let detour = folder.join("detour");
    std::fs::create_dir_all(&detour).unwrap();
    let destination = folder.join("file.txt");
    std::fs::write(&destination, b"preserve").unwrap();
    let source_alias = detour.join("..").join("file.txt");
    let resolutions = HashMap::from([(
        source_alias.to_string_lossy().into_owned(),
        ConflictAction::Replace,
    )]);

    let move_op = operation();
    FileOperationManager::perform_copy(
        &move_op,
        vec![source_alias.clone()],
        Some(folder.clone()),
        true,
        Some(resolutions.clone()),
    )
    .unwrap();
    assert_eq!(move_op.lock().unwrap().processed_files, 0);
    assert!(move_op.lock().unwrap().moved_files.is_empty());
    assert_eq!(std::fs::read(&destination).unwrap(), b"preserve");

    let copy_op = operation();
    FileOperationManager::perform_copy(
        &copy_op,
        vec![source_alias],
        Some(folder.clone()),
        false,
        Some(resolutions),
    )
    .unwrap();
    let duplicate = folder.join("file - Copie.txt");
    assert_eq!(std::fs::read(&destination).unwrap(), b"preserve");
    assert_eq!(std::fs::read(&duplicate).unwrap(), b"preserve");
    assert!(!crate::systems::transfer_naming::same_entry(&destination, &duplicate));
    let copy_op = copy_op.lock().unwrap();
    assert_eq!(copy_op.copied_files[0].1, std::fs::canonicalize(duplicate).unwrap());
    assert!(copy_op.overwritten_files.is_empty());
}

#[test]
fn move_preserves_source_when_cancelled_before_copy() {
    let temp = tempfile::tempdir().unwrap();
    let source_dir = temp.path().join("source");
    let target_dir = temp.path().join("target");
    std::fs::create_dir(&source_dir).unwrap();
    std::fs::create_dir(&target_dir).unwrap();
    let source = source_dir.join("file.txt");
    std::fs::write(&source, b"preserve me").unwrap();
    let op = operation();
    op.lock()
        .unwrap()
        .cancel_flag
        .store(true, Ordering::Relaxed);

    assert!(FileOperationManager::perform_copy(
        &op,
        vec![source.clone()],
        Some(target_dir.clone()),
        true,
        None,
    )
    .is_ok());
    assert_eq!(std::fs::read(source).unwrap(), b"preserve me");
    assert!(!target_dir.join("file.txt").exists());
}

#[test]
fn move_preserves_empty_directories() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("source");
    let target = temp.path().join("target");
    std::fs::create_dir_all(source.join("empty/nested")).unwrap();
    std::fs::create_dir(&target).unwrap();

    FileOperationManager::perform_copy(
        &operation(),
        vec![source.clone()],
        Some(target.clone()),
        true,
        None,
    )
    .unwrap();

    assert!(target.join("source/empty/nested").is_dir());
    assert!(!source.exists());
}

#[cfg(unix)]
#[test]
fn copy_rejects_symbolic_links_instead_of_following_them() {
    use std::os::unix::fs::symlink;

    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("source");
    let target = temp.path().join("target");
    let outside = temp.path().join("outside.txt");
    std::fs::create_dir(&source).unwrap();
    std::fs::create_dir(&target).unwrap();
    std::fs::write(&outside, b"outside source tree").unwrap();
    symlink(&outside, source.join("link.txt")).unwrap();

    assert!(FileOperationManager::perform_copy(
        &operation(),
        vec![source.clone()],
        Some(target.clone()),
        false,
        None,
    )
    .is_err());
    assert!(target.join("source/link.txt").exists() == false);
    assert_eq!(std::fs::read(outside).unwrap(), b"outside source tree");
}
