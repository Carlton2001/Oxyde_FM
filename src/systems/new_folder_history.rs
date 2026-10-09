//! Create and undo a single empty folder without adopting a substituted name.
use super::path_parents::HeldParents;
use crate::models::error::{NewFolderHistoryFailure, NewFolderHistoryIssue as Issue};
use crate::models::transaction::HistoryFileIdentity;
use crate::models::{CommandError, TransactionDetails};
use crate::platform::platform;
use std::{io, path::Path};

fn failure(issue: Issue, path: &Path) -> CommandError {
    CommandError::NewFolderHistory(NewFolderHistoryFailure {
        issue,
        path: path.to_string_lossy().into_owned(),
    })
}

pub(crate) fn create(path: &Path) -> io::Result<Option<HistoryFileIdentity>> {
    let parents = HeldParents::new(path)?;
    let resolved = parents.resolved_path(path)?;
    let file = platform().create_journal_directory(parents.directory(), &resolved, false)?;
    // Creation already succeeded. A missing proof disables Undo, not the creation.
    Ok(HistoryFileIdentity::from_file(&file)
        .map_err(|error| {
            log::warn!(
                "New folder identity unavailable for {}: {error}",
                path.display()
            )
        })
        .ok())
}

fn single_path(details: &TransactionDetails) -> Result<&Path, CommandError> {
    if details.paths.len() != 1 {
        return Err(failure(
            Issue::Unverified,
            Path::new(
                details
                    .paths
                    .first()
                    .map(String::as_str)
                    .unwrap_or_default(),
            ),
        ));
    }
    Ok(Path::new(&details.paths[0]))
}

pub(crate) fn undo(details: &TransactionDetails) -> Result<(), CommandError> {
    undo_with_checkpoint(details, &mut || Ok(()))
}

fn undo_with_checkpoint(
    details: &TransactionDetails,
    checkpoint: &mut dyn FnMut() -> io::Result<()>,
) -> Result<(), CommandError> {
    let path = single_path(details)?;
    let expected = details
        .new_folder_identity
        .ok_or_else(|| failure(Issue::Unverified, path))?;
    let parents = HeldParents::new(path).map_err(|error| {
        if error.kind() == io::ErrorKind::NotFound {
            failure(Issue::Missing, path)
        } else {
            CommandError::from(error)
        }
    })?;
    let resolved = parents.resolved_path(path)?;
    let file = platform()
        .open_rename_item(parents.directory(), &resolved)
        .map_err(|error| {
            if error.kind() == io::ErrorKind::NotFound {
                failure(Issue::Missing, path)
            } else {
                CommandError::from(error)
            }
        })?;
    let metadata = file.metadata()?;
    if !metadata.is_dir()
        || platform().file_attributes(&metadata, "").2
        || !expected.matches(&file)?
    {
        return Err(failure(Issue::Changed, path));
    }
    checkpoint()?;
    platform()
        .remove_open_empty_directory(&file, &resolved)
        .map_err(|error| match error.kind() {
            io::ErrorKind::DirectoryNotEmpty => failure(Issue::NotEmpty, path),
            io::ErrorKind::NotFound => failure(Issue::Missing, path),
            _ => CommandError::from(error),
        })
}

pub(crate) fn redo(details: &mut TransactionDetails) -> Result<(), CommandError> {
    let path = single_path(details)?.to_path_buf();
    let identity = create(&path).map_err(|error| match error.kind() {
        io::ErrorKind::AlreadyExists => failure(Issue::Conflict, &path),
        io::ErrorKind::NotFound => failure(Issue::ParentMissing, &path),
        _ => CommandError::from(error),
    })?;
    details.new_folder_identity = identity;
    Ok(())
}

#[cfg(all(test, target_os = "windows"))]
mod tests {
    use super::*;

    fn details(path: &Path) -> TransactionDetails {
        TransactionDetails {
            new_folder_identity: create(path).unwrap(),
            moved_file_identities: None,
            paths: vec![path.to_string_lossy().into_owned()],
            target_dir: None,
            old_path: None,
            new_path: None,
            created_files: None,
            created_dirs: None,
            overwritten_files: None,
            moved_files: None,
            trash_metadata_backups: None,
            moved_dirs: None,
            copied_files: None,
            copied_file_hashes: None,
            copied_file_identities: None,
            copied_directory_identities: None,
        }
    }

    #[test]
    fn new_folder_undo_pins_its_object_and_preserves_a_child_arriving_before_deletion() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("created");
        let journal = details(&path);
        let error = undo_with_checkpoint(&journal, &mut || {
            assert!(std::fs::rename(&path, root.path().join("displaced")).is_err());
            std::fs::write(path.join("late.txt"), b"late contents")
        })
        .unwrap_err();
        assert!(
            matches!(error, CommandError::NewFolderHistory(failure) if failure.issue == Issue::NotEmpty)
        );
        assert_eq!(
            std::fs::read(path.join("late.txt")).unwrap(),
            b"late contents"
        );
        assert!(!root.path().join("displaced").exists());
    }

    #[test]
    fn new_folder_undo_preserves_a_reparse_replacement_and_stays_in_its_bound_parent() {
        use crate::platform::windows_reparse_tests::{Retarget, junction};
        let root = tempfile::tempdir().unwrap();
        let selected = root.path().join("selected");
        let outside = root.path().join("outside");
        std::fs::create_dir(&selected).unwrap();
        std::fs::create_dir(&outside).unwrap();
        let link = root.path().join("link");
        let template = root.path().join("template");
        junction(&link, &selected);
        junction(&template, &outside);
        let path = link.join("created");
        let journal = details(&path);
        std::fs::create_dir(outside.join("created")).unwrap();
        std::fs::write(outside.join("created/witness"), b"outside witness").unwrap();
        let retarget = Retarget::new(&link, &template);
        undo_with_checkpoint(&journal, &mut || {
            retarget.apply();
            Ok(())
        })
        .unwrap();
        assert!(!selected.join("created").exists());
        assert_eq!(
            std::fs::read(outside.join("created/witness")).unwrap(),
            b"outside witness"
        );
        drop(retarget);
        std::fs::remove_dir(&link).unwrap();
        std::fs::remove_dir(&template).unwrap();
        let path = selected.join("another");
        let journal = details(&path);
        std::fs::rename(&path, selected.join("original")).unwrap();
        junction(&path, &outside);
        let error = undo(&journal).unwrap_err();
        assert!(
            matches!(error, CommandError::NewFolderHistory(failure) if failure.issue == Issue::Changed)
        );
        assert!(path.is_dir());
        assert_eq!(
            std::fs::read(outside.join("created/witness")).unwrap(),
            b"outside witness"
        );
        std::fs::remove_dir(&path).unwrap();
    }
}
