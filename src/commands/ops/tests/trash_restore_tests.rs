    use super::*;

    #[test]
    #[ignore = "purges only one generated fixture and restores its generated sibling"]
    fn native_purge_only_removes_selected_trash_payload() {
        let root = tempfile::tempdir().unwrap();
        let selected = root.path().join("Supprimer école.txt");
        let sibling = root.path().join("Garder école.txt");
        let ordinary = root.path().join("Hors corbeille.txt");
        for path in [&selected, &sibling, &ordinary] {
            std::fs::write(path, b"purge fixture").unwrap();
        }
        crate::platform::platform()
            .shell_delete(&[selected.clone(), sibling.clone()], true)
            .unwrap();
        let entries = list_trash().unwrap();
        let selected_entry = entries
            .iter()
            .find(|entry| std::path::Path::new(&entry.original_path) == selected)
            .unwrap();
        let sibling_entry = entries
            .iter()
            .find(|entry| std::path::Path::new(&entry.original_path) == sibling)
            .unwrap();
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let purged = runtime.block_on(purge_recycle_bin(vec![
            selected_entry.path.clone(),
            ordinary.to_string_lossy().into_owned(),
        ]));
        let selected_still_present = PathBuf::from(&selected_entry.path).exists();
        let sibling_still_present = PathBuf::from(&sibling_entry.path).exists();
        // Restore generated survivors before assertions, without touching user entries.
        let remaining: Vec<_> = [selected_entry, sibling_entry]
            .into_iter()
            .filter(|entry| PathBuf::from(&entry.path).exists())
            .map(|entry| entry.path.clone())
            .collect();
        let restored = runtime.block_on(restore_items(remaining));
        purged.unwrap();
        restored.unwrap();
        assert!(!selected_still_present);
        assert!(sibling_still_present);
        assert!(!selected.exists());
        assert_eq!(std::fs::read(sibling).unwrap(), b"purge fixture");
        assert_eq!(std::fs::read(ordinary).unwrap(), b"purge fixture");
    }

    #[test]
    #[ignore = "uses only a generated fixture to check a native Recycle Bin restore conflict"]
    fn native_restore_conflict_preserves_existing_file_and_trash_item() {
        let root = tempfile::tempdir().unwrap();
        let file = root.path().join("Conflit école.txt");
        std::fs::write(&file, b"original fixture").unwrap();
        crate::platform::platform()
            .shell_delete(std::slice::from_ref(&file), true)
            .unwrap();
        let entry = list_trash()
            .unwrap()
            .into_iter()
            .find(|entry| std::path::Path::new(&entry.original_path) == file)
            .unwrap();
        std::fs::write(&file, b"existing fixture").unwrap();
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let result = runtime.block_on(restore_items(vec![entry.path.clone()]));
        let existing = std::fs::read(&file).unwrap();
        let still_in_trash = list_trash()
            .unwrap()
            .iter()
            .any(|item| item.path == entry.path);

        // Remove only the generated blocking file, then restore the fixture
        // before assertions so an assertion failure does not leave it in trash.
        let restored = if still_in_trash {
            std::fs::remove_file(&file).unwrap();
            runtime.block_on(restore_items(vec![entry.path]))
        } else {
            Ok(Vec::new())
        };
        assert!(result.is_err(), "a restore conflict must be reported");
        assert_eq!(existing, b"existing fixture");
        assert!(still_in_trash, "the original fixture must remain restorable");
        assert_eq!(restored.unwrap(), vec![file.to_string_lossy().to_string()]);
        assert_eq!(std::fs::read(&file).unwrap(), b"original fixture");
    }

    #[test]
    #[ignore = "sends only generated fixtures to the native Windows Recycle Bin and restores them"]
    fn native_restore_preserves_original_names() {
        let root = tempfile::tempdir().unwrap();
        let files = [
            root.path().join("Document école – version 2.txt"),
            root.path().join("Raccourci école.lnk"),
            root.path().join("Lien école.url"),
        ];
        for file in &files {
            std::fs::write(file, b"restoration fixture").unwrap();
        }
        let folder = root.path().join("Dossier école");
        std::fs::create_dir(&folder).unwrap();
        std::fs::write(folder.join("contenu.txt"), b"nested fixture").unwrap();
        let sources: Vec<_> = files.iter().cloned().chain([folder.clone()]).collect();
        crate::platform::platform()
            .shell_delete(&sources, true)
            .unwrap();
        let entries: Vec<_> = list_trash()
            .unwrap()
            .into_iter()
            .filter(|entry| {
                // Identify only this test's fixtures, even if a metadata path has a
                // trailing terminator. Validate the unmodified names after restore.
                PathBuf::from(entry.original_path.trim_end_matches('\0')).parent()
                    == Some(root.path())
            })
            .collect();
        let ids = entries.iter().map(|entry| entry.path.clone()).collect();
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let restored = runtime.block_on(restore_items(ids)).unwrap();

        for file in &files {
            assert_eq!(std::fs::read(file).unwrap(), b"restoration fixture");
        }
        assert_eq!(
            std::fs::read(folder.join("contenu.txt")).unwrap(),
            b"nested fixture"
        );
        assert_eq!(entries.len(), sources.len());
        for entry in &entries {
            let expected = sources
                .iter()
                .find(|source| {
                    source.as_path() == std::path::Path::new(entry.original_path.trim_end_matches('\0'))
                })
                .unwrap();
            assert_eq!(entry.name, expected.file_name().unwrap().to_string_lossy());
            assert_eq!(PathBuf::from(&entry.original_path), *expected);
        }
        let mut restored: Vec<_> = restored.into_iter().map(PathBuf::from).collect();
        let mut expected = sources;
        restored.sort();
        expected.sort();
        assert_eq!(restored, expected);
        assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 4);
    }
