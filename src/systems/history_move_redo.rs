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
        Err(super::history_move::move_identity_failure(from, false))
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
    /// The link moved again from `source` to `path`.
    MovedLink(crate::models::transaction::HistoryLink),
}

/// Accept only the journal's link, still pointing to the same target.
fn same_link(link: &crate::models::transaction::HistoryLink, current: &fs::File) -> io::Result<()> {
    let identity = link
        .identity
        .snapshot()
        .ok_or_else(|| super::history_move::move_identity_failure(&link.source, false))?;
    let metadata = current.metadata()?;
    match platform().source_link_target(current, &metadata) {
        Some(Ok(target)) if target == link.target && identity.matches(current)? => Ok(()),
        Some(Err(error)) => Err(error),
        _ => Err(super::history_move::move_identity_failure(&link.source, true)),
    }
}
fn move_journal_link(
    directories: &mut Directories,
    link: &crate::models::transaction::HistoryLink,
    from: &Path,
    to: &Path,
) -> io::Result<()> {
    let source = directories.file_parents(from)?;
    let destination = directories.file_parents(to)?;
    let moved = super::link_moves::move_link(
        source.directory(),
        from,
        destination.directory(),
        to,
        &link.target,
        false,
        &mut |current| same_link(link, current),
    )?;
    link.identity.refresh_from_file(&moved)
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
                Effect::MovedLink(link) => {
                    let result = move_journal_link(&mut self.directories, &link, &link.path, &link.source);
                    (link.path, result)
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
    let links = details.moved_links.clone().unwrap_or_default();
    let mut paths = created.clone();
    for link in &links {
        for path in [&link.source, &link.path] {
            paths.push(path.parent().ok_or_else(|| io::Error::other("Move Redo path has no parent"))?.to_path_buf());
        }
    }
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
    let attributes = super::history_directory_attributes::RecordedDirectories::created(details);
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
            let identity = identities
                .get(destination)
                .and_then(|proof| proof.snapshot())
                .ok_or_else(|| super::history_move::move_identity_failure(source, false))?;
            if !identity.matches(&file)? {
                return Err(super::history_move::move_identity_failure(source, true));
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
        for link in &links {
            let parents = journal.directories.file_parents(&link.source)?;
            same_link(link, &platform().track_source_item(parents.directory(), &link.source)?)?;
            if fs::symlink_metadata(&link.path).is_ok() {
                return Err(failure(Issue::Conflict, link.path.to_string_lossy()));
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
        for link in &links {
            journal.ensure(link.path.parent().unwrap(), checkpoint)?;
            checkpoint.before(Phase::Move, &link.source)?;
            move_journal_link(&mut journal.directories, link, &link.source, &link.path)
                .map_err(|error| at(error, &link.path))?;
            journal.effects.push(Effect::MovedLink(link.clone()));
        }
        for (_, destination) in moved_dirs {
            journal.ensure(Path::new(destination), checkpoint)?;
        }
        for source in &removed {
            journal.remove(source, checkpoint)?;
        }
        let recreated: Vec<_> = journal
            .effects
            .iter()
            .filter_map(|effect| match effect {
                Effect::Created(path) => Some(path.clone()),
                _ => None,
            })
            .collect();
        attributes.apply(&mut journal.directories, &recreated);
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
#[path = "history_move_redo/tests/tests.rs"]
mod tests;
