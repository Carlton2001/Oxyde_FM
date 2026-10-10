use super::*;
fn operation(source: &Path, destination: &Path, is_move: bool) -> Arc<Mutex<FileOperation>> {
    let mut operation = FileOperation::new(if is_move { FileOpType::Move } else { FileOpType::Copy },
        vec![source.to_path_buf()], Some(destination.to_path_buf()), None);
    operation.turbo = true;
    operation.turbo_flag.store(true, Ordering::Relaxed);
    Arc::new(Mutex::new(operation))
}
#[test]
fn same_folder_copy_creates_numbered_duplicates_and_journals_the_actual_names() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("École.txt");
    std::fs::write(&source, "original").unwrap();
    for name in ["École - Copie.txt", "École - Copie (2).txt"] {
        let op = operation(&source, root.path(), false);
        FileOperationManager::perform_copy(&op, vec![source.clone()], Some(root.path().to_path_buf()), false, None).unwrap();
        assert_eq!(std::fs::read(root.path().join(name)).unwrap(), b"original");
        let op = op.lock().unwrap();
        assert_eq!(op.copied_files.len(), 1);
        assert_eq!(op.copied_files[0].1.file_name().unwrap(), name);
        assert_eq!(op.copied_file_identities.len(), 1);
        assert!(op.overwritten_files.is_empty());
        std::fs::write(root.path().join(name), "witness").unwrap();
    }
    assert_eq!(std::fs::read(source).unwrap(), b"original");
    assert_eq!(std::fs::read(root.path().join("École - Copie.txt")).unwrap(), b"witness");
}
#[test]
fn same_folder_move_is_ignored_before_conflicts_and_executor_effects() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("original.txt");
    std::fs::write(&source, "original").unwrap();
    assert!(futures::executor::block_on(crate::commands::ops::check_conflicts(
        vec![source.to_string_lossy().into_owned()], root.path().to_string_lossy().into_owned())).unwrap().is_empty());
    let op = operation(&source, root.path(), true);
    FileOperationManager::perform_copy(&op, vec![source.clone()], Some(root.path().to_path_buf()), true, None).unwrap();
    let op = op.lock().unwrap();
    assert_eq!(op.processed_files, 0);
    assert!(op.moved_files.is_empty() && op.created_files.is_empty() && op.overwritten_files.is_empty());
    assert_eq!(std::fs::read(source).unwrap(), b"original");
}
#[test]
fn same_folder_directory_copy_preserves_the_original_tree_and_literal_folder_name() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("Dossier.ext");
    std::fs::create_dir_all(source.join("nested")).unwrap();
    std::fs::write(source.join("nested/file.txt"), "original").unwrap();
    let op = operation(&source, root.path(), false);
    FileOperationManager::perform_copy(&op, vec![source.clone()], Some(root.path().to_path_buf()), false, None).unwrap();
    assert_eq!(std::fs::read(root.path().join("Dossier.ext - Copie/nested/file.txt")).unwrap(), b"original");
    assert_eq!(std::fs::read(source.join("nested/file.txt")).unwrap(), b"original");
    assert!(op.lock().unwrap().created_dirs.iter().any(|path| path.file_name().unwrap() == "Dossier.ext - Copie"));
}
