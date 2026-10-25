use super::*;
use std::io::Read;
use crate::models::HistoryManager;

struct Fixture {
    root: tempfile::TempDir,
    originals: Vec<PathBuf>,
    physical: Vec<PathBuf>,
}
impl Fixture {
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let originals: Vec<_> = [
            "Rapport école 🦀.txt",
            "Raccourci école.lnk",
            "Lien école.url",
            "Dossier école",
            "Dossier vide",
        ]
        .iter()
        .map(|name| root.path().join(name))
        .collect();
        for source in &originals[..3] {
            std::fs::write(source, b"trash transfer fixture").unwrap();
        }
        for source in &originals[3..] {
            std::fs::create_dir(source).unwrap();
        }
        std::fs::write(originals[3].join("contenu.txt"), b"nested fixture").unwrap();
        platform().shell_delete(&originals, true).unwrap();
        let listed = crate::commands::ops::list_trash().unwrap();
        let physical = originals
            .iter()
            .map(|source| {
                PathBuf::from(
                    &listed
                        .iter()
                        .find(|entry| std::path::Path::new(&entry.original_path) == *source)
                        .unwrap()
                        .path,
                )
            })
            .collect();
        Self {
            root,
            originals,
            physical,
        }
    }
    fn target(&self, name: &str) -> PathBuf {
        let path = self.root.path().join(name);
        std::fs::create_dir(&path).unwrap();
        path
    }
    fn assert_payload(&self, target: &Path) {
        for source in &self.originals[..3] {
            assert_eq!(
                std::fs::read(target.join(source.file_name().unwrap())).unwrap(),
                b"trash transfer fixture"
            );
        }
        assert_eq!(
            std::fs::read(
                target
                    .join(self.originals[3].file_name().unwrap())
                    .join("contenu.txt")
            )
            .unwrap(),
            b"nested fixture"
        );
        assert!(target.join(self.originals[4].file_name().unwrap()).is_dir());
        assert_eq!(
            std::fs::read_dir(target).unwrap().count(),
            self.originals.len()
        );
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        // Restore only generated entries still present, including on assertion failure.
        if let Ok(listed) = crate::commands::ops::list_trash() {
            let ids: Vec<_> = listed
                .into_iter()
                .filter(|entry| {
                    self.originals
                        .contains(&PathBuf::from(&entry.original_path))
                })
                .map(|entry| entry.path)
                .collect();
            if !ids.is_empty() {
                let result = std::thread::spawn(move || {
                    tokio::runtime::Runtime::new()
                        .unwrap()
                        .block_on(crate::commands::ops::restore_items(ids))
                })
                .join();
                if !matches!(result, Ok(Ok(_))) {
                    eprintln!("Generated trash fixture cleanup failed: {result:?}");
                }
            }
        }
    }
}
async fn queued(
    history: Arc<HistoryManager>,
    sources: Vec<PathBuf>,
    target: PathBuf,
    move_files: bool,
    resolutions: Option<HashMap<String, ConflictAction>>,
) -> OpStatus {
    let manager = FileOperationManager::new();
    let id = manager.queue_operation_on(
        &tokio::runtime::Handle::current(),
        history,
        FileOperation::new(
            if move_files {
                FileOpType::Move
            } else {
                FileOpType::Copy
            },
            sources,
            Some(target),
            resolutions,
        ),
    );
    tokio::time::timeout(std::time::Duration::from_secs(20), async {
        loop {
            let status = manager.get_operation(&id).unwrap().status;
            if FileOperationManager::is_final(&status) {
                return status;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap()
}

#[test]
#[ignore = "copies/moves only generated native trash fixtures through the GPUI operation queue"]
fn native_trash_transfer_names_metadata_and_history_roundtrip() {
    let fixture = Fixture::new();
    let target = fixture.target("output");
    let runtime = tokio::runtime::Runtime::new().unwrap();
    runtime.block_on(async {
        let history = Arc::new(HistoryManager::new());
        assert_eq!(
            queued(
                history.clone(),
                fixture.physical.clone(),
                target.clone(),
                false,
                None
            )
            .await,
            OpStatus::Completed
        );
        fixture.assert_payload(&target);
        assert_eq!(
            platform()
                .trash_transfer_entries(&fixture.physical)
                .unwrap()
                .len(),
            fixture.physical.len()
        );
        let conflicts = crate::commands::ops::check_conflicts(
            fixture
                .physical
                .iter()
                .map(|path| path.to_string_lossy().into_owned())
                .collect(),
            target.to_string_lossy().into_owned(),
        )
        .await
        .unwrap();
        for source in &fixture.originals {
            let name = source.file_name().unwrap().to_string_lossy();
            assert!(conflicts
                .iter()
                .any(|conflict| conflict.name == name && conflict.source.name == name));
        }
        crate::commands::ops::undo_last_action(&history)
            .await
            .unwrap();
        assert_eq!(std::fs::read_dir(&target).unwrap().count(), 0);
        crate::commands::ops::redo_last_action(&history)
            .await
            .unwrap();
        fixture.assert_payload(&target);
        crate::commands::ops::undo_last_action(&history)
            .await
            .unwrap();
        history.clear();
        assert_eq!(
            queued(
                history.clone(),
                fixture.physical.clone(),
                target.clone(),
                true,
                None
            )
            .await,
            OpStatus::Completed
        );
        fixture.assert_payload(&target);
        let backups = history.get_state().undo_stack[0]
            .details
            .trash_metadata_backups
            .clone()
            .unwrap();
        assert_eq!(backups.len(), fixture.originals.len());
        let saved = PathBuf::from(&backups[0]).with_extension("fixture-saved");
        std::fs::rename(&backups[0], &saved).unwrap();
        assert!(crate::commands::ops::undo_last_action(&history)
            .await
            .is_err());
        fixture.assert_payload(&target);
        assert!(fixture.physical.iter().all(|path| !path.exists()));
        std::fs::rename(&saved, &backups[0]).unwrap();
        for _ in 0..2 {
            assert!(fixture.physical.iter().all(|path| !path.exists()));
            assert!(backups.iter().all(|path| std::path::Path::new(path).exists()));
            crate::commands::ops::undo_last_action(&history)
                .await
                .unwrap();
            assert_eq!(
                platform()
                    .trash_transfer_entries(&fixture.physical)
                    .unwrap()
                    .len(),
                fixture.physical.len()
            );
            assert!(backups.iter().all(|path| !Path::new(path).exists()));
            assert_eq!(std::fs::read_dir(&target).unwrap().count(), 0);
            let blocker = target.join(fixture.originals[0].file_name().unwrap());
            std::fs::write(&blocker, b"redo conflict fixture").unwrap();
            assert!(crate::commands::ops::redo_last_action(&history)
                .await
                .is_err());
            assert_eq!(std::fs::read(&blocker).unwrap(), b"redo conflict fixture");
            assert!(fixture.physical.iter().all(|path| path.exists()));
            std::fs::remove_file(blocker).unwrap();
            crate::commands::ops::redo_last_action(&history)
                .await
                .unwrap();
            fixture.assert_payload(&target);
        }
        history.clear();
        assert!(backups.iter().all(|path| !Path::new(path).exists()));
    });
}

#[test]
#[ignore = "checks conflicts and partial moves using only generated native trash fixtures"]
fn native_trash_transfer_conflicts_preserve_skipped_metadata() {
    let fixture = Fixture::new();
    let target = fixture.target("output");
    let original_name = fixture.originals[0].file_name().unwrap();
    std::fs::write(target.join(original_name), b"existing fixture").unwrap();
    let folder = target.join(fixture.originals[3].file_name().unwrap());
    std::fs::create_dir(&folder).unwrap();
    std::fs::write(folder.join("contenu.txt"), b"existing nested fixture").unwrap();
    std::fs::write(
        fixture.physical[3].join("nouveau.txt"),
        b"new nested fixture",
    )
    .unwrap();
    let runtime = tokio::runtime::Runtime::new().unwrap();
    runtime.block_on(async {
        let history = Arc::new(HistoryManager::new());
        let sources = vec![fixture.physical[0].clone(), fixture.physical[3].clone()];
        // Queue revalidation uses the logical destination name too.
        assert!(matches!(
            queued(history.clone(), sources.clone(), target.clone(), true, None).await,
            OpStatus::Error(_)
        ));
        assert!(history.get_state().undo_stack.is_empty());
        let resolutions = HashMap::from([
            (
                fixture.physical[0].to_string_lossy().into_owned(),
                ConflictAction::Skip,
            ),
            (
                fixture.physical[3]
                    .join("contenu.txt")
                    .to_string_lossy()
                    .into_owned(),
                ConflictAction::Skip,
            ),
            (
                fixture.physical[3].to_string_lossy().into_owned(),
                ConflictAction::Replace,
            ),
        ]);
        assert_eq!(
            queued(
                history.clone(),
                sources.clone(),
                target.clone(),
                true,
                Some(resolutions)
            )
            .await,
            OpStatus::Completed
        );
        assert_eq!(
            std::fs::read(target.join(original_name)).unwrap(),
            b"existing fixture"
        );
        assert_eq!(
            std::fs::read(folder.join("contenu.txt")).unwrap(),
            b"existing nested fixture"
        );
        assert_eq!(
            std::fs::read(folder.join("nouveau.txt")).unwrap(),
            b"new nested fixture"
        );
        assert_eq!(
            platform().trash_transfer_entries(&sources).unwrap().len(),
            sources.len()
        );
        assert!(history.get_state().undo_stack[0]
            .details
            .trash_metadata_backups
            .as_ref()
            .unwrap()
            .is_empty());
        crate::commands::ops::undo_last_action(&history)
            .await
            .unwrap();
        assert!(fixture.physical[3].join("nouveau.txt").exists());
        assert!(!folder.join("nouveau.txt").exists());
    });
}
#[test]
#[ignore = "transfers only generated native trash fixtures into a temporary archive"]
fn native_trash_transfer_into_archive_uses_original_names() {
    let fixture = Fixture::new();
    let archive = fixture.root.path().join("output.zip");
    let file = std::fs::File::create(&archive).unwrap();
    let mut writer = zip::ZipWriter::new(file);
    writer
        .add_directory("contents/", zip::write::SimpleFileOptions::default())
        .unwrap();
    writer.finish().unwrap();
    let target = archive.join("contents");
    for move_files in [false, true] {
        let op = Arc::new(Mutex::new(FileOperation::new(
            if move_files {
                FileOpType::Move
            } else {
                FileOpType::Copy
            },
            fixture.physical.clone(),
            Some(target.clone()),
            None,
        )));
        let resolutions = if move_files {
            Some(
                fixture
                    .physical
                    .iter()
                    .map(|path| (path.to_string_lossy().into_owned(), ConflictAction::Replace))
                    .collect(),
            )
        } else {
            None
        };
        FileOperationManager::perform_copy(
            &op,
            fixture.physical.clone(),
            Some(target.clone()),
            move_files,
            resolutions,
        )
        .unwrap();
        let mut zip = zip::ZipArchive::new(std::fs::File::open(&archive).unwrap()).unwrap();
        for source in &fixture.originals[..3] {
            let mut content = Vec::new();
            zip.by_name(&format!(
                "contents/{}",
                source.file_name().unwrap().to_string_lossy()
            ))
            .unwrap()
            .read_to_end(&mut content)
            .unwrap();
            assert_eq!(content, b"trash transfer fixture");
        }
        assert!(fixture
            .physical
            .iter()
            .all(|path| path.exists() != move_files));
        assert!(op.lock().unwrap().trash_metadata_backups.is_empty());
    }
}
#[test]
#[ignore = "checks Delete Undo/Redo with only generated native trash fixtures"]
fn native_delete_undo_redo_preserves_original_names() {
    let fixture = Fixture::new();
    let runtime = tokio::runtime::Runtime::new().unwrap();
    runtime.block_on(async {
        crate::commands::ops::restore_items(
            fixture
                .physical
                .iter()
                .map(|path| path.to_string_lossy().into_owned())
                .collect(),
        )
        .await
        .unwrap();
        let history = Arc::new(HistoryManager::new());
        let operation = Arc::new(Mutex::new(FileOperation::new(
            FileOpType::Trash,
            fixture.originals.clone(),
            None,
            None,
        )));
        FileOperationManager::execute_operation(history.clone(), operation.clone()).await;
        assert_eq!(operation.lock().unwrap().status, OpStatus::Completed);
        for _ in 0..2 {
            assert!(fixture.originals.iter().all(|path| !path.exists()));
            crate::commands::ops::undo_last_action(&history)
                .await
                .unwrap();
            fixture.assert_payload(fixture.root.path());
            assert_eq!(history.get_state().redo_stack.len(), 1);
            crate::commands::ops::redo_last_action(&history)
                .await
                .unwrap();
            assert!(fixture.originals.iter().all(|path| !path.exists()));
            assert_eq!(history.get_state().undo_stack.len(), 1);
        }
        crate::commands::ops::undo_last_action(&history)
            .await
            .unwrap();
        fixture.assert_payload(fixture.root.path());
    });
}
