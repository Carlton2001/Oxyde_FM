use super::*;

#[test]
fn real_directories_are_skipped_and_the_actual_nested_archive_is_resolved() {
    let root = tempfile::tempdir().unwrap();
    for directory in ["ordinary.zip", "ordinary.7z", "ordinary.tar.gz"] {
        let folder = root.path().join(directory);
        std::fs::create_dir_all(folder.join("child")).unwrap();
        std::fs::write(folder.join("plain.txt"), b"real").unwrap();
        for path in [
            &folder,
            &folder.join("plain.txt"),
            &folder.join("child/new"),
        ] {
            assert!(resolve_virtual_path(path).is_none(), "{path:?}");
            assert!(!is_navigable_archive(path));
            assert!(!is_writable_archive(path));
        }
        for name in ["inner.zip", "inner.7z", "inner.tar.gz"] {
            let archive = folder.join(name);
            std::fs::write(&archive, b"archive placeholder").unwrap();
            assert_eq!(
                resolve_virtual_path(&archive),
                Some((archive.clone(), String::new()))
            );
            assert_eq!(
                resolve_virtual_path(&archive.join("été.zip/nested.7z/日本語.txt")),
                Some((archive.clone(), "été.zip/nested.7z/日本語.txt".into()))
            );
            assert!(is_navigable_archive(&archive));
            assert!(!is_navigable_archive(&archive.join("nested.zip")));
        }
    }
}

#[test]
fn missing_archive_candidates_do_not_become_virtual_business_paths() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("missing.zip/file.txt");
    assert!(split_virtual_path(&path.to_string_lossy()).is_some());
    assert!(resolve_virtual_path(&path).is_none());
}
