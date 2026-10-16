use super::*;

#[test]
fn conflict_preparation_rechecks_changed_targets_and_keeps_file_directory_totals() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source/selected");
    let target_parent = root.path().join("target");
    let target = target_parent.join("selected");
    for directory in [&source, &target] {
        std::fs::create_dir_all(directory.join("d")).unwrap();
    }
    std::fs::write(source.join("x"), [42; 3]).unwrap();
    std::fs::write(source.join("d/y"), [42; 7]).unwrap();
    std::fs::write(target.join("x"), [42; 5]).unwrap();
    std::fs::write(target.join("d/y"), [42; 11]).unwrap();
    let check = || {
        check_conflicts_real(
            vec![source.to_string_lossy().into_owned()],
            target_parent.to_string_lossy().into_owned(),
        )
        .unwrap()
    };
    let first = check();
    assert_eq!(first.len(), 4);
    assert_eq!(
        (
            first[0].source.size,
            first[0].source.folders_count,
            first[0].source.files_count
        ),
        (10, Some(1), Some(2))
    );
    assert_eq!(first[0].target.size, 16);
    std::fs::write(target.join("d/y"), [42; 22]).unwrap();
    assert_eq!(check()[0].target.size, 27);
    assert!(target
        .canonicalize()
        .unwrap()
        .starts_with(root.path().canonicalize().unwrap()));
    std::fs::remove_dir_all(&target).unwrap();
    std::fs::write(&target, [42; 9]).unwrap();
    let replacement = check();
    assert_eq!(replacement.len(), 1);
    assert_eq!(replacement[0].target.size, 9);
    assert_eq!(replacement[0].target.files_count, Some(1));
}
