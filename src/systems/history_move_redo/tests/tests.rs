    use super::*;
    use crate::platform::windows_reparse_tests::{Retarget, junction};

    fn details(source: &Path, destination: &Path) -> TransactionDetails {
        TransactionDetails {
            deleted_items: None,
            paths: vec![source.to_string_lossy().into_owned()],
            target_dir: None,
            old_path: None,
            new_path: None,
            created_files: Some(vec![]),
            created_dirs: Some(vec![]),
            overwritten_files: Some(vec![]),
            moved_files: Some(vec![(
                source.to_string_lossy().into_owned(),
                destination.to_string_lossy().into_owned(),
            )]),
            moved_dirs: Some(vec![]),
            copied_files: Some(vec![]),
            copied_file_hashes: Some(vec![(
                destination.to_string_lossy().into_owned(),
                blake3::hash(b"moving file").to_hex().to_string(),
            )]),
            copied_file_identities: None,
            copied_directory_identities: None,
            created_links: None,
            moved_links: None,
            renamed_item_identity: None,
            moved_directory_identities: None,
            new_folder_identity: None,
            moved_file_identities: Some(vec![(
                destination.to_string_lossy().into_owned(),
                crate::models::transaction::HistoryMoveIdentity::from_file(
                    &fs::File::open(source).unwrap(),
                )
                .unwrap(),
            )]),
            trash_metadata_backups: None,
        }
    }
    // Fixtures begin after a successful logical Undo: destinations do not yet
    // exist, while every source proof must describe the current legitimate object.
    fn directory_proofs(tx: &mut TransactionDetails) {
        tx.copied_directory_identities = Some(
            tx.created_dirs
                .as_deref()
                .unwrap_or_default()
                .iter()
                .map(|path| {
                    (
                        path.clone(),
                        crate::models::transaction::HistoryDirectoryIdentity::vacant(),
                    )
                })
                .collect(),
        );
        tx.moved_directory_identities = Some(
            tx.moved_dirs
                .as_deref()
                .unwrap_or_default()
                .iter()
                .map(|(path, _)| {
                    (
                        path.clone(),
                        crate::models::transaction::HistoryDirectoryIdentity::from_file(
                            &platform()
                                .hold_resolved_operation_directory(&fs::canonicalize(path).unwrap())
                                .unwrap(),
                        )
                        .unwrap(),
                    )
                })
                .collect(),
        );
    }
    struct Hook<F>(F);
    impl<F: FnMut(Phase, &Path) -> io::Result<()>> Checkpoint for Hook<F> {
        fn before(&mut self, phase: Phase, path: &Path) -> io::Result<()> {
            (self.0)(phase, path)
        }
    }

    #[test]
    fn redo_move_rechecks_identity_after_plan_before_publication() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source.txt");
        let destination = root.path().join("destination.txt");
        let saved = root.path().join("selected-original.txt");
        fs::write(&source, b"moving file").unwrap();
        let tx = details(&source, &destination);
        let error = redo_with(
            &tx,
            &mut Hook(|phase, _: &Path| {
                if matches!(phase, Phase::Move) {
                    fs::rename(&source, &saved)?;
                    fs::write(&source, b"moving file")?;
                }
                Ok(())
            }),
        )
        .unwrap_err();
        assert!(
            matches!(CommandError::from(error), CommandError::HistoryMove(failure) if failure.issue == Issue::Changed)
        );
        assert_eq!(fs::read(&source).unwrap(), b"moving file");
        assert_eq!(fs::read(&saved).unwrap(), b"moving file");
        assert!(!destination.exists());
    }

    #[test]
    fn redo_move_cleanup_preserves_a_substituted_empty_directory() {
        let root = tempfile::tempdir().unwrap();
        let created = root.path().join("created");
        let displaced = root.path().join("displaced");
        let directories = Directories::prepare(std::slice::from_ref(&created), &[]).unwrap();
        let mut journal = Journal {
            directory_identities: HashMap::new(),
            directories,
            effects: vec![],
            finished: false,
        };
        journal.create(&created, &mut Native).unwrap();
        fs::rename(&created, &displaced).unwrap();
        fs::create_dir(&created).unwrap();
        assert!(!journal.rollback().is_empty());
        assert!(
            created.is_dir(),
            "Redo move cleanup removed an independent replacement directory"
        );
        assert!(displaced.is_dir());
    }

    #[test]
    fn redo_move_uses_bound_destination_after_parent_junction_retarget() {
        let root = tempfile::tempdir().unwrap();
        let first = root.path().join("first");
        let outside = root.path().join("outside");
        fs::create_dir(&first).unwrap();
        fs::create_dir(&outside).unwrap();
        let link = root.path().join("link");
        let template = root.path().join("template");
        junction(&link, &first);
        junction(&template, &outside);
        let source = root.path().join("source.txt");
        fs::write(&source, b"moving file").unwrap();
        let destination = link.join("new/child/file.txt");
        let mut tx = details(&source, &destination);
        tx.created_dirs = Some(vec![
            link.join("new").to_string_lossy().into_owned(),
            link.join("new/child").to_string_lossy().into_owned(),
        ]);
        directory_proofs(&mut tx);
        let retarget = Retarget::new(&link, &template);
        let mut once = false;
        redo_with(
            &tx,
            &mut Hook(|phase, _: &Path| {
                if matches!(phase, Phase::Create) && !once {
                    once = true;
                    retarget.apply();
                }
                Ok(())
            }),
        )
        .unwrap();
        assert_eq!(
            fs::read(first.join("new/child/file.txt")).unwrap(),
            b"moving file"
        );
        assert_eq!(fs::read_dir(&outside).unwrap().count(), 0);
        assert!(!source.exists());
        drop(retarget);
        fs::remove_dir(link).unwrap();
        fs::remove_dir(template).unwrap();
    }

    #[test]
    fn redo_move_refuses_a_directory_arriving_before_exclusive_creation() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source.txt");
        fs::write(&source, b"moving file").unwrap();
        let parent = root.path().join("destination");
        let tx = details(&source, &parent.join("file.txt"));
        let error = redo_with(
            &tx,
            &mut Hook(|phase, path: &Path| {
                if matches!(phase, Phase::Create) {
                    fs::create_dir(path)?;
                    fs::write(path.join("witness"), b"independent")?;
                }
                Ok(())
            }),
        )
        .unwrap_err();
        assert!(
            matches!(CommandError::from(error), CommandError::HistoryMove(failure) if failure.issue == Issue::Conflict)
        );
        assert_eq!(fs::read(&source).unwrap(), b"moving file");
        assert_eq!(fs::read(parent.join("witness")).unwrap(), b"independent");
        assert!(!parent.join("file.txt").exists());
    }

    #[test]
    fn redo_move_source_cleanup_refuses_substitution_and_rolls_files_back_to_bound_source() {
        let root = tempfile::tempdir().unwrap();
        let parent = root.path().join("source");
        fs::create_dir(&parent).unwrap();
        let original = root.path().join("original");
        let source = parent.join("file.txt");
        fs::write(&source, b"moving file").unwrap();
        let destination = root.path().join("destination/file.txt");
        let mut tx = details(&source, &destination);
        tx.created_dirs = Some(vec![
            destination.parent().unwrap().to_string_lossy().into_owned(),
        ]);
        directory_proofs(&mut tx);
        tx.moved_dirs = Some(vec![(
            parent.to_string_lossy().into_owned(),
            destination.parent().unwrap().to_string_lossy().into_owned(),
        )]);
        directory_proofs(&mut tx);
        redo_with(
            &tx,
            &mut Hook(|phase, path: &Path| {
                if matches!(phase, Phase::Remove) {
                    fs::rename(path, &original)?;
                    fs::create_dir(path)?;
                }
                Ok(())
            }),
        )
        .unwrap_err();
        assert!(parent.is_dir());
        assert_eq!(fs::read_dir(&parent).unwrap().count(), 0);
        assert_eq!(fs::read(original.join("file.txt")).unwrap(), b"moving file");
        assert!(!destination.parent().unwrap().exists());
    }

    #[test]
    fn redo_move_keeps_a_child_arriving_before_source_cleanup() {
        let root = tempfile::tempdir().unwrap();
        let parent = root.path().join("source");
        fs::create_dir(&parent).unwrap();
        let source = parent.join("file.txt");
        fs::write(&source, b"moving file").unwrap();
        let destination = root.path().join("destination/file.txt");
        let mut tx = details(&source, &destination);
        tx.created_dirs = Some(vec![
            destination.parent().unwrap().to_string_lossy().into_owned(),
        ]);
        directory_proofs(&mut tx);
        tx.moved_dirs = Some(vec![(
            parent.to_string_lossy().into_owned(),
            destination.parent().unwrap().to_string_lossy().into_owned(),
        )]);
        directory_proofs(&mut tx);
        redo_with(
            &tx,
            &mut Hook(|phase, path: &Path| {
                if matches!(phase, Phase::Remove) {
                    fs::write(path.join("late.txt"), b"independent")?;
                }
                Ok(())
            }),
        )
        .unwrap();
        assert_eq!(fs::read(parent.join("late.txt")).unwrap(), b"independent");
        assert_eq!(fs::read(&destination).unwrap(), b"moving file");
        assert!(!source.exists());
        super::super::history_file_undo::undo(&tx).unwrap();
        assert_eq!(fs::read(&source).unwrap(), b"moving file");
        assert_eq!(fs::read(parent.join("late.txt")).unwrap(), b"independent");
        redo(&tx).unwrap();
        assert_eq!(fs::read(parent.join("late.txt")).unwrap(), b"independent");
    }

    #[test]
    fn redo_move_rolls_back_removed_directories_before_files_and_preserves_hidden_attribute() {
        let root = tempfile::tempdir().unwrap();
        let first = root.path().join("first");
        let second = root.path().join("second");
        fs::create_dir(&second).unwrap();
        let root_parents = super::super::path_parents::HeldParents::new(&first).unwrap();
        let first_handle = platform()
            .create_journal_directory(root_parents.directory(), &first, false)
            .unwrap();
        let source = first.join("file.txt");
        fs::write(&source, b"moving file").unwrap();
        let destination = root.path().join("destination/file.txt");
        let mut tx = details(&source, &destination);
        let permissions = fs::metadata(&first).unwrap().permissions();
        platform()
            .restore_journal_directory(&first_handle, &first, permissions, true)
            .unwrap();
        drop(first_handle);
        drop(root_parents);
        tx.created_dirs = Some(vec![
            destination.parent().unwrap().to_string_lossy().into_owned(),
        ]);
        directory_proofs(&mut tx);
        tx.moved_dirs = Some(vec![
            (
                first.to_string_lossy().into_owned(),
                destination.parent().unwrap().to_string_lossy().into_owned(),
            ),
            (
                second.to_string_lossy().into_owned(),
                destination.parent().unwrap().to_string_lossy().into_owned(),
            ),
        ]);
        directory_proofs(&mut tx);
        let error = redo_with(
            &tx,
            &mut Hook(|phase, path: &Path| {
                if matches!(phase, Phase::Remove) && path == second {
                    return Err(io::Error::other("injected directory failure"));
                }
                Ok(())
            }),
        )
        .unwrap_err();
        assert!(error.to_string().contains("injected directory failure"));
        assert_eq!(fs::read(&source).unwrap(), b"moving file");
        assert!(second.is_dir());
        assert!(!destination.parent().unwrap().exists());
        let restored = tx
            .moved_directory_identities
            .as_ref()
            .unwrap()
            .iter()
            .find(|(path, _)| Path::new(path) == first)
            .unwrap()
            .1
            .snapshot()
            .unwrap();
        assert!(
            restored
                .matches(
                    &platform()
                        .hold_resolved_operation_directory(&fs::canonicalize(&first).unwrap())
                        .unwrap()
                )
                .unwrap()
        );
        assert!(!fs::metadata(&first).unwrap().permissions().readonly());
        assert!(
            platform()
                .file_attributes(&fs::metadata(&first).unwrap(), "")
                .0
        );
        let mut permissions = fs::metadata(&first).unwrap().permissions();
        permissions.set_readonly(false);
        fs::set_permissions(&first, permissions).unwrap();
        redo_with(&tx, &mut Native).unwrap();
        assert_eq!(fs::read(&destination).unwrap(), b"moving file");
        assert!(!first.exists());
        assert!(!second.exists());
    }

    #[test]
    fn redo_move_recreates_nested_directories_below_a_canonical_root() {
        let root = tempfile::tempdir().unwrap();
        let canonical = fs::canonicalize(root.path()).unwrap();
        let path = canonical.join("one/two");
        let directories = Directories::prepare(std::slice::from_ref(&path), &[]).unwrap();
        let mut journal = Journal {
            directory_identities: HashMap::new(),
            directories,
            effects: vec![],
            finished: false,
        };
        journal.ensure(&path, &mut Native).unwrap();
        journal.ensure(&path, &mut Native).unwrap();
        assert_eq!(journal.effects.len(), 2);
        assert!(path.is_dir());
        assert!(journal.rollback().is_empty());
        assert!(!canonical.join("one").exists());
    }

    #[test]
    fn redo_move_reports_incomplete_rollback_and_keeps_new_content() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source.txt");
        fs::write(&source, b"moving file").unwrap();
        let destination = root.path().join("destination/file.txt");
        let tx = details(&source, &destination);
        let error = redo_with(
            &tx,
            &mut Hook(|phase, _: &Path| {
                if matches!(phase, Phase::Move) {
                    fs::write(
                        destination.parent().unwrap().join("witness"),
                        b"independent",
                    )?;
                    return Err(io::Error::other("injected move failure"));
                }
                Ok(())
            }),
        )
        .unwrap_err();
        assert!(
            matches!(CommandError::from(error), CommandError::HistoryMove(failure) if failure.issue == Issue::Incomplete && failure.detail.contains("destination"))
        );
        assert_eq!(fs::read(&source).unwrap(), b"moving file");
        assert_eq!(
            fs::read(destination.parent().unwrap().join("witness")).unwrap(),
            b"independent"
        );
        assert!(!destination.exists());
    }

    #[test]
    fn redo_move_readonly_source_is_preserved_without_clearing_permissions() {
        let root = tempfile::tempdir().unwrap();
        let parent = root.path().join("source");
        fs::create_dir(&parent).unwrap();
        let source = parent.join("file.txt");
        fs::write(&source, b"moving file").unwrap();
        let destination = root.path().join("destination/file.txt");
        let mut permissions = fs::metadata(&parent).unwrap().permissions();
        permissions.set_readonly(true);
        fs::set_permissions(&parent, permissions).unwrap();
        let mut tx = details(&source, &destination);
        tx.moved_dirs = Some(vec![(
            parent.to_string_lossy().into_owned(),
            destination.parent().unwrap().to_string_lossy().into_owned(),
        )]);
        directory_proofs(&mut tx);
        let result = redo_with(&tx, &mut Native);
        assert!(result.is_err());
        assert_eq!(fs::read(&source).unwrap(), b"moving file");
        assert!(fs::metadata(&parent).unwrap().permissions().readonly());
        assert!(!destination.parent().unwrap().exists());
        let mut permissions = fs::metadata(&parent).unwrap().permissions();
        permissions.set_readonly(false);
        fs::set_permissions(&parent, permissions).unwrap();
    }

    #[test]
    fn redo_move_rollback_preserves_an_output_changed_after_publication() {
        let root = tempfile::tempdir().unwrap();
        let first = root.path().join("first.txt");
        let second = root.path().join("second.txt");
        fs::write(&first, b"moving file").unwrap();
        fs::write(&second, b"moving file").unwrap();
        let destination = root.path().join("destination/first.txt");
        let mut tx = details(&first, &destination);
        tx.moved_files.as_mut().unwrap().push((
            second.to_string_lossy().into_owned(),
            root.path()
                .join("destination/second.txt")
                .to_string_lossy()
                .into_owned(),
        ));
        tx.moved_file_identities.as_mut().unwrap().push((
            root.path()
                .join("destination/second.txt")
                .to_string_lossy()
                .into_owned(),
            crate::models::transaction::HistoryMoveIdentity::from_file(
                &fs::File::open(&second).unwrap(),
            )
            .unwrap(),
        ));
        let error = redo_with(
            &tx,
            &mut Hook(|phase, path: &Path| {
                if matches!(phase, Phase::Move) && path == second {
                    fs::write(&destination, b"new independent contents")?;
                    return Err(io::Error::other("injected second move failure"));
                }
                Ok(())
            }),
        )
        .unwrap_err();
        assert!(
            matches!(CommandError::from(error), CommandError::HistoryMove(failure) if failure.issue == Issue::Incomplete)
        );
        assert_eq!(fs::read(&destination).unwrap(), b"new independent contents");
        assert_eq!(fs::read(second).unwrap(), b"moving file");
        assert!(!first.exists());
        assert!(!root.path().join("destination/second.txt").exists());
    }

    #[test]
    fn redo_move_rollback_never_recovers_files_into_a_replacement_source_directory() {
        let root = tempfile::tempdir().unwrap();
        let first = root.path().join("first");
        let second = root.path().join("second");
        fs::create_dir(&first).unwrap();
        fs::create_dir(&second).unwrap();
        let source = first.join("file.txt");
        fs::write(&source, b"moving file").unwrap();
        let destination = root.path().join("destination/file.txt");
        let mut tx = details(&source, &destination);
        tx.moved_dirs = Some(vec![
            (
                first.to_string_lossy().into_owned(),
                destination.parent().unwrap().to_string_lossy().into_owned(),
            ),
            (
                second.to_string_lossy().into_owned(),
                destination.parent().unwrap().to_string_lossy().into_owned(),
            ),
        ]);
        directory_proofs(&mut tx);
        let error = redo_with(
            &tx,
            &mut Hook(|phase, path: &Path| {
                if matches!(phase, Phase::Remove) && path == second {
                    assert!(!first.exists());
                    fs::create_dir(&first)?;
                    fs::write(first.join("witness"), b"independent")?;
                    return Err(io::Error::other(
                        "injected cleanup failure after replacement",
                    ));
                }
                Ok(())
            }),
        )
        .unwrap_err();
        assert!(
            matches!(CommandError::from(error), CommandError::HistoryMove(failure) if failure.issue == Issue::Incomplete && failure.detail.contains("destination"))
        );
        assert_eq!(fs::read(first.join("witness")).unwrap(), b"independent");
        assert!(!source.exists());
        assert_eq!(fs::read(destination).unwrap(), b"moving file");
        assert!(second.is_dir());
    }

    #[test]
    fn redo_move_unwind_cleans_only_its_empty_created_directories() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source.txt");
        fs::write(&source, b"moving file").unwrap();
        let destination = root.path().join("one/two/file.txt");
        let tx = details(&source, &destination);
        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = redo_with(
                &tx,
                &mut Hook(|phase, _: &Path| {
                    if matches!(phase, Phase::Move) {
                        panic!("injected move panic");
                    }
                    Ok(())
                }),
            );
        }));
        assert!(panic.is_err());
        assert_eq!(fs::read(source).unwrap(), b"moving file");
        assert!(!root.path().join("one").exists());
    }
