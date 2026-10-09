//! Move Redo directory effects remain bound through success, rollback and unwinding.
use super::history_directories::{Directories, Removal};
use crate::models::error::{HistoryMoveFailure, HistoryMoveIssue as Issue};
use crate::models::{CommandError, TransactionDetails};
use crate::platform::platform;
use std::{
    collections::HashMap,
    fs, io,
    path::{Path, PathBuf},
};

fn failure(issue: Issue, detail: impl Into<String>) -> io::Error {
    io::Error::other(HistoryMoveFailure {
        issue,
        detail: detail.into(),
    })
}
fn at(error: io::Error, path: &Path) -> io::Error {
    if error.kind() == io::ErrorKind::AlreadyExists {
        failure(Issue::Conflict, path.to_string_lossy())
    } else {
        error
    }
}

#[derive(Clone, Copy)]
enum Phase {
    Create,
    Move,
    Remove,
}
trait Checkpoint {
    fn before(&mut self, _phase: Phase, _path: &Path) -> io::Result<()> {
        Ok(())
    }
}
struct Native;
impl Checkpoint for Native {}

fn transfer_file(
    from: &Path,
    to: &Path,
    expected: Option<&str>,
    identity: Option<&crate::models::transaction::HistoryMoveIdentity>,
    source: &super::path_parents::HeldParents,
    destination: &super::path_parents::HeldParents,
) -> io::Result<String> {
    if let Some(identity) = identity {
        super::history_move::move_file_with_journal_identity(
            from,
            to,
            expected,
            identity,
            source,
            destination,
        )
    } else {
        // Only recovery sidecars reach this branch after plan validation.
        super::history_move::move_file_without_identity(from, to, expected, source, destination)
    }
}

enum Effect {
    Created(PathBuf),
    Moved {
        from: PathBuf,
        to: PathBuf,
        hash: String,
        identity: Option<crate::models::transaction::HistoryMoveIdentity>,
    },
    Removed {
        path: PathBuf,
        permissions: fs::Permissions,
        hidden: bool,
    },
}
struct Journal {
    directory_identities: HashMap<PathBuf, crate::models::transaction::HistoryDirectoryIdentity>,
    directories: Directories,
    effects: Vec<Effect>,
    finished: bool,
}
impl Journal {
    fn ensure(&mut self, path: &Path, checkpoint: &mut impl Checkpoint) -> io::Result<()> {
        let mut missing = Vec::new();
        let mut ancestor = path;
        loop {
            match self.directories.directory(ancestor) {
                Ok(_) => {
                    if let Some(expected) = self.directory_identities.get(ancestor) {
                        self.directories
                            .verify_history_identity(ancestor, expected)?;
                    }
                    break;
                }
                Err(error) if error.kind() == io::ErrorKind::NotFound => {
                    missing.push(ancestor.to_path_buf());
                    ancestor = ancestor
                        .parent()
                        .ok_or_else(|| io::Error::other("Unbound move Redo parent"))?;
                }
                Err(error) => return Err(error),
            }
        }
        for path in missing.into_iter().rev() {
            self.create(&path, checkpoint)?;
        }
        Ok(())
    }
    fn create(&mut self, path: &Path, checkpoint: &mut impl Checkpoint) -> io::Result<()> {
        checkpoint.before(Phase::Create, path)?;
        self.directories
            .create(path, false)
            .map_err(|error| at(error, path))?;
        self.effects.push(Effect::Created(path.to_path_buf()));
        if let Some(expected) = self.directory_identities.get(path) {
            expected.refresh_from_file(self.directories.directory(path)?)?;
        }
        Ok(())
    }
    fn transfer(
        &mut self,
        from: &Path,
        to: &Path,
        expected: Option<&str>,
        identity: Option<&crate::models::transaction::HistoryMoveIdentity>,
        checkpoint: &mut impl Checkpoint,
    ) -> io::Result<()> {
        let source = self.directories.file_parents(from)?;
        let destination = self.directories.file_parents(to)?;
        checkpoint.before(Phase::Move, from)?;
        let hash = transfer_file(from, to, expected, identity, &source, &destination)
            .map_err(|error| at(error, to))?;
        self.effects.push(Effect::Moved {
            from: from.to_path_buf(),
            to: to.to_path_buf(),
            hash,
            identity: identity.cloned(),
        });
        Ok(())
    }
    fn remove(&mut self, path: &Path, checkpoint: &mut impl Checkpoint) -> io::Result<()> {
        let metadata = self.directories.metadata(path)?;
        checkpoint.before(Phase::Remove, path)?;
        if let Some(expected) = self.directory_identities.get(path) {
            self.directories.verify_history_identity(path, expected)?;
        }
        if let (Some(metadata), Removal::Deleted) = (metadata, self.directories.remove(path)?) {
            self.effects.push(Effect::Removed {
                path: path.to_path_buf(),
                permissions: metadata.permissions(),
                hidden: platform().file_attributes(&metadata, "").0,
            });
        }
        Ok(())
    }
    fn rollback(&mut self) -> Vec<String> {
        let mut errors = Vec::new();
        let mut blocked_parents: Vec<PathBuf> = Vec::new();
        while let Some(effect) = self.effects.pop() {
            let (path, result) = match effect {
                Effect::Created(path) => {
                    let result = self.directories.remove(&path).and_then(|removal| {
                        if matches!(removal, Removal::NotEmpty) {
                            Err(io::Error::other("Created move Redo directory contains recovery or independent content"))
                        } else { Ok(()) }
                    });
                    (path, result)
                }
                Effect::Moved {
                    from,
                    to,
                    hash,
                    identity,
                } => {
                    let result = (|| {
                        if blocked_parents
                            .iter()
                            .any(|parent| from.starts_with(parent))
                        {
                            return Err(io::Error::other(
                                "Source parent could not be restored; published file retained for recovery",
                            ));
                        }
                        let source = self.directories.file_parents(&to)?;
                        let destination = self.directories.file_parents(&from)?;
                        transfer_file(
                            &to,
                            &from,
                            Some(&hash),
                            identity.as_ref(),
                            &source,
                            &destination,
                        )
                        .map(|_| ())
                    })();
                    (to, result)
                }
                Effect::Removed {
                    path,
                    permissions,
                    hidden,
                } => {
                    let result = (|| {
                        if blocked_parents
                            .iter()
                            .any(|parent| path.starts_with(parent))
                        {
                            return Err(io::Error::other(
                                "Ancestor could not be restored; replacement preserved",
                            ));
                        }
                        let directory = self.directories.create(&path, hidden)?;
                        if let Some(expected) = self.directory_identities.get(&path) {
                            expected.refresh_from_file(directory)?;
                        }
                        platform().restore_journal_directory(directory, &path, permissions, hidden)
                    })();
                    if result.is_err() {
                        blocked_parents.push(path.clone());
                    }
                    (path, result)
                }
            };
            if let Err(error) = result {
                errors.push(format!("{}: {error}", path.display()));
            }
        }
        self.finished = true;
        errors
    }
}
impl Drop for Journal {
    fn drop(&mut self) {
        if !self.finished {
            for error in self.rollback() {
                log::error!("Move Redo unwind rollback: {error}");
            }
        }
    }
}

pub(crate) fn redo(details: &TransactionDetails) -> Result<(), CommandError> {
    redo_with(details, &mut Native).map_err(|error| match CommandError::from(error) {
        CommandError::HistoryIdentity(failure) => CommandError::HistoryMove(HistoryMoveFailure {
            issue: match failure.issue {
                crate::models::error::HistoryIdentityIssue::Changed => Issue::Changed,
                crate::models::error::HistoryIdentityIssue::Unverified => Issue::Unverified,
            },
            detail: failure.path,
        }),
        other => other,
    })
}
fn redo_with(details: &TransactionDetails, checkpoint: &mut impl Checkpoint) -> io::Result<()> {
    let (Some(moved), Some(moved_dirs), Some(hashes)) = (
        &details.moved_files,
        &details.moved_dirs,
        &details.copied_file_hashes,
    ) else {
        return Err(failure(
            Issue::Unverified,
            details.paths.first().cloned().unwrap_or_default(),
        ));
    };
    if details
        .overwritten_files
        .as_ref()
        .is_some_and(|files| !files.is_empty())
    {
        return Err(failure(
            Issue::Unverified,
            details.paths.first().cloned().unwrap_or_default(),
        ));
    }
    let hashes: HashMap<_, _> = hashes
        .iter()
        .map(|(path, hash)| (PathBuf::from(path), hash.as_str()))
        .collect();
    let identities: HashMap<_, _> = details
        .moved_file_identities
        .as_deref()
        .unwrap_or_default()
        .iter()
        .map(|(path, identity)| (PathBuf::from(path), identity))
        .collect();
    let mut created: Vec<_> = details
        .created_dirs
        .as_deref()
        .unwrap_or_default()
        .iter()
        .map(PathBuf::from)
        .collect();
    created.sort_by_key(|path| path.components().count());
    created.dedup();
    let mut removed: Vec<_> = moved_dirs
        .iter()
        .map(|(source, _)| PathBuf::from(source))
        .collect();
    removed.sort_by_key(|path| std::cmp::Reverse(path.components().count()));
    removed.dedup();
    let mut directory_identities: HashMap<_, _> = details
        .copied_directory_identities
        .as_deref()
        .unwrap_or_default()
        .iter()
        .map(|(path, identity)| (PathBuf::from(path), identity.clone()))
        .collect();
    for path in &created {
        if !directory_identities.contains_key(path) {
            return Err(super::history_move::move_identity_failure(path, false));
        }
    }
    let restored: HashMap<_, _> = details
        .moved_directory_identities
        .as_deref()
        .unwrap_or_default()
        .iter()
        .map(|(path, identity)| (PathBuf::from(path), identity.clone()))
        .collect();
    for path in &removed {
        let identity = restored
            .get(path)
            .filter(|proof| proof.snapshot().is_some())
            .ok_or_else(|| super::history_move::move_identity_failure(path, false))?;
        directory_identities.insert(path.clone(), identity.clone());
    }
    let mut paths = created.clone();
    for (source, destination) in moved.iter().chain(moved_dirs) {
        for path in [source, destination] {
            let path = Path::new(path);
            let parent = path
                .parent()
                .ok_or_else(|| io::Error::other("Move Redo path has no parent"))?;
            paths.push(parent.to_path_buf());
        }
    }
    let directories = Directories::prepare(&paths, &removed)?;
    let mut journal = Journal {
        directory_identities,
        directories,
        effects: Vec::new(),
        finished: false,
    };
    let result = (|| {
        // Bind restored source directories before moving any of their files.
        for path in removed.iter().rev() {
            journal.directories.directory(path).map_err(|error| {
                if error.kind() == io::ErrorKind::NotFound {
                    super::history_move::move_identity_failure(path, true)
                } else {
                    error
                }
            })?;
            journal
                .directories
                .verify_history_identity(path, &journal.directory_identities[path])?;
        }
        // Validate all existing sources and destinations before the first effect.
        for (source, destination) in moved {
            let source = Path::new(source);
            let destination = Path::new(destination);
            let parents = journal.directories.file_parents(source)?;
            let mut file = parents.open_file(source)?;
            let sidecar = details
                .trash_metadata_backups
                .as_deref()
                .unwrap_or_default()
                .iter()
                .any(|path| Path::new(path) == destination);
            if !sidecar {
                let identity = identities
                    .get(destination)
                    .and_then(|proof| proof.snapshot())
                    .ok_or_else(|| super::history_move::move_identity_failure(source, false))?;
                if !identity.matches(&file)? {
                    return Err(super::history_move::move_identity_failure(source, true));
                }
            }
            if let Some(expected) = hashes.get(destination) {
                if *expected != super::history_move::hash(&mut file)? {
                    return Err(failure(Issue::Changed, source.to_string_lossy()));
                }
            }
            match journal.directories.file_parents(destination) {
                Ok(parents) => {
                    let resolved = parents.resolved_path(destination)?;
                    match fs::symlink_metadata(resolved) {
                        Ok(_) => {
                            return Err(failure(Issue::Conflict, destination.to_string_lossy()));
                        }
                        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                        Err(error) => return Err(error),
                    }
                }
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => return Err(error),
            }
        }
        for path in &created {
            match journal.directories.directory(path) {
                Ok(_) => return Err(failure(Issue::Conflict, path.to_string_lossy())),
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => return Err(error),
            }
        }
        for path in &created {
            journal.ensure(path.parent().unwrap(), checkpoint)?;
            journal.create(path, checkpoint)?;
        }
        for (source, destination) in moved {
            let destination = Path::new(destination);
            journal.ensure(destination.parent().unwrap(), checkpoint)?;
            journal.transfer(
                Path::new(source),
                destination,
                hashes.get(destination).copied(),
                identities.get(destination).copied(),
                checkpoint,
            )?;
        }
        for (_, destination) in moved_dirs {
            journal.ensure(Path::new(destination), checkpoint)?;
        }
        for source in &removed {
            journal.remove(source, checkpoint)?;
        }
        Ok(())
    })();
    if let Err(error) = result {
        let errors = journal.rollback();
        return if errors.is_empty() {
            Err(error)
        } else {
            Err(failure(
                Issue::Incomplete,
                format!("{error}; {}", errors.join("; ")),
            ))
        };
    }
    journal.finished = true;
    journal.effects.clear();
    Ok(())
}

#[cfg(all(test, target_os = "windows"))]
mod tests {
    use super::*;
    use crate::platform::windows_reparse_tests::{Retarget, junction};

    fn details(source: &Path, destination: &Path) -> TransactionDetails {
        TransactionDetails {
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
}
