    use super::*;

    #[test]
    fn copy_directory_without_identity_is_refused_before_any_effect() {
        let mut fixture = Fixture::new();
        fixture.details.copied_directory_identities = None;
        let error = undo_copy(&fixture.details).unwrap_err();
        assert!(
            matches!(error, CommandError::HistoryIdentity(failure) if failure.issue == crate::models::error::HistoryIdentityIssue::Unverified)
        );
        fixture.unchanged();
    }

    #[test]
    fn copy_directory_identity_is_checked_after_planning_before_staging() {
        let fixture = Fixture::new();
        let selected = fixture.root.path().join("destination");
        let displaced = fixture.root.path().join("original-directories");
        let plan = Plan::copy(&fixture.details).unwrap();
        fs::rename(&selected, &displaced).unwrap();
        fs::create_dir(&selected).unwrap();
        let error = apply_plan(&NativeIo, plan).unwrap_err();
        assert!(
            matches!(CommandError::from(error), CommandError::HistoryIdentity(failure) if failure.issue == crate::models::error::HistoryIdentityIssue::Changed)
        );
        assert!(selected.is_dir());
        assert_eq!(fs::read_dir(&selected).unwrap().count(), 0);
        assert_eq!(
            fs::read(displaced.join("child/moved.txt")).unwrap(),
            b"moved content"
        );
        assert_eq!(
            fs::read(displaced.join("child/partial.txt")).unwrap(),
            b"partial content"
        );
        assert!(fixture.staging().is_empty());
    }

    #[test]
    fn copy_without_identity_is_refused_without_guessing_from_contents() {
        let mut fixture = Fixture::new();
        fixture.details.copied_file_identities = None;
        let error = undo_copy(&fixture.details).unwrap_err();
        assert!(
            matches!(error, CommandError::HistoryIdentity(failure) if failure.issue == crate::models::error::HistoryIdentityIssue::Unverified)
        );
        assert_eq!(fs::read(&fixture.destination).unwrap(), b"moved content");
        assert_eq!(fs::read(&fixture.partial).unwrap(), b"partial content");
        assert!(fixture.staging().is_empty());
        assert!(!fixture.original.exists());
    }

    #[test]
    fn copy_identity_is_rechecked_after_planning_before_the_native_move() {
        struct Replace {
            selected: PathBuf,
            saved: PathBuf,
        }
        impl UndoIo for Replace {
            fn move_file(
                &self,
                from: &Path,
                to: &Path,
                hash: &str,
                parents: (&HeldParents, &HeldParents),
                identity: Option<HistoryFileIdentity>,
                journal_identity: Option<&crate::models::transaction::HistoryMoveIdentity>,
            ) -> io::Result<()> {
                if from == self.selected {
                    fs::rename(from, &self.saved)?;
                    fs::write(from, b"moved content")?;
                }
                NativeIo.move_file(from, to, hash, parents, identity, journal_identity)
            }
        }
        let fixture = Fixture::new();
        let saved = fixture.root.path().join("original-output");
        let plan = Plan::copy(&fixture.details).unwrap();
        let error = apply_plan(
            &Replace {
                selected: fixture.destination.clone(),
                saved: saved.clone(),
            },
            plan,
        )
        .unwrap_err();
        assert!(
            matches!(CommandError::from(error), CommandError::HistoryIdentity(failure) if failure.issue == crate::models::error::HistoryIdentityIssue::Changed)
        );
        assert_eq!(fs::read(&fixture.destination).unwrap(), b"moved content");
        assert_eq!(fs::read(saved).unwrap(), b"moved content");
        assert_eq!(fs::read(&fixture.partial).unwrap(), b"partial content");
        assert!(fixture.staging().is_empty());
    }

    #[test]
    fn trash_sidecars_require_durable_identity_before_undo_and_redo() {
        let mut fixture = Fixture::new();
        fixture.details.trash_metadata_backups =
            Some(vec![fixture.destination.to_string_lossy().into_owned()]);
        let proof = fixture.details.moved_file_identities.take().unwrap();
        assert!(
            matches!(undo(&fixture.details), Err(CommandError::HistoryMove(failure))
            if failure.issue == crate::models::error::HistoryMoveIssue::Unverified)
        );
        fixture.unchanged();
        fixture.details.moved_file_identities = Some(proof);
        undo(&fixture.details).unwrap();
        fixture.details.moved_file_identities = None;
        assert!(
            matches!(super::super::history_move_redo::redo(&fixture.details), Err(CommandError::HistoryMove(failure))
            if failure.issue == crate::models::error::HistoryMoveIssue::Unverified)
        );
        assert_eq!(fs::read(&fixture.original).unwrap(), b"moved content");
        assert!(!fixture.destination.exists());
    }

    fn empty_directory_move() -> Fixture {
        let mut fixture = Fixture::new();
        fs::remove_file(&fixture.destination).unwrap();
        fs::remove_file(&fixture.partial).unwrap();
        fixture.details.created_files = Some(vec![]);
        fixture.details.moved_files = Some(vec![]);
        fixture.details.copied_file_hashes = Some(vec![]);
        let source = fixture.original.parent().unwrap();
        let destination = fixture.destination.parent().unwrap().parent().unwrap();
        fixture.details.moved_dirs = Some(vec![
            (
                source.to_string_lossy().into_owned(),
                destination.to_string_lossy().into_owned(),
            ),
            (
                source.join("child").to_string_lossy().into_owned(),
                destination.join("child").to_string_lossy().into_owned(),
            ),
        ]);
        fixture.details.moved_directory_identities = Some(
            fixture
                .details
                .moved_dirs
                .as_ref()
                .unwrap()
                .iter()
                .map(|(path, _)| (path.clone(), HistoryDirectoryIdentity::vacant()))
                .collect(),
        );
        fixture
    }

    #[test]
    fn move_directory_replacement_before_undo_is_preserved_and_original_can_retry() {
        let fixture = empty_directory_move();
        let selected = fixture.destination.parent().unwrap();
        let saved = fixture.root.path().join("original-directory");
        fs::rename(selected, &saved).unwrap();
        fs::create_dir(selected).unwrap();
        assert!(
            matches!(undo(&fixture.details), Err(CommandError::HistoryMove(failure))
            if failure.issue == crate::models::error::HistoryMoveIssue::Changed)
        );
        assert!(selected.is_dir());
        assert!(saved.is_dir());
        assert!(!fixture.original.parent().unwrap().exists());
        fs::remove_dir(selected).unwrap();
        fs::rename(&saved, selected).unwrap();
        for _ in 0..2 {
            undo(&fixture.details).unwrap();
            assert!(fixture.original.parent().unwrap().join("child").is_dir());
            assert!(!selected.parent().unwrap().exists());
            super::super::history_move_redo::redo(&fixture.details).unwrap();
            assert!(selected.is_dir());
            assert!(!fixture.original.parent().unwrap().exists());
        }
    }

    #[test]
    fn move_directory_missing_proof_is_refused_before_any_effect() {
        for source in [false, true] {
            let mut fixture = empty_directory_move();
            if source {
                fixture.details.moved_directory_identities = None;
            } else {
                fixture.details.copied_directory_identities = None;
            }
            assert!(
                matches!(undo(&fixture.details), Err(CommandError::HistoryMove(failure))
                if failure.issue == crate::models::error::HistoryMoveIssue::Unverified)
            );
            assert!(fixture.destination.parent().unwrap().is_dir());
            assert!(!fixture.original.parent().unwrap().exists());
        }
    }

    #[test]
    fn move_directory_independent_source_arrival_is_preserved_before_undo() {
        for after_plan in [false, true] {
            let fixture = empty_directory_move();
            let plan = after_plan.then(|| Plan::new(&fixture.details).unwrap());
            let source = fixture.original.parent().unwrap();
            fs::create_dir(source).unwrap();
            fs::write(source.join("witness.txt"), b"independent").unwrap();
            let error = if let Some(plan) = plan {
                apply_plan(&NativeIo, plan).unwrap_err()
            } else {
                undo_with(&NativeIo, &fixture.details).unwrap_err()
            };
            assert!(
                matches!(CommandError::from(error), CommandError::HistoryMove(failure)
                if failure.issue == crate::models::error::HistoryMoveIssue::Changed)
            );
            assert_eq!(
                fs::read(source.join("witness.txt")).unwrap(),
                b"independent"
            );
            assert!(fixture.destination.parent().unwrap().is_dir());
        }
    }

    #[test]
    fn move_directory_replacement_after_planning_is_preserved_before_undo() {
        let fixture = empty_directory_move();
        let plan = Plan::new(&fixture.details).unwrap();
        let selected = fixture.destination.parent().unwrap();
        let saved = fixture.root.path().join("original-directory");
        fs::rename(selected, &saved).unwrap();
        fs::create_dir(selected).unwrap();
        let error = apply_plan(&NativeIo, plan).unwrap_err();
        assert!(
            matches!(CommandError::from(error), CommandError::HistoryIdentity(failure)
            if failure.issue == crate::models::error::HistoryIdentityIssue::Changed)
        );
        assert!(selected.is_dir());
        assert!(saved.is_dir());
        assert!(!fixture.original.parent().unwrap().exists());
    }

    #[test]
    fn move_directory_restored_source_replacement_is_preserved_before_redo() {
        let fixture = empty_directory_move();
        undo(&fixture.details).unwrap();
        let source = fixture.original.parent().unwrap();
        let saved = fixture.root.path().join("original-source");
        fs::rename(source, &saved).unwrap();
        fs::create_dir(source).unwrap();
        assert!(
            matches!(super::super::history_move_redo::redo(&fixture.details),
            Err(CommandError::HistoryMove(failure)) if failure.issue == crate::models::error::HistoryMoveIssue::Changed)
        );
        assert!(source.is_dir());
        assert!(saved.join("child").is_dir());
        assert!(
            !fixture
                .destination
                .parent()
                .unwrap()
                .parent()
                .unwrap()
                .exists()
        );
        fs::remove_dir(source).unwrap();
        fs::rename(saved, source).unwrap();
        super::super::history_move_redo::redo(&fixture.details).unwrap();
        undo(&fixture.details).unwrap();
    }

    #[test]
    fn move_directory_undo_rollback_refreshes_proofs_for_retry() {
        let fixture = empty_directory_move();
        let child = fixture.destination.parent().unwrap();
        let parent = child.parent().unwrap();
        let ops = FaultIo {
            remove: |path: &Path| {
                if path == parent {
                    assert!(!child.exists());
                    return Err(io::Error::other("injected failure after child removal"));
                }
                Ok(())
            },
        };
        assert!(undo_with(&ops, &fixture.details).is_err());
        assert!(child.is_dir());
        assert!(!fixture.original.parent().unwrap().exists());
        undo(&fixture.details).unwrap();
        super::super::history_move_redo::redo(&fixture.details).unwrap();
        undo(&fixture.details).unwrap();
    }

    struct Fixture {
        root: tempfile::TempDir,
        original: PathBuf,
        destination: PathBuf,
        partial: PathBuf,
        details: TransactionDetails,
    }

    #[test]
    fn move_partial_output_identity_is_required_before_any_undo_effect() {
        for replace in [false, true] {
            let mut fixture = Fixture::new();
            let saved = fixture.root.path().join("original-partial");
            if replace {
                fs::rename(&fixture.partial, &saved).unwrap();
                fs::write(&fixture.partial, b"partial content").unwrap();
            } else {
                fixture.details.copied_file_identities = None;
            }
            let error = undo(&fixture.details).unwrap_err();
            let issue = if replace {
                crate::models::error::HistoryMoveIssue::Changed
            } else {
                crate::models::error::HistoryMoveIssue::Unverified
            };
            assert!(matches!(error, CommandError::HistoryMove(failure) if failure.issue == issue));
            fixture.unchanged();
            if replace {
                assert_eq!(fs::read(saved).unwrap(), b"partial content");
            }
        }
    }

    #[test]
    fn move_partial_output_is_rechecked_after_planning_and_completed_moves_roll_back() {
        struct Replace {
            partial: PathBuf,
            saved: PathBuf,
        }
        impl UndoIo for Replace {
            fn move_file(
                &self,
                from: &Path,
                to: &Path,
                hash: &str,
                parents: (&HeldParents, &HeldParents),
                identity: Option<HistoryFileIdentity>,
                journal_identity: Option<&crate::models::transaction::HistoryMoveIdentity>,
            ) -> io::Result<()> {
                if from == self.partial {
                    fs::rename(from, &self.saved)?;
                    fs::write(from, b"partial content")?;
                }
                NativeIo.move_file(from, to, hash, parents, identity, journal_identity)
            }
        }
        let fixture = Fixture::new();
        let saved = fixture.root.path().join("original-partial");
        let error = undo_with(
            &Replace {
                partial: fixture.partial.clone(),
                saved: saved.clone(),
            },
            &fixture.details,
        )
        .unwrap_err();
        assert!(
            matches!(CommandError::from(error), CommandError::HistoryIdentity(failure) if failure.issue == crate::models::error::HistoryIdentityIssue::Changed)
        );
        fixture.unchanged();
        assert_eq!(fs::read(saved).unwrap(), b"partial content");
        assert!(
            fixture.details.moved_file_identities.as_ref().unwrap()[0]
                .1
                .snapshot()
                .unwrap()
                .matches(&fs::File::open(&fixture.destination).unwrap())
                .unwrap()
        );
    }

    #[test]
    fn move_undo_rechecks_identity_after_plan_before_publication() {
        struct Replace(PathBuf);
        impl UndoIo for Replace {
            fn move_file(
                &self,
                from: &Path,
                to: &Path,
                hash: &str,
                parents: (&HeldParents, &HeldParents),
                identity: Option<HistoryFileIdentity>,
                journal_identity: Option<&crate::models::transaction::HistoryMoveIdentity>,
            ) -> io::Result<()> {
                fs::rename(from, &self.0)?;
                fs::write(from, b"moved content")?;
                NativeIo.move_file(from, to, hash, parents, identity, journal_identity)
            }
        }
        let fixture = Fixture::new();
        let saved = fixture.root.path().join("selected-original");
        let plan = Plan::new(&fixture.details).unwrap();
        let error = apply_plan(&Replace(saved.clone()), plan).unwrap_err();
        assert!(
            matches!(CommandError::from(error), CommandError::HistoryMove(failure) if failure.issue == crate::models::error::HistoryMoveIssue::Changed)
        );
        fixture.unchanged();
        assert_eq!(fs::read(saved).unwrap(), b"moved content");
    }
    impl Fixture {
        fn new() -> Self {
            let root = tempfile::tempdir().unwrap();
            let original = root.path().join("source/moved.txt");
            let directory = root.path().join("destination/child");
            fs::create_dir_all(&directory).unwrap();
            let destination = directory.join("moved.txt");
            let partial = directory.join("partial.txt");
            fs::write(&destination, b"moved content").unwrap();
            fs::write(&partial, b"partial content").unwrap();
            let name = |path: &Path| path.to_string_lossy().into_owned();
            let details = TransactionDetails {
                deleted_items: None,
                created_links: None,
                moved_links: None,
                renamed_item_identity: None,
                moved_directory_identities: None,
                new_folder_identity: None,
                moved_file_identities: Some(vec![(
                    name(&destination),
                    crate::models::transaction::HistoryMoveIdentity::from_file(
                        &fs::File::open(&destination).unwrap(),
                    )
                    .unwrap(),
                )]),
                copied_directory_identities: Some(
                    [directory.parent().unwrap(), directory.as_path()]
                        .into_iter()
                        .map(|path| {
                            (
                                name(path),
                                crate::models::transaction::HistoryDirectoryIdentity::from_file(
                                    &platform()
                                        .hold_resolved_operation_directory(
                                            &fs::canonicalize(path).unwrap(),
                                        )
                                        .unwrap(),
                                )
                                .unwrap(),
                            )
                        })
                        .collect(),
                ),
                copied_file_identities: Some(
                    [&destination, &partial]
                        .into_iter()
                        .map(|path| {
                            (
                                name(path),
                                HistoryFileIdentity::from_file(&fs::File::open(path).unwrap())
                                    .unwrap(),
                            )
                        })
                        .collect(),
                ),
                paths: vec![name(&original)],
                target_dir: None,
                old_path: None,
                new_path: None,
                created_files: Some(vec![name(&destination), name(&partial)]),
                created_dirs: Some(vec![name(directory.parent().unwrap()), name(&directory)]),
                overwritten_files: Some(vec![]),
                moved_files: Some(vec![(name(&original), name(&destination))]),
                trash_metadata_backups: None,
                moved_dirs: Some(vec![]),
                copied_files: Some(vec![]),
                copied_file_hashes: Some(vec![
                    (
                        name(&destination),
                        blake3::hash(b"moved content").to_hex().to_string(),
                    ),
                    (
                        name(&partial),
                        blake3::hash(b"partial content").to_hex().to_string(),
                    ),
                ]),
            };
            Self {
                root,
                original,
                destination,
                partial,
                details,
            }
        }
        fn staging(&self) -> Vec<PathBuf> {
            fs::read_dir(self.root.path())
                .unwrap()
                .map(|entry| entry.unwrap().path())
                .filter(|path| {
                    path.file_name()
                        .unwrap()
                        .to_string_lossy()
                        .starts_with(".oxyde-undo-")
                })
                .collect()
        }
        fn unchanged(&self) {
            assert!(!self.original.exists());
            assert!(!self.original.parent().unwrap().exists());
            assert_eq!(fs::read(&self.destination).unwrap(), b"moved content");
            assert_eq!(fs::read(&self.partial).unwrap(), b"partial content");
            assert!(self.staging().is_empty());
        }
    }
    struct FaultIo<F> {
        remove: F,
    }
    impl<F: Fn(&Path) -> io::Result<()>> UndoIo for FaultIo<F> {
        fn move_file(
            &self,
            from: &Path,
            to: &Path,
            hash: &str,
            parents: (&HeldParents, &HeldParents),
            identity: Option<HistoryFileIdentity>,
            journal_identity: Option<&crate::models::transaction::HistoryMoveIdentity>,
        ) -> io::Result<()> {
            NativeIo.move_file(from, to, hash, parents, identity, journal_identity)
        }
        fn before_create_dir(&self, path: &Path) -> io::Result<()> {
            NativeIo.before_create_dir(path)
        }
        fn before_remove_dir(&self, path: &Path) -> io::Result<()> {
            (self.remove)(path)
        }
    }

    #[test]
    fn copy_undo_preserves_a_directory_replaced_before_removal() {
        let fixture = Fixture::new();
        let selected = fixture.partial.parent().unwrap().to_path_buf();
        let relocated = fixture.root.path().join("relocated");
        let ops = FaultIo {
            remove: |path: &Path| {
                if path == selected {
                    fs::rename(path, &relocated)?;
                    fs::create_dir(path)?;
                }
                Ok(())
            },
        };
        let result = apply_plan(&ops, Plan::copy(&fixture.details).unwrap());
        assert!(selected.is_dir(), "undo deleted a replacement directory");
        assert!(relocated.is_dir());
        assert!(result.is_err());
        assert_eq!(fs::read_dir(&selected).unwrap().count(), 0);
        assert_eq!(
            fs::read(relocated.join("moved.txt")).unwrap(),
            b"moved content"
        );
        assert_eq!(
            fs::read(relocated.join("partial.txt")).unwrap(),
            b"partial content"
        );
        assert!(fixture.staging().is_empty());
    }

    #[test]
    fn rollback_preserves_a_replacement_for_a_directory_created_by_undo() {
        let fixture = Fixture::new();
        let source = fixture.original.parent().unwrap().to_path_buf();
        let relocated = fixture.root.path().join("relocated-source");
        let ops = FaultIo {
            remove: |_path: &Path| {
                fs::rename(&source, &relocated)?;
                fs::create_dir(&source)?;
                Err(io::Error::other("injected source replacement"))
            },
        };
        let error = undo_with(&ops, &fixture.details).unwrap_err().to_string();
        assert!(error.contains("replacement was preserved"), "{error}");
        assert!(source.is_dir());
        assert_eq!(fs::read_dir(&source).unwrap().count(), 0);
        assert_eq!(fs::read_dir(&relocated).unwrap().count(), 0);
        assert_eq!(fs::read(&fixture.destination).unwrap(), b"moved content");
        assert_eq!(fs::read(&fixture.partial).unwrap(), b"partial content");
        assert!(fixture.staging().is_empty());
    }

    #[test]
    fn undo_keeps_a_directory_absent_from_the_initial_capture() {
        struct Arrival(PathBuf);
        impl UndoIo for Arrival {
            fn move_file(
                &self,
                from: &Path,
                to: &Path,
                hash: &str,
                parents: (&HeldParents, &HeldParents),
                identity: Option<HistoryFileIdentity>,
                journal_identity: Option<&crate::models::transaction::HistoryMoveIdentity>,
            ) -> io::Result<()> {
                NativeIo.move_file(from, to, hash, parents, identity, journal_identity)?;
                if !self.0.exists() {
                    fs::create_dir(&self.0)?;
                }
                Ok(())
            }
        }
        let mut fixture = Fixture::new();
        let arrival = fixture.root.path().join("new-empty");
        fs::create_dir(&arrival).unwrap();
        fixture
            .details
            .copied_directory_identities
            .as_mut()
            .unwrap()
            .push((
                arrival.to_string_lossy().into_owned(),
                HistoryDirectoryIdentity::from_file(
                    &platform()
                        .hold_resolved_operation_directory(&fs::canonicalize(&arrival).unwrap())
                        .unwrap(),
                )
                .unwrap(),
            ));
        fs::remove_dir(&arrival).unwrap();
        fixture
            .details
            .created_dirs
            .as_mut()
            .unwrap()
            .push(arrival.to_string_lossy().into_owned());
        undo_with(&Arrival(arrival.clone()), &fixture.details).unwrap();
        assert!(arrival.is_dir());
        assert_eq!(fs::read(&fixture.original).unwrap(), b"moved content");
        assert!(!fixture.destination.parent().unwrap().exists());
        assert!(fixture.staging().is_empty());
    }

    #[cfg(target_os = "windows")]
    fn exercise_undo_restoration_replacement(copy: bool) {
        let fixture = Fixture::new();
        let selected = fixture.destination.parent().unwrap().to_path_buf();
        let parent = selected.parent().unwrap().to_path_buf();
        let ops = FaultIo {
            remove: |path: &Path| {
                if path == parent {
                    assert!(!selected.exists());
                    fs::create_dir(&selected)?;
                    fs::write(selected.join("independent.txt"), b"independent contents")?;
                    return Err(io::Error::other("injected restoration conflict"));
                }
                Ok(())
            },
        };
        let result = if copy {
            apply_plan(&ops, Plan::copy(&fixture.details).unwrap())
        } else {
            undo_with(&ops, &fixture.details)
        };
        let error = result.unwrap_err();
        let failure = error
            .get_ref()
            .unwrap()
            .downcast_ref::<crate::models::error::HistoryUndoRecoveryFailure>()
            .unwrap();
        assert!(
            failure
                .technical_detail
                .contains("injected restoration conflict")
        );
        let locations: Vec<_> = failure
            .locations
            .lines()
            .map(|path| fs::canonicalize(path).unwrap())
            .collect();
        assert!(
            locations.contains(&fs::canonicalize(&selected).unwrap()),
            "{}",
            failure.locations
        );
        for path in fixture.staging() {
            assert!(
                locations.contains(&fs::canonicalize(path).unwrap()),
                "{}",
                failure.locations
            );
        }
        assert_eq!(
            fs::read(selected.join("independent.txt")).unwrap(),
            b"independent contents"
        );
        assert!(
            !fixture.destination.exists(),
            "Undo rollback recovered a file into an independent replacement directory"
        );
        assert!(
            !fixture.partial.exists(),
            "Undo rollback recovered partial output into a replacement directory"
        );
        let mut recovery: Vec<_> = fixture
            .staging()
            .iter()
            .map(|path| fs::read(path.join("output")).unwrap())
            .collect();
        if copy {
            assert!(!fixture.original.exists());
            recovery.sort();
            assert_eq!(
                recovery,
                vec![b"moved content".to_vec(), b"partial content".to_vec()]
            );
        } else {
            assert_eq!(fs::read(&fixture.original).unwrap(), b"moved content");
            assert_eq!(recovery, vec![b"partial content".to_vec()]);
        }
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn copy_undo_rollback_preserves_a_replacement_after_directory_restoration_fails() {
        exercise_undo_restoration_replacement(true);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn move_undo_rollback_preserves_a_replacement_after_directory_restoration_fails() {
        exercise_undo_restoration_replacement(false);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn undo_rollback_blocks_descendants_of_an_unrestored_parent() {
        let fixture = Fixture::new();
        let selected = fixture.destination.parent().unwrap().to_path_buf();
        let parent = selected.parent().unwrap().to_path_buf();
        let trigger = fixture.root.path().join("last-empty");
        fs::create_dir(&trigger).unwrap();
        let mut plan = Plan::copy(&fixture.details).unwrap();
        plan.remove_dirs.push(trigger.clone());
        let ops = FaultIo {
            remove: |path: &Path| {
                if path == trigger {
                    assert!(!parent.exists());
                    fs::create_dir(&parent)?;
                    fs::write(parent.join("independent.txt"), b"independent contents")?;
                    return Err(io::Error::other("injected parent restoration conflict"));
                }
                Ok(())
            },
        };
        let error = apply_plan(&ops, plan).unwrap_err();
        assert!(matches!(
            CommandError::from(error),
            CommandError::HistoryUndoRecovery(_)
        ));
        assert_eq!(
            fs::read(parent.join("independent.txt")).unwrap(),
            b"independent contents"
        );
        assert!(
            !selected.exists(),
            "Rollback created descendants inside an independent replacement parent"
        );
        assert!(trigger.is_dir());
        let mut contents: Vec<_> = fixture
            .staging()
            .iter()
            .map(|path| fs::read(path.join("output")).unwrap())
            .collect();
        contents.sort();
        assert_eq!(
            contents,
            vec![b"moved content".to_vec(), b"partial content".to_vec()]
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn undo_unwind_preserves_replacements_and_staged_recovery() {
        let fixture = Fixture::new();
        let selected = fixture.destination.parent().unwrap().to_path_buf();
        let parent = selected.parent().unwrap().to_path_buf();
        let ops = FaultIo {
            remove: |path: &Path| {
                if path == parent {
                    assert!(!selected.exists());
                    fs::create_dir(&selected).unwrap();
                    fs::write(selected.join("independent.txt"), b"independent contents").unwrap();
                    panic!("injected Undo panic after directory replacement");
                }
                Ok(())
            },
        };
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            apply_plan(&ops, Plan::copy(&fixture.details).unwrap())
        }));
        assert!(result.is_err());
        assert_eq!(
            fs::read(selected.join("independent.txt")).unwrap(),
            b"independent contents"
        );
        assert!(!fixture.destination.exists());
        assert!(!fixture.partial.exists());
        let mut contents: Vec<_> = fixture
            .staging()
            .iter()
            .map(|path| fs::read(path.join("output")).unwrap())
            .collect();
        contents.sort();
        assert_eq!(
            contents,
            vec![b"moved content".to_vec(), b"partial content".to_vec()]
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn undo_rollback_restores_files_outside_a_blocked_parent() {
        let mut fixture = Fixture::new();
        let safe = fixture.root.path().join("safe.txt");
        fs::write(&safe, b"safe contents").unwrap();
        let name = safe.to_string_lossy().into_owned();
        fixture
            .details
            .created_files
            .as_mut()
            .unwrap()
            .push(name.clone());
        fixture.details.copied_file_hashes.as_mut().unwrap().push((
            name.clone(),
            blake3::hash(b"safe contents").to_hex().to_string(),
        ));
        fixture
            .details
            .copied_file_identities
            .as_mut()
            .unwrap()
            .push((
                name,
                HistoryFileIdentity::from_file(&fs::File::open(&safe).unwrap()).unwrap(),
            ));
        let selected = fixture.destination.parent().unwrap().to_path_buf();
        let parent = selected.parent().unwrap().to_path_buf();
        let ops = FaultIo {
            remove: |path: &Path| {
                if path == parent {
                    fs::create_dir(&selected)?;
                    fs::write(selected.join("independent.txt"), b"independent contents")?;
                    return Err(io::Error::other("injected isolated restoration conflict"));
                }
                Ok(())
            },
        };
        let error = apply_plan(&ops, Plan::copy(&fixture.details).unwrap()).unwrap_err();
        assert!(matches!(
            CommandError::from(error),
            CommandError::HistoryUndoRecovery(_)
        ));
        assert_eq!(fs::read(&safe).unwrap(), b"safe contents");
        assert_eq!(
            fs::read(selected.join("independent.txt")).unwrap(),
            b"independent contents"
        );
        assert!(!fixture.destination.exists());
        assert!(!fixture.partial.exists());
        let mut contents: Vec<_> = fixture
            .staging()
            .iter()
            .map(|path| fs::read(path.join("output")).unwrap())
            .collect();
        contents.sort();
        assert_eq!(
            contents,
            vec![b"moved content".to_vec(), b"partial content".to_vec()]
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn copy_undo_retains_recovery_without_writing_through_a_new_junction() {
        use crate::platform::windows_reparse_tests::{Retarget, junction};
        let fixture = Fixture::new();
        let selected = fixture.partial.parent().unwrap();
        let outside = fixture.root.path().join("outside");
        fs::create_dir(&outside).unwrap();
        let template = fixture.root.path().join("template");
        junction(&template, &outside);
        let retarget = Retarget::new(selected, &template);
        let ops = FaultIo {
            remove: |path: &Path| {
                if path == selected {
                    retarget.apply();
                }
                Ok(())
            },
        };
        let error = apply_plan(&ops, Plan::copy(&fixture.details).unwrap())
            .unwrap_err()
            .to_string();
        assert!(error.contains("rollback incomplete"), "{error}");
        assert!(error.contains("staging retained"), "{error}");
        assert_eq!(fs::read_dir(&outside).unwrap().count(), 0);
        let staging = fixture.staging();
        assert_eq!(staging.len(), 2);
        let mut contents: Vec<_> = staging
            .iter()
            .map(|path| fs::read(path.join("output")).unwrap())
            .collect();
        contents.sort();
        assert_eq!(
            contents,
            vec![b"moved content".to_vec(), b"partial content".to_vec()]
        );
        for path in &staging {
            assert!(error.contains(path.to_str().unwrap()), "{error}");
        }
        retarget.clear();
        drop(retarget);
        fs::remove_dir(template).unwrap();
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn undo_directory_creation_stays_in_the_initial_junction_parent() {
        use crate::platform::windows_reparse_tests::{Retarget, junction};
        struct RetargetIo(Retarget);
        impl UndoIo for RetargetIo {
            fn move_file(
                &self,
                from: &Path,
                to: &Path,
                hash: &str,
                parents: (&HeldParents, &HeldParents),
                identity: Option<HistoryFileIdentity>,
                journal_identity: Option<&crate::models::transaction::HistoryMoveIdentity>,
            ) -> io::Result<()> {
                NativeIo.move_file(from, to, hash, parents, identity, journal_identity)
            }
            fn before_create_dir(&self, _path: &Path) -> io::Result<()> {
                self.0.apply();
                Ok(())
            }
        }
        let root = tempfile::tempdir().unwrap();
        let chosen = root.path().join("chosen");
        let outside = root.path().join("outside");
        fs::create_dir(&chosen).unwrap();
        fs::create_dir(&outside).unwrap();
        let link = root.path().join("link");
        let template = root.path().join("template");
        junction(&link, &chosen);
        junction(&template, &outside);
        let ops = RetargetIo(Retarget::new(&link, &template));
        apply_plan(
            &ops,
            Plan {
                directory_identities: HashMap::new(),
                identities: HashMap::new(),
                files: vec![],
                discard: vec![],
                links: vec![],
                moved_links: vec![],
                source_dirs: vec![link.join("created")],
                remove_dirs: vec![],
            },
        )
        .unwrap();
        assert!(chosen.join("created").is_dir());
        assert!(!outside.join("created").exists());
        drop(ops);
        fs::remove_dir(&link).unwrap();
        fs::remove_dir(&template).unwrap();
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn undo_directory_removal_stays_in_the_initial_junction_parent() {
        use crate::platform::windows_reparse_tests::{Retarget, junction};
        let root = tempfile::tempdir().unwrap();
        let chosen = root.path().join("chosen");
        let outside = root.path().join("outside");
        fs::create_dir_all(chosen.join("empty")).unwrap();
        fs::create_dir_all(outside.join("empty")).unwrap();
        fs::write(outside.join("witness"), b"outside witness").unwrap();
        let link = root.path().join("link");
        let template = root.path().join("template");
        junction(&link, &chosen);
        junction(&template, &outside);
        let retarget = Retarget::new(&link, &template);
        let ops = FaultIo {
            remove: |_path: &Path| {
                retarget.apply();
                Ok(())
            },
        };
        apply_plan(
            &ops,
            Plan {
                directory_identities: HashMap::new(),
                identities: HashMap::new(),
                files: vec![],
                discard: vec![],
                links: vec![],
                moved_links: vec![],
                source_dirs: vec![],
                remove_dirs: vec![link.join("empty")],
            },
        )
        .unwrap();
        assert!(!chosen.join("empty").exists());
        assert!(outside.join("empty").is_dir());
        assert_eq!(
            fs::read(outside.join("witness")).unwrap(),
            b"outside witness"
        );
        drop(retarget);
        fs::remove_dir(&link).unwrap();
        fs::remove_dir(&template).unwrap();
    }

    #[test]
    fn late_directory_error_restores_staged_output_removed_directory_and_files() {
        let fixture = Fixture::new();
        let parent = fixture.root.path().join("destination");
        let ops = FaultIo {
            remove: |path: &Path| {
                if path == parent {
                    Err(io::Error::new(
                        ErrorKind::PermissionDenied,
                        "injected final directory error",
                    ))
                } else {
                    Ok(())
                }
            },
        };
        let error = undo_with(&ops, &fixture.details).unwrap_err();
        assert_eq!(error.to_string(), "injected final directory error");
        fixture.unchanged();
        undo_with(&NativeIo, &fixture.details).unwrap();
        assert_eq!(fs::read(&fixture.original).unwrap(), b"moved content");
        assert!(!parent.exists());
        assert!(fixture.staging().is_empty());
    }

    #[test]
    fn unwind_rolls_back_all_completed_phases() {
        let fixture = Fixture::new();
        let parent = fixture.root.path().join("destination");
        let ops = FaultIo {
            remove: |path: &Path| {
                assert_ne!(path, parent, "injected late panic");
                Ok(())
            },
        };
        let result = std::panic::catch_unwind(|| undo_with(&ops, &fixture.details));
        assert!(result.is_err());
        fixture.unchanged();
    }

    #[test]
    fn source_directory_error_removes_only_parents_created_by_undo() {
        struct CreateError(PathBuf);
        impl UndoIo for CreateError {
            fn move_file(
                &self,
                from: &Path,
                to: &Path,
                hash: &str,
                parents: (&HeldParents, &HeldParents),
                identity: Option<HistoryFileIdentity>,
                journal_identity: Option<&crate::models::transaction::HistoryMoveIdentity>,
            ) -> io::Result<()> {
                NativeIo.move_file(from, to, hash, parents, identity, journal_identity)
            }
            fn before_create_dir(&self, path: &Path) -> io::Result<()> {
                if path == self.0 {
                    Err(io::Error::new(
                        ErrorKind::PermissionDenied,
                        "injected directory creation error",
                    ))
                } else {
                    Ok(())
                }
            }
            fn before_remove_dir(&self, _path: &Path) -> io::Result<()> {
                Ok(())
            }
        }
        let mut fixture = Fixture::new();
        let failure = fixture.root.path().join("additional/child");
        fixture.details.moved_dirs = Some(vec![(
            failure.to_string_lossy().into_owned(),
            String::new(),
        )]);
        fixture.details.moved_directory_identities = Some(
            fixture
                .details
                .moved_dirs
                .as_ref()
                .unwrap()
                .iter()
                .map(|(path, _)| (path.clone(), HistoryDirectoryIdentity::vacant()))
                .collect(),
        );
        let error = undo_with(&CreateError(failure.clone()), &fixture.details).unwrap_err();
        assert_eq!(error.to_string(), "injected directory creation error");
        assert!(!failure.parent().unwrap().exists());
        fixture.unchanged();
    }

    #[test]
    fn rollback_conflict_retains_staged_content_and_preserves_foreign_file() {
        let fixture = Fixture::new();
        let parent = fixture.root.path().join("destination");
        let ops = FaultIo {
            remove: |path: &Path| {
                if path == parent {
                    fs::create_dir(fixture.partial.parent().unwrap())?;
                    fs::write(&fixture.partial, b"foreign content")?;
                    Err(io::Error::new(
                        ErrorKind::PermissionDenied,
                        "injected conflict",
                    ))
                } else {
                    Ok(())
                }
            },
        };
        let error = undo_with(&ops, &fixture.details).unwrap_err().to_string();
        assert!(
            error.starts_with("injected conflict; rollback incomplete:"),
            "{error}"
        );
        assert_eq!(fs::read(&fixture.partial).unwrap(), b"foreign content");
        assert!(!fixture.destination.exists());
        assert_eq!(fs::read(&fixture.original).unwrap(), b"moved content");
        let staging = fixture.staging();
        assert_eq!(staging.len(), 1);
        assert_eq!(
            fs::read(staging[0].join("output")).unwrap(),
            b"partial content"
        );
        assert!(error.contains(staging[0].to_str().unwrap()), "{error}");
    }

    #[test]
    fn changed_partial_output_is_refused_before_any_file_moves() {
        let fixture = Fixture::new();
        fs::write(&fixture.partial, b"external modification").unwrap();
        let error = undo_with(&NativeIo, &fixture.details).unwrap_err();
        assert!(error.to_string().contains("file changed"));
        assert!(!fixture.original.exists());
        assert_eq!(fs::read(&fixture.destination).unwrap(), b"moved content");
        assert_eq!(
            fs::read(&fixture.partial).unwrap(),
            b"external modification"
        );
        assert!(fixture.staging().is_empty());
    }

    #[test]
    fn partial_output_without_journal_hash_is_refused() {
        let mut fixture = Fixture::new();
        fixture.details.copied_file_hashes.as_mut().unwrap().pop();
        let error = undo_with(&NativeIo, &fixture.details).unwrap_err();
        assert!(error.to_string().contains("unverified copy/move output"));
        fixture.unchanged();
    }

    #[test]
    fn successful_undo_keeps_foreign_directory_contents() {
        let fixture = Fixture::new();
        let foreign = fixture.partial.parent().unwrap().join("foreign.txt");
        fs::write(&foreign, b"foreign content").unwrap();
        undo_with(&NativeIo, &fixture.details).unwrap();
        assert_eq!(fs::read(&fixture.original).unwrap(), b"moved content");
        assert_eq!(fs::read(&foreign).unwrap(), b"foreign content");
        assert!(!fixture.destination.exists());
        assert!(!fixture.partial.exists());
        assert!(fixture.staging().is_empty());
    }

    #[test]
    fn copy_directory_error_and_unwind_restore_all_staged_outputs() {
        for panic in [false, true] {
            let fixture = Fixture::new();
            let parent = fixture.root.path().join("destination");
            let ops = FaultIo {
                remove: |path: &Path| {
                    if path == parent {
                        assert!(!panic, "injected Copy Undo panic");
                        Err(io::Error::new(
                            ErrorKind::PermissionDenied,
                            "injected Copy Undo directory error",
                        ))
                    } else {
                        Ok(())
                    }
                },
            };
            let result = std::panic::catch_unwind(|| {
                apply_plan(&ops, Plan::copy(&fixture.details).unwrap())
            });
            if panic {
                assert!(result.is_err());
            } else {
                assert_eq!(
                    result.unwrap().unwrap_err().to_string(),
                    "injected Copy Undo directory error"
                );
            }
            fixture.unchanged();
            for (path, identity) in fixture
                .details
                .copied_directory_identities
                .as_ref()
                .unwrap()
            {
                let file = platform()
                    .hold_resolved_operation_directory(&fs::canonicalize(path).unwrap())
                    .unwrap();
                assert!(
                    identity.snapshot().unwrap().matches(&file).unwrap(),
                    "rollback did not refresh {}",
                    path
                );
            }
            apply_plan(&NativeIo, Plan::copy(&fixture.details).unwrap()).unwrap();
            assert!(!parent.exists());
            assert!(fixture.staging().is_empty());
        }
    }

    #[test]
    fn copy_missing_output_is_refused_before_staging_other_files() {
        let fixture = Fixture::new();
        fs::remove_file(&fixture.partial).unwrap();
        assert_eq!(
            Plan::copy(&fixture.details).err().unwrap().kind(),
            ErrorKind::NotFound
        );
        assert_eq!(fs::read(&fixture.destination).unwrap(), b"moved content");
        assert!(fixture.staging().is_empty());
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn copy_writer_on_second_output_rolls_back_first_staged_file() {
        let fixture = Fixture::new();
        let plan = Plan::copy(&fixture.details).unwrap();
        let writer = fs::File::options()
            .write(true)
            .open(&fixture.partial)
            .unwrap();
        let error = apply_plan(&NativeIo, plan).unwrap_err();
        assert_eq!(error.raw_os_error(), Some(32));
        fixture.unchanged();
        drop(writer);
        apply_plan(&NativeIo, Plan::copy(&fixture.details).unwrap()).unwrap();
        assert!(!fixture.destination.exists());
        assert!(!fixture.partial.exists());
        assert!(fixture.staging().is_empty());
    }
