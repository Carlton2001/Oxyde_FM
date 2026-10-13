use super::*;
use oxyde_lib::utils::archive_mutation_test::{with_hook_for_tests, Stage};

fn fixture(root: &Path) -> Vec<PathBuf> {
    let paths = ["source-a.zip", "source-b.zip", "target.zip"].map(|name| root.join(name));
    zip(&paths[0], "first", b"one", false);
    zip(&paths[1], "second", b"two", false);
    zip(&paths[2], "existing", b"old target", false);
    paths.to_vec()
}
fn transfer(paths: &[PathBuf], op: &Arc<Mutex<FileOperation>>) -> Result<(), String> {
    FileOperationManager::perform_copy(
        op,
        vec![paths[0].join("first"), paths[1].join("second")],
        Some(paths[2].clone()),
        true,
        None,
    )
}

#[test]
fn grouped_publication_restores_every_original_on_late_failure_and_between_publications_cancellation(
) {
    native(|| {
        for cancellation in [false, true] {
            let root = tempfile::tempdir().unwrap();
            let paths = fixture(root.path());
            let before: Vec<_> = paths
                .iter()
                .map(fs::read)
                .collect::<Result<_, _>>()
                .unwrap();
            let op = operation(FileOpType::Move);
            let cancel = op.lock().unwrap().cancel_flag.clone();
            let result = with_hook_for_tests(
                move |stage, archive, _| {
                    if stage == Stage::AfterBackup {
                        if cancellation
                            && archive.file_name().is_some_and(|name| name == "target.zip")
                        {
                            cancel.store(true, Ordering::Release);
                        }
                        if !cancellation
                            && archive
                                .file_name()
                                .is_some_and(|name| name == "source-b.zip")
                        {
                            return Err(std::io::Error::other("injected late publication failure"));
                        }
                    }
                    Ok(())
                },
                || transfer(&paths, &op),
            );
            let error = result.unwrap_err();
            assert!(
                oxyde_lib::models::error::ArchiveRecoveryFailure::from_message(&error).is_none(),
                "{error}"
            );
            for (path, expected) in paths.iter().zip(before) {
                assert_eq!(fs::read(path).unwrap(), expected);
            }
            assert!(fs::read_dir(root.path()).unwrap().all(|item| !item
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with(".oxyde-")));
        }
    });
}

#[test]
fn failed_group_rollback_preserves_foreign_replacements_and_reports_original_backup() {
    native(|| {
        let root = tempfile::tempdir().unwrap();
        let paths = fixture(root.path());
        let before: Vec<_> = paths
            .iter()
            .map(fs::read)
            .collect::<Result<_, _>>()
            .unwrap();
        let backup = Arc::new(Mutex::new(None::<PathBuf>));
        let witness = backup.clone();
        let result = with_hook_for_tests(
            move |stage, archive, saved| {
                if stage == Stage::AfterBackup
                    && archive
                        .file_name()
                        .is_some_and(|name| name == "source-b.zip")
                {
                    *witness.lock().unwrap() = Some(saved.to_path_buf());
                    fs::write(archive, b"foreign replacement")?;
                    return Err(std::io::Error::other("injected late failure"));
                }
                Ok(())
            },
            || transfer(&paths, &operation(FileOpType::Move)),
        );
        let report =
            oxyde_lib::models::error::ArchiveRecoveryFailure::from_message(&result.unwrap_err())
                .unwrap();
        assert!(!report.completed);
        let backup = backup.lock().unwrap().clone().unwrap();
        assert!(report
            .locations
            .iter()
            .any(|path| Path::new(path) == backup));
        assert_eq!(fs::read(backup).unwrap(), before[1]);
        assert_eq!(fs::read(&paths[1]).unwrap(), b"foreign replacement");
        assert_eq!(fs::read(&paths[0]).unwrap(), before[0]);
        assert_eq!(fs::read(&paths[2]).unwrap(), before[2]);
    });
}
