    use super::*;
    use std::fs;
    use std::io::ErrorKind;

    use super::super::copy_output::Output;
    use super::super::path_parents::HeldParents;
    use std::path::PathBuf;

    #[test]
    fn output_preserves_an_existing_file_after_a_native_creation_collision() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source");
        let destination = root.path().join("destination");
        fs::write(&source, b"selected contents").unwrap();
        fs::write(&destination, b"independent contents").unwrap();
        let parents = HeldParents::new(&destination).unwrap();
        let output = Output::new(&parents, &destination, false).unwrap();
        let mut captured = false;
        let error = platform()
            .copy_file_with_progress(
                &source,
                output.path(),
                None,
                parents.directory(),
                &mut |file| {
                    captured = true;
                    output.capture(file)
                },
                &mut |_| Ok(()),
            )
            .unwrap_err();
        assert_eq!(error.kind(), ErrorKind::AlreadyExists);
        assert!(!captured);
        drop(output);
        assert_eq!(fs::read(&destination).unwrap(), b"independent contents");
    }
    #[test]
    fn output_refuses_a_same_content_replacement_for_verification_and_cleanup() {
        let root = tempfile::tempdir().unwrap();
        let destination = root.path().join("destination");
        let parents = HeldParents::new(&destination).unwrap();
        let output = Output::new(&parents, &destination, false).unwrap();
        let file = platform()
            .create_operation_output(parents.directory(), output.path())
            .unwrap();
        use std::io::Write;
        (&file).write_all(b"matching contents").unwrap();
        output.capture(&file).unwrap();
        drop(file);
        drop(output.open_verified().unwrap());
        let displaced = root.path().join("original-output");
        fs::rename(&destination, &displaced).unwrap();
        fs::write(&destination, b"matching contents").unwrap();
        assert!(output.open_verified().is_err());
        drop(output);
        assert_eq!(fs::read(&destination).unwrap(), b"matching contents");
        assert_eq!(fs::read(displaced).unwrap(), b"matching contents");
    }
    #[test]
    fn output_cleanup_removes_only_the_created_file() {
        let root = tempfile::tempdir().unwrap();
        let destination = root.path().join("destination");
        let parents = HeldParents::new(&destination).unwrap();
        let output = Output::new(&parents, &destination, false).unwrap();
        let file = platform()
            .create_operation_output(parents.directory(), output.path())
            .unwrap();
        output.capture(&file).unwrap();
        drop(file);
        fs::write(root.path().join("independent"), b"independent contents").unwrap();
        drop(output);
        assert!(!destination.exists());
        assert_eq!(
            fs::read(root.path().join("independent")).unwrap(),
            b"independent contents"
        );
    }
    #[test]
    fn output_never_adopts_a_file_it_did_not_create() {
        let root = tempfile::tempdir().unwrap();
        let destination = root.path().join("destination");
        let parents = HeldParents::new(&destination).unwrap();
        let output = Output::new(&parents, &destination, false).unwrap();
        fs::write(&destination, b"independent contents").unwrap();
        assert!(output.open_verified().is_err());
        drop(output);
        assert_eq!(fs::read(&destination).unwrap(), b"independent contents");
    }
    #[test]
    fn a_partial_output_the_system_refuses_to_remove_is_emptied() {
        use std::io::Write;
        use std::os::windows::fs::OpenOptionsExt;
        let root = tempfile::tempdir().unwrap();
        let destination = root.path().join("destination");
        let parents = HeldParents::new(&destination).unwrap();
        let output = Output::new(&parents, &destination, false).unwrap();
        // A creation without delete access, still open without delete sharing: as a
        // drop folder, the system refuses to remove the file.
        let file = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .share_mode(0x1 | 0x2)
            .open(output.path())
            .unwrap();
        (&file).write_all(b"partial contents").unwrap();
        output.capture(&file).unwrap();
        drop(output);
        assert_eq!(file.metadata().unwrap().len(), 0);
        drop(file);
        assert_eq!(fs::read(&destination).unwrap(), b"");
    }
    #[test]
    fn a_replacement_writes_beside_its_target_and_takes_its_place() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source");
        let destination = root.path().join("rapport.pdf");
        fs::write(&source, b"new contents").unwrap();
        fs::write(&destination, b"old contents").unwrap();
        let mut seen = None;
        copy(&source, &destination, true, &mut |bytes| {
            if bytes > 0 && seen.is_none() {
                seen = fs::read_dir(root.path())?
                    .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
                    .find(|name| name.starts_with(".rapport.pdf.oxyde-"));
                assert_eq!(fs::read(&destination)?, b"old contents");
            }
            Ok(())
        })
        .unwrap();
        assert!(seen.is_some_and(|name| name.len() == ".rapport.pdf.oxyde-".len() + 6));
        assert_eq!(fs::read(&destination).unwrap(), b"new contents");
        assert_eq!(fs::read_dir(root.path()).unwrap().count(), 2);
    }
    #[test]
    fn a_new_file_is_written_under_its_own_name() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source");
        let destination = root.path().join("destination");
        fs::write(&source, vec![0x42; 256 * 1024]).unwrap();
        let mut checked = false;
        copy(&source, &destination, false, &mut |bytes| {
            if bytes > 0 && !checked {
                checked = true;
                assert!(destination.exists());
                assert_eq!(fs::read_dir(root.path())?.count(), 2);
            }
            Ok(())
        })
        .unwrap();
        assert!(checked);
        assert_eq!(fs::read(&destination).unwrap(), fs::read(&source).unwrap());
    }
    #[test]
    fn identity_checked_cleanup_preserves_a_matching_replacement() {
        let root = tempfile::tempdir().unwrap();
        let output = root.path().join("output");
        let saved = root.path().join("selected-output");
        fs::write(&output, b"matching contents").unwrap();
        let identity = crate::models::transaction::HistoryFileIdentity::from_file(
            &fs::File::open(&output).unwrap(),
        )
        .unwrap();
        fs::rename(&output, &saved).unwrap();
        fs::write(&output, b"matching contents").unwrap();
        let expected = blake3::hash(b"matching contents").to_hex().to_string();
        let error =
            remove_verified_output_identity(&output, &expected, Some(identity)).unwrap_err();
        assert!(matches!(
            crate::models::CommandError::from(error),
            crate::models::CommandError::HistoryIdentity(_)
        ));
        assert_eq!(fs::read(output).unwrap(), b"matching contents");
        assert_eq!(fs::read(saved).unwrap(), b"matching contents");
    }

    #[test]
    fn identity_cleanup_without_a_snapshot_preserves_the_output() {
        let root = tempfile::tempdir().unwrap();
        let output = root.path().join("output");
        fs::write(&output, b"retained contents").unwrap();
        let expected = blake3::hash(b"retained contents").to_hex().to_string();
        let error = remove_verified_output_identity(&output, &expected, None).unwrap_err();
        assert!(
            matches!(crate::models::CommandError::from(error), crate::models::CommandError::HistoryIdentity(failure) if failure.issue == crate::models::error::HistoryIdentityIssue::Unverified)
        );
        assert_eq!(fs::read(output).unwrap(), b"retained contents");
    }

    #[test]
    fn redo_copy_rejects_changed_sources_and_cancels_verification_before_writing() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source");
        let destination = root.path().join("destination");
        fs::write(&source, vec![0x42; 256 * 1024]).unwrap();
        let error = copy_verified(&source, &destination, "old hash", &mut |_| Ok(())).unwrap_err();
        assert!(
            matches!(crate::models::CommandError::from(error), crate::models::CommandError::HistoryCopy(failure) if failure.issue == crate::models::error::HistoryCopyIssue::SourceChanged)
        );
        assert!(!destination.exists());
        let expected = blake3::hash(&vec![0x42; 256 * 1024]).to_hex().to_string();
        let mut calls = 0;
        let error = copy_verified(&source, &destination, &expected, &mut |_| {
            calls += 1;
            if calls == 2 {
                Err(io::Error::new(
                    ErrorKind::Interrupted,
                    "cancel verification",
                ))
            } else {
                Ok(())
            }
        })
        .unwrap_err();
        assert_eq!(error.kind(), ErrorKind::Interrupted);
        assert!(!destination.exists());
        assert_eq!(fs::read(&source).unwrap(), vec![0x42; 256 * 1024]);
        assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
    }

    #[test]
    fn redo_copy_never_keeps_a_hash_mismatch() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source");
        let destination = root.path().join("destination");
        fs::write(&source, b"unexpected copied bytes").unwrap();
        let parents = super::super::path_parents::HeldParents::new(&destination).unwrap();
        let result = copy_with_parents_checked(
            &source,
            &destination,
            false,
            None,
            None,
            Some("expected journal hash"),
            parents,
            &mut |_| Ok(()),
            None,
            false,
        );
        let error = match result {
            Ok(_) => panic!("unverified output was kept"),
            Err(error) => error,
        };
        assert!(matches!(
            crate::models::CommandError::from(error),
            crate::models::CommandError::HistoryCopy(_)
        ));
        assert!(!destination.exists());
        assert_eq!(fs::read(&source).unwrap(), b"unexpected copied bytes");
        assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
    }

    #[test]
    fn redo_copy_stays_in_its_resolved_source_after_junction_retargeting() {
        use crate::platform::windows_reparse_tests::{Retarget, junction};
        let root = tempfile::tempdir().unwrap();
        let selected = root.path().join("selected");
        let outside = root.path().join("outside");
        fs::create_dir(&selected).unwrap();
        fs::create_dir(&outside).unwrap();
        fs::write(selected.join("source"), b"verified source").unwrap();
        fs::write(outside.join("source"), b"outside witness").unwrap();
        let link = root.path().join("link");
        let template = root.path().join("template");
        junction(&link, &selected);
        junction(&template, &outside);
        let retarget = Retarget::new(&link, &template);
        let destination = root.path().join("destination");
        let expected = blake3::hash(b"verified source").to_hex().to_string();
        copy_verified_with_checkpoint(
            &link.join("source"),
            &destination,
            &expected,
            &mut || {
                retarget.apply();
                Ok(())
            },
            &mut |_| Ok(()),
        )
        .unwrap();
        assert_eq!(fs::read(destination).unwrap(), b"verified source");
        assert_eq!(
            fs::read(selected.join("source")).unwrap(),
            b"verified source"
        );
        assert_eq!(
            fs::read(outside.join("source")).unwrap(),
            b"outside witness"
        );
        drop(retarget);
        fs::remove_dir(link).unwrap();
        fs::remove_dir(template).unwrap();
    }

    #[test]
    fn redo_copy_keeps_the_verified_source_when_replacement_is_attempted() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source");
        let destination = root.path().join("destination");
        let displaced = root.path().join("displaced");
        fs::write(&source, b"verified contents").unwrap();
        let expected = blake3::hash(b"verified contents").to_hex().to_string();
        let mut blocked = false;
        let result = copy_verified_with_checkpoint(
            &source,
            &destination,
            &expected,
            &mut || {
                if fs::rename(&source, &displaced).is_ok() {
                    fs::write(&source, b"independent replacement")?;
                } else {
                    blocked = true;
                    assert!(fs::write(&source, b"concurrent write").is_err());
                }
                Ok(())
            },
            &mut |_| Ok(()),
        );
        assert!(result.is_ok(), "verified copy failed: {result:?}");
        assert_eq!(
            fs::read(&destination).unwrap(),
            b"verified contents",
            "replacement was published after successful hash verification"
        );
        assert!(blocked);
        assert_eq!(fs::read(&source).unwrap(), b"verified contents");
        assert!(!displaced.exists());
    }

    #[test]
    fn verified_copy_cleanup_preserves_readonly_foreign_hardlink_attributes() {
        let root = tempfile::tempdir().unwrap();
        let witness = root.path().join("foreign-witness");
        let output = root.path().join("copied-output");
        fs::write(&witness, b"same verified data").unwrap();
        fs::hard_link(&witness, &output).unwrap();
        let mut permissions = fs::metadata(&witness).unwrap().permissions();
        permissions.set_readonly(true);
        fs::set_permissions(&witness, permissions.clone()).unwrap();
        let expected = blake3::hash(b"same verified data").to_hex().to_string();
        let result = remove_verified_output(&output, &expected);
        let preserved = fs::metadata(&witness).unwrap().permissions().readonly();
        permissions.set_readonly(false);
        fs::set_permissions(&witness, permissions).unwrap();
        assert!(
            preserved,
            "verified copy cleanup changed the foreign hardlink attributes"
        );
        assert!(result.is_err());
        assert_eq!(fs::read(&witness).unwrap(), b"same verified data");
        assert_eq!(fs::read(&output).unwrap(), b"same verified data");
    }

    #[test]
    fn a_copy_never_leaves_its_output_in_a_new_reparse_target() {
        use crate::platform::windows_reparse_tests::{Retarget, junction};
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source");
        let selected = root.path().join("selected");
        let outside = root.path().join("outside");
        fs::write(&source, vec![0x42; 1024 * 1024]).unwrap();
        fs::create_dir(&selected).unwrap();
        fs::create_dir(&outside).unwrap();
        fs::write(outside.join("witness"), b"outside witness").unwrap();
        let template = root.path().join("template");
        junction(&template, &outside);
        let retarget = Retarget::new(&selected, &template);
        let mut changed = false;
        let result = copy(&source, &selected.join("output"), false, &mut |bytes| {
            if !changed {
                retarget.apply();
                changed = true;
            }
            if bytes > 0 {
                return Err(io::Error::new(ErrorKind::Interrupted, "test stop"));
            }
            Ok(())
        });
        retarget.clear();
        assert!(result.is_err());
        assert_eq!(fs::read_dir(&outside).unwrap().count(), 1);
        assert_eq!(
            fs::read(outside.join("witness")).unwrap(),
            b"outside witness"
        );
        assert_eq!(fs::read_dir(&selected).unwrap().count(), 0);
        drop(retarget);
        fs::remove_dir(template).unwrap();
    }

    #[test]
    fn an_output_created_through_a_new_reparse_point_is_refused() {
        use crate::platform::windows_reparse_tests::{Retarget, junction};
        let root = tempfile::tempdir().unwrap();
        let selected = root.path().join("selected");
        let outside = root.path().join("outside");
        fs::create_dir(&selected).unwrap();
        fs::create_dir(&outside).unwrap();
        let template = root.path().join("template");
        junction(&template, &outside);
        let parents = HeldParents::new(&selected.join("output")).unwrap();
        let retarget = Retarget::new(&selected, &template);
        retarget.apply();
        // CopyFileEx creates by path: after the caller's check, the empty held
        // folder became a junction and the file lands in its target.
        let file = fs::File::create_new(selected.join("output")).unwrap();
        assert!(outside.join("output").exists());
        assert!(platform()
            .verify_created_child(parents.directory(), &file, &selected.join("output"))
            .is_err());
        drop(file);
        retarget.clear();
        fs::remove_file(outside.join("output")).unwrap();
        let file = fs::File::create_new(selected.join("output")).unwrap();
        platform()
            .verify_created_child(parents.directory(), &file, &selected.join("output"))
            .unwrap();
        drop((file, retarget, parents));
        fs::remove_dir(template).unwrap();
    }

    #[test]
    fn an_active_output_pins_its_folder_and_is_removed_after_cancellation() {
        use crate::platform::windows_reparse_tests::{Retarget, junction};
        let root = tempfile::tempdir().unwrap();
        let selected = root.path().join("selected");
        fs::create_dir(&selected).unwrap();
        let source = root.path().join("source");
        let destination = selected.join("destination");
        fs::write(&source, vec![0x42_u8; 3 * 1024 * 1024]).unwrap();
        let outside = root.path().join("outside");
        fs::create_dir(&outside).unwrap();
        fs::write(outside.join("witness"), b"unchanged").unwrap();
        let template = root.path().join("template");
        junction(&template, &outside);
        let mut checked = false;
        let error = copy(&source, &destination, false, &mut |bytes| {
            if bytes > 0 {
                // A folder holding a file cannot become a reparse point.
                let retarget = Retarget::new(&selected, &template);
                assert!(retarget.try_apply().is_err());
                checked = true;
                return Err(io::Error::new(ErrorKind::Interrupted, "test stop"));
            }
            Ok(())
        })
        .unwrap_err();
        assert!(checked);
        assert_eq!(error.kind(), ErrorKind::Interrupted);
        assert_eq!(fs::read(outside.join("witness")).unwrap(), b"unchanged");
        assert_eq!(fs::read_dir(&outside).unwrap().count(), 1);
        assert_eq!(fs::read_dir(&selected).unwrap().count(), 0);
        fs::remove_dir(template).unwrap();
    }

    #[test]
    fn copy_stays_in_the_selected_parent_after_junction_retargeting() {
        use crate::platform::windows_reparse_tests::{Retarget, junction};
        for replace in [false, true] {
            let root = tempfile::tempdir().unwrap();
            let source_directory = root.path().join("source-directory");
            fs::create_dir(&source_directory).unwrap();
            let source = source_directory.join("input");
            let selected = root.path().join("selected");
            let outside = root.path().join("outside");
            fs::create_dir(&selected).unwrap();
            fs::create_dir(&outside).unwrap();
            fs::write(&source, b"selected data").unwrap();
            fs::write(outside.join("witness"), b"outside witness").unwrap();
            fs::write(outside.join("input"), b"foreign source").unwrap();
            if replace {
                fs::write(selected.join("output"), b"old data").unwrap();
                fs::write(outside.join("output"), b"foreign output").unwrap();
            }
            let link = root.path().join("link");
            let template = root.path().join("template");
            let source_link = root.path().join("source-link");
            junction(&link, &selected);
            junction(&template, &outside);
            junction(&source_link, &source_directory);
            let retarget = Retarget::new(&link, &template);
            let source_retarget = Retarget::new(&source_link, &template);
            let mut changed = false;
            copy(
                &source_link.join("input"),
                &link.join("output"),
                replace,
                &mut |_| {
                    if !changed {
                        retarget.apply();
                        source_retarget.apply();
                        changed = true;
                    }
                    Ok(())
                },
            )
            .unwrap();
            assert_eq!(fs::read(selected.join("output")).unwrap(), b"selected data");
            assert_eq!(
                fs::read(outside.join("witness")).unwrap(),
                b"outside witness"
            );
            assert_eq!(fs::read(outside.join("input")).unwrap(), b"foreign source");
            assert_eq!(fs::read(&source).unwrap(), b"selected data");
            if replace {
                assert_eq!(fs::read(outside.join("output")).unwrap(), b"foreign output");
            } else {
                assert!(!outside.join("output").exists());
            }
            assert_eq!(
                fs::read_dir(&outside).unwrap().count(),
                if replace { 3 } else { 2 }
            );
            assert!(fs::read_dir(&selected).unwrap().all(|entry| {
                !entry.unwrap().file_name().to_string_lossy().contains(".oxyde-")
            }));
            drop((retarget, source_retarget));
            fs::remove_dir(link).unwrap();
            fs::remove_dir(template).unwrap();
            fs::remove_dir(source_link).unwrap();
        }
    }

    struct Fixture {
        root: tempfile::TempDir,
        source: PathBuf,
        destination: PathBuf,
    }
    impl Fixture {
        fn new() -> Self {
            let root = tempfile::tempdir().unwrap();
            let source = root.path().join("source");
            let destination = root.path().join("destination");
            fs::write(&source, vec![0x42_u8; 3 * 1024 * 1024]).unwrap();
            Self {
                root,
                source,
                destination,
            }
        }
        /// Neither a staging folder nor a replacement's temporary is left.
        fn no_temporary(&self) {
            assert!(fs::read_dir(self.root.path()).unwrap().all(|entry| {
                !entry.unwrap().file_name().to_string_lossy().contains(".oxyde-")
            }));
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            for path in [&self.source, &self.destination] {
                if let Ok(metadata) = fs::metadata(path) {
                    let mut permissions = metadata.permissions();
                    permissions.set_readonly(false);
                    fs::set_permissions(path, permissions).unwrap();
                }
            }
        }
    }

    #[test]
    fn copy_keeps_source_and_publication_parents_fixed_during_progress() {
        let root = tempfile::tempdir().unwrap();
        let source_parent = root.path().join("source-parent");
        let target_parent = root.path().join("target-parent");
        fs::create_dir(&source_parent).unwrap();
        fs::create_dir(&target_parent).unwrap();
        let source = source_parent.join("file");
        let destination = target_parent.join("file");
        fs::write(&source, vec![0x42; 256 * 1024]).unwrap();
        let mut checked = false;
        copy(&source, &destination, false, &mut |bytes| {
            if bytes > 0 && !checked {
                assert!(fs::rename(&source_parent, root.path().join("replaced-source")).is_err());
                assert!(fs::rename(&target_parent, root.path().join("replaced-target")).is_err());
                checked = true;
            }
            Ok(())
        })
        .unwrap();
        assert!(checked);
        assert_eq!(fs::read(&source).unwrap(), fs::read(&destination).unwrap());
        fs::rename(&target_parent, root.path().join("released-target")).unwrap();
    }

    #[test]
    fn cancellation_during_native_copy_removes_the_partial_output() {
        let fixture = Fixture::new();
        let error = copy(&fixture.source, &fixture.destination, false, &mut |bytes| {
            if bytes > 0 {
                return Err(io::Error::new(
                    ErrorKind::Interrupted,
                    "injected mid-copy cancellation",
                ));
            }
            Ok(())
        })
        .unwrap_err();
        assert_eq!(error.to_string(), "injected mid-copy cancellation");
        assert!(!fixture.destination.exists());
        assert_eq!(
            fs::metadata(&fixture.source).unwrap().len(),
            3 * 1024 * 1024
        );
        fixture.no_temporary();
    }

    #[test]
    fn cancellation_before_replacement_preserves_old_contents_and_removes_the_temporary() {
        let fixture = Fixture::new();
        fs::write(&fixture.destination, b"old contents").unwrap();
        let error = copy(&fixture.source, &fixture.destination, true, &mut |bytes| {
            if bytes > 0 {
                return Err(io::Error::new(
                    ErrorKind::Interrupted,
                    "injected overwrite stop",
                ));
            }
            Ok(())
        })
        .unwrap_err();
        assert_eq!(error.to_string(), "injected overwrite stop");
        assert_eq!(fs::read(&fixture.destination).unwrap(), b"old contents");
        assert!(fixture.source.is_file());
        fixture.no_temporary();
    }

    #[test]
    fn callback_panic_stops_native_copy_without_crossing_ffi() {
        let fixture = Fixture::new();
        let error = copy(&fixture.source, &fixture.destination, false, &mut |bytes| {
            if bytes > 0 {
                panic!("injected callback panic");
            }
            Ok(())
        })
        .unwrap_err();
        assert_eq!(error.to_string(), "Copy progress callback panicked");
        assert!(!fixture.destination.exists());
        fixture.no_temporary();
    }

    #[test]
    fn creation_conflict_keeps_a_foreign_file() {
        let fixture = Fixture::new();
        let mut permissions = fs::metadata(&fixture.source).unwrap().permissions();
        permissions.set_readonly(true);
        fs::set_permissions(&fixture.source, permissions).unwrap();
        let error = copy(&fixture.source, &fixture.destination, false, &mut |bytes| {
            if bytes == 0 {
                fs::write(&fixture.destination, b"foreign content")?;
            }
            Ok(())
        })
        .unwrap_err();
        assert_eq!(error.kind(), ErrorKind::AlreadyExists);
        assert_eq!(fs::read(&fixture.destination).unwrap(), b"foreign content");
        assert!(
            fs::metadata(&fixture.source)
                .unwrap()
                .permissions()
                .readonly()
        );
        fixture.no_temporary();
    }

    #[test]
    fn hidden_overwrite_copies_streams_and_preserves_monotonic_main_progress() {
        let fixture = Fixture::new();
        fs::write(
            fixture.source.with_file_name("source:extra"),
            b"stream data",
        )
        .unwrap();
        fs::write(&fixture.destination, b"old contents").unwrap();
        platform()
            .set_hidden_attribute(&fixture.source, true)
            .unwrap();
        platform()
            .set_hidden_attribute(&fixture.destination, true)
            .unwrap();
        let mut last = 0;
        let hash = copy(&fixture.source, &fixture.destination, true, &mut |bytes| {
            assert!(bytes >= last);
            last = bytes;
            Ok(())
        })
        .unwrap();
        assert_eq!(last, 3 * 1024 * 1024);
        assert_eq!(
            hash,
            blake3::hash(&fs::read(&fixture.source).unwrap())
                .to_hex()
                .to_string()
        );
        assert_eq!(
            fs::read(fixture.destination.with_file_name("destination:extra")).unwrap(),
            b"stream data"
        );
        assert!(
            platform()
                .file_attributes(&fs::metadata(&fixture.destination).unwrap(), "")
                .0
        );
        fixture.no_temporary();
    }

    #[test]
    fn empty_readonly_copy_is_published_with_attributes() {
        let fixture = Fixture::new();
        fs::write(&fixture.source, []).unwrap();
        let mut permissions = fs::metadata(&fixture.source).unwrap().permissions();
        permissions.set_readonly(true);
        fs::set_permissions(&fixture.source, permissions).unwrap();
        let hash = copy(
            &fixture.source,
            &fixture.destination,
            false,
            &mut |_| Ok(()),
        )
        .unwrap();
        assert_eq!(hash, blake3::hash(b"").to_hex().to_string());
        assert_eq!(fs::metadata(&fixture.destination).unwrap().len(), 0);
        assert!(
            fs::metadata(&fixture.destination)
                .unwrap()
                .permissions()
                .readonly()
        );
        fixture.no_temporary();
    }

    #[test]
    fn native_checkpoint_can_pause_and_source_writer_is_refused_until_resume() {
        let fixture = Fixture::new();
        let (paused_tx, paused_rx) = std::sync::mpsc::sync_channel(1);
        let (resume_tx, resume_rx) = std::sync::mpsc::sync_channel(1);
        std::thread::scope(|scope| {
            let fixture = &fixture;
            let worker = scope.spawn(move || {
                let mut paused = false;
                copy(&fixture.source, &fixture.destination, false, &mut |bytes| {
                    if bytes > 0 && !paused {
                        paused = true;
                        paused_tx.send(()).unwrap();
                        resume_rx
                            .recv_timeout(std::time::Duration::from_secs(5))
                            .map_err(io::Error::other)?;
                    }
                    Ok(())
                })
            });
            paused_rx
                .recv_timeout(std::time::Duration::from_secs(5))
                .unwrap();
            // Written under its own name, held against other writers.
            assert!(fixture.destination.exists());
            assert!(fs::File::options().write(true).open(&fixture.destination).is_err());
            assert!(
                fs::File::options()
                    .write(true)
                    .open(&fixture.source)
                    .is_err()
            );
            resume_tx.send(()).unwrap();
            worker.join().unwrap().unwrap();
        });
        assert_eq!(
            fs::read(&fixture.destination).unwrap(),
            fs::read(&fixture.source).unwrap()
        );
        fixture.no_temporary();
    }

    #[test]
    fn a_replacement_whose_target_is_gone_creates_the_file() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source.txt");
        let destination = root.path().join("removed meanwhile.txt");
        fs::write(&source, b"new contents").unwrap();
        copy(&source, &destination, true, &mut |_| Ok(())).unwrap();
        assert_eq!(fs::read(&destination).unwrap(), b"new contents");
    }
