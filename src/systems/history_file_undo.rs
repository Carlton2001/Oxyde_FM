//! Reversible Copy/Move Undo effects, retained until every filesystem phase succeeds.
use super::history_move::hash_file;
use super::path_parents::HeldParents;
use crate::models::transaction::{HistoryDirectoryIdentity, HistoryFileIdentity};
use crate::models::{CommandError, TransactionDetails};
use crate::platform::platform;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::{self, ErrorKind};
use std::path::{Path, PathBuf};

trait UndoIo {
    fn move_file(
        &self,
        from: &Path,
        to: &Path,
        hash: &str,
        parents: (&HeldParents, &HeldParents),
        identity: Option<HistoryFileIdentity>,
        journal_identity: Option<&crate::models::transaction::HistoryMoveIdentity>,
    ) -> io::Result<()>;
    fn before_create_dir(&self, _path: &Path) -> io::Result<()> {
        Ok(())
    }
    fn before_remove_dir(&self, _path: &Path) -> io::Result<()> {
        Ok(())
    }
}
struct NativeIo;
impl UndoIo for NativeIo {
    fn move_file(
        &self,
        from: &Path,
        to: &Path,
        hash: &str,
        parents: (&HeldParents, &HeldParents),
        identity: Option<HistoryFileIdentity>,
        journal_identity: Option<&crate::models::transaction::HistoryMoveIdentity>,
    ) -> io::Result<()> {
        if let Some(identity) = journal_identity {
            super::history_move::move_file_with_journal_identity(
                from,
                to,
                Some(hash),
                identity,
                parents.0,
                parents.1,
            )
            .map(|_| ())
        } else if let Some(identity) = identity {
            super::history_move::move_file_with_identity(
                from, to, hash, identity, parents.0, parents.1,
            )
        } else {
            Err(super::history_move::move_identity_failure(from, false))
        }
    }
}

struct FileMove {
    identity: Option<HistoryFileIdentity>,
    journal_identity: Option<crate::models::transaction::HistoryMoveIdentity>,
    from: PathBuf,
    to: PathBuf,
    hash: String,
}
struct Plan {
    directory_identities: HashMap<PathBuf, HistoryDirectoryIdentity>,
    identities: HashMap<PathBuf, HistoryFileIdentity>,
    files: Vec<FileMove>,
    discard: Vec<(PathBuf, String)>,
    source_dirs: Vec<PathBuf>,
    remove_dirs: Vec<PathBuf>,
}
fn verified_file(path: &Path, expected: Option<&String>) -> io::Result<String> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_file()
        || metadata.file_type().is_symlink()
        || platform().file_attributes(&metadata, "").2
    {
        return Err(io::Error::other(format!(
            "Not an ordinary journal file: {}",
            path.display()
        )));
    }
    let hash = hash_file(path)?;
    if expected.is_some_and(|expected| expected != &hash) {
        return Err(io::Error::other(format!(
            "Cannot undo move; file changed: {}",
            path.display()
        )));
    }
    Ok(hash)
}
impl Plan {
    fn copy(details: &TransactionDetails) -> io::Result<Self> {
        let (Some(created), Some(created_dirs), Some(hashes)) = (
            &details.created_files,
            &details.created_dirs,
            &details.copied_file_hashes,
        ) else {
            return Err(io::Error::other(
                "This copy predates the effect journal and cannot be undone safely",
            ));
        };
        if details
            .overwritten_files
            .as_ref()
            .is_some_and(|files| !files.is_empty())
        {
            return Err(io::Error::other(
                "This copy overwrote existing files and cannot be undone",
            ));
        }
        let directory_identities: HashMap<_, _> = details
            .copied_directory_identities
            .as_deref()
            .unwrap_or_default()
            .iter()
            .map(|(path, identity)| (PathBuf::from(path), identity.clone()))
            .collect();
        for path in created_dirs.iter().map(PathBuf::from) {
            if directory_identities
                .get(&path)
                .and_then(HistoryDirectoryIdentity::snapshot)
                .is_none()
            {
                return Err(io::Error::other(
                    crate::models::error::HistoryIdentityFailure {
                        issue: crate::models::error::HistoryIdentityIssue::Unverified,
                        path: path.to_string_lossy().into_owned(),
                    },
                ));
            }
        }
        let hashes: HashMap<_, _> = hashes
            .iter()
            .map(|(path, hash)| (PathBuf::from(path), hash))
            .collect();
        let identities: HashMap<_, _> = details
            .copied_file_identities
            .as_deref()
            .unwrap_or_default()
            .iter()
            .map(|(path, identity)| (PathBuf::from(path), *identity))
            .collect();
        for path in created.iter().map(PathBuf::from) {
            let identity = identities.get(&path).ok_or_else(|| {
                io::Error::other(crate::models::error::HistoryIdentityFailure {
                    issue: crate::models::error::HistoryIdentityIssue::Unverified,
                    path: path.to_string_lossy().into_owned(),
                })
            })?;
            let parents = HeldParents::new(&path)?;
            let file = crate::platform::platform()
                .open_archive_source(&parents.resolved_path(&path)?, false)?;
            if !identity.matches(&file)? {
                return Err(super::file_copy::identity_changed(&path));
            }
        }
        Ok(Self {
            directory_identities,
            identities,
            files: vec![],
            source_dirs: vec![],
            discard: verified_outputs(created, &hashes, &HashSet::new())?,
            remove_dirs: sorted_directories(created_dirs),
        })
    }
    fn new(details: &TransactionDetails) -> io::Result<Self> {
        let (Some(moved), Some(created), Some(created_dirs), Some(moved_dirs), Some(hashes)) = (
            &details.moved_files,
            &details.created_files,
            &details.created_dirs,
            &details.moved_dirs,
            &details.copied_file_hashes,
        ) else {
            return Err(io::Error::other(
                "This move predates the effect journal and cannot be undone safely",
            ));
        };
        for backup in details
            .trash_metadata_backups
            .as_deref()
            .unwrap_or_default()
        {
            verified_file(Path::new(backup), None)?;
        }
        let hashes: HashMap<_, _> = hashes
            .iter()
            .map(|(path, hash)| (PathBuf::from(path), hash))
            .collect();
        let mut files = Vec::new();
        let move_identities: HashMap<_, _> = details
            .moved_file_identities
            .as_deref()
            .unwrap_or_default()
            .iter()
            .map(|(path, identity)| (PathBuf::from(path), identity.clone()))
            .collect();
        let mut moved_targets = HashSet::new();
        for (original, destination) in moved.iter().rev() {
            let original = PathBuf::from(original);
            let destination = PathBuf::from(destination);
            match fs::symlink_metadata(&original) {
                Ok(_) => {
                    return Err(io::Error::new(
                        ErrorKind::AlreadyExists,
                        format!(
                            "Cannot undo move; original path exists: {}",
                            original.display()
                        ),
                    ));
                }
                Err(error) if error.kind() == ErrorKind::NotFound => {}
                Err(error) => return Err(error),
            }
            let hash = verified_file(&destination, hashes.get(&destination).copied())?;
            // Recovery sidecars obey the same proof as user files.
            let identity = move_identities
                .get(&destination)
                .ok_or_else(|| super::history_move::move_identity_failure(&destination, false))?;
            let parents = HeldParents::new(&destination)?;
            let file = parents.open_file(&destination)?;
            if !identity
                .snapshot()
                .ok_or_else(|| super::history_move::move_identity_failure(&destination, false))?
                .matches(&file)?
            {
                return Err(super::history_move::move_identity_failure(
                    &destination,
                    true,
                ));
            }
            moved_targets.insert(destination.clone());
            files.push(FileMove {
                identity: None,
                journal_identity: Some(identity.clone()),
                from: destination,
                to: original,
                hash,
            });
        }
        let discard = verified_outputs(created, &hashes, &moved_targets)?;
        let identities: HashMap<_, _> = details
            .copied_file_identities
            .as_deref()
            .unwrap_or_default()
            .iter()
            .map(|(path, identity)| (PathBuf::from(path), *identity))
            .collect();
        // A published copy whose source could not be removed is still user data.
        // Verify every such output before the first completed file is moved back.
        for (path, _) in &discard {
            let identity = identities
                .get(path)
                .ok_or_else(|| super::history_move::move_identity_failure(path, false))?;
            let parents = HeldParents::new(path)?;
            let file = parents.open_file(path)?;
            if !identity.matches(&file)? {
                return Err(super::history_move::move_identity_failure(path, true));
            }
        }
        let mut source_dirs: Vec<_> = moved_dirs
            .iter()
            .map(|(source, _)| PathBuf::from(source))
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        source_dirs.sort_by_key(|path| path.components().count());
        let remove_dirs = sorted_directories(created_dirs);
        let mut directory_identities: HashMap<_, _> = details
            .copied_directory_identities
            .as_deref()
            .unwrap_or_default()
            .iter()
            .map(|(path, identity)| (PathBuf::from(path), identity.clone()))
            .collect();
        for path in &remove_dirs {
            if directory_identities
                .get(path)
                .and_then(HistoryDirectoryIdentity::snapshot)
                .is_none()
            {
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
        for path in &source_dirs {
            let proof = restored
                .get(path)
                .ok_or_else(|| super::history_move::move_identity_failure(path, false))?;
            // The initial source was removed. A later Redo may keep our restored
            // directory if independent children arrive; accept only that same object.
            match fs::symlink_metadata(path) {
                Ok(_) => {
                    let snapshot = proof
                        .snapshot()
                        .ok_or_else(|| super::history_move::move_identity_failure(path, true))?;
                    let parents = HeldParents::new(path)?;
                    let directory =
                        platform().track_operation_directory(parents.directory(), path)?;
                    if !snapshot.matches(&directory)? {
                        return Err(super::history_move::move_identity_failure(path, true));
                    }
                }
                Err(error) if error.kind() == ErrorKind::NotFound => {}
                Err(error) => return Err(error),
            }
            directory_identities.insert(path.clone(), proof.clone());
        }
        Ok(Self {
            directory_identities,
            identities,
            files,
            discard,
            source_dirs,
            remove_dirs,
        })
    }
}

fn verified_outputs(
    created: &[String],
    hashes: &HashMap<PathBuf, &String>,
    excluded: &HashSet<PathBuf>,
) -> io::Result<Vec<(PathBuf, String)>> {
    let mut outputs = Vec::new();
    let mut seen = HashSet::new();
    for path in created.iter().map(PathBuf::from) {
        if excluded.contains(&path) || !seen.insert(path.clone()) {
            continue;
        }
        let expected = hashes.get(&path).copied().ok_or_else(|| {
            io::Error::other(format!(
                "Cannot undo unverified copy/move output: {}",
                path.display()
            ))
        })?;
        let hash = verified_file(&path, Some(expected))?;
        outputs.push((path, hash));
    }
    Ok(outputs)
}
fn sorted_directories(paths: &[String]) -> Vec<PathBuf> {
    let mut paths: Vec<_> = paths
        .iter()
        .map(PathBuf::from)
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    paths.sort_by_key(|path| std::cmp::Reverse(path.components().count()));
    paths
}

enum Effect {
    Move(FileMove),
    CreatedDir(PathBuf),
    RemovedDir {
        path: PathBuf,
        permissions: fs::Permissions,
        hidden: bool,
    },
}
#[derive(Default)]
struct Journal {
    directory_identities: HashMap<PathBuf, HistoryDirectoryIdentity>,
    effects: Vec<Effect>,
    staging: Vec<(PathBuf, fs::File)>,
    directories: super::history_directories::Directories,
    discarded: Vec<(PathBuf, String, Option<HistoryFileIdentity>)>,
    finished: bool,
    recovery_locations: Vec<PathBuf>,
}
impl Journal {
    fn ensure_dir(&mut self, ops: &impl UndoIo, path: &Path) -> io::Result<()> {
        let mut missing = Vec::new();
        let mut current = path;
        loop {
            match self.directories.directory(current) {
                Ok(_) => {
                    if let Some(expected) = self.directory_identities.get(current) {
                        self.directories
                            .verify_history_identity(current, expected)?;
                    }
                    break;
                }
                Err(error) if error.kind() == ErrorKind::NotFound => {
                    missing.push(current.to_path_buf());
                    current = current.parent().ok_or_else(|| {
                        io::Error::new(ErrorKind::NotFound, "Filesystem root is unavailable")
                    })?;
                }
                Err(error) => return Err(error),
            }
        }
        for path in missing.into_iter().rev() {
            ops.before_create_dir(&path)?;
            self.directories.create(&path, false)?;
            self.effects.push(Effect::CreatedDir(path.clone()));
            if let Some(expected) = self.directory_identities.get(&path) {
                expected.refresh_from_file(self.directories.directory(&path)?)?;
            }
        }
        Ok(())
    }
    fn transfer(&mut self, ops: &impl UndoIo, effect: FileMove) -> io::Result<()> {
        let source = self.directories.file_parents(&effect.from)?;
        let destination = self.directories.file_parents(&effect.to)?;
        ops.move_file(
            &effect.from,
            &effect.to,
            &effect.hash,
            (&source, &destination),
            effect.identity,
            effect.journal_identity.as_ref(),
        )?;
        self.effects.push(Effect::Move(effect));
        Ok(())
    }
    fn stage(
        &mut self,
        ops: &impl UndoIo,
        path: PathBuf,
        hash: String,
        remove_dirs: &[PathBuf],
        identity: Option<HistoryFileIdentity>,
    ) -> io::Result<()> {
        let parent = staging_parent(&path, remove_dirs)?;
        // Once it can contain user data, this directory must not be recursively
        // deleted by RAII: a failed rollback retains it for recovery.
        let directory = parent.join(format!(".oxyde-undo-{}", uuid::Uuid::new_v4()));
        let pin = self.directories.stage(&directory)?;
        let saved = directory.join("output");
        self.staging.push((directory, pin));
        self.transfer(
            ops,
            FileMove {
                identity,
                journal_identity: None,
                from: path,
                to: saved.clone(),
                hash: hash.clone(),
            },
        )?;
        self.discarded.push((saved, hash, identity));
        Ok(())
    }
    fn remove_dir(&mut self, ops: &impl UndoIo, path: &Path) -> io::Result<()> {
        let Some(metadata) = self.directories.metadata(path)? else {
            return Ok(());
        };
        ops.before_remove_dir(path)?;
        if let Some(expected) = self.directory_identities.get(path) {
            self.directories.verify_history_identity(path, expected)?;
        }
        if matches!(
            self.directories.remove(path)?,
            super::history_directories::Removal::Deleted
        ) {
            self.effects.push(Effect::RemovedDir {
                path: path.to_path_buf(),
                permissions: metadata.permissions(),
                hidden: platform().file_attributes(&metadata, "").0,
            });
        }
        Ok(())
    }
    fn clean_empty_staging(&mut self) -> Vec<String> {
        let mut errors = Vec::new();
        for (path, pin) in self.staging.drain(..) {
            if let Err(error) = self.directories.clean_stage(&path, pin) {
                if error.kind() != ErrorKind::NotFound {
                    self.recovery_locations.push(
                        self.directories
                            .resolved(&path)
                            .unwrap_or_else(|_| path.clone()),
                    );
                    errors.push(format!("staging retained at {}: {error}", path.display()));
                }
            }
        }
        errors
    }
    fn rollback(&mut self) -> Vec<String> {
        let mut errors = Vec::new();
        let mut blocked_parents: Vec<PathBuf> = Vec::new();
        while let Some(effect) = self.effects.pop() {
            let (path, result) = match effect {
                Effect::Move(effect) => {
                    let result = (|| {
                        if blocked_parents
                            .iter()
                            .any(|parent| effect.from.starts_with(parent))
                        {
                            return Err(io::Error::other(
                                "Undo parent could not be restored; file retained for recovery",
                            ));
                        }
                        let source = self.directories.file_parents(&effect.to)?;
                        let destination = self.directories.file_parents(&effect.from)?;
                        NativeIo.move_file(
                            &effect.to,
                            &effect.from,
                            &effect.hash,
                            (&source, &destination),
                            effect.identity,
                            effect.journal_identity.as_ref(),
                        )
                    })();
                    (effect.to, result)
                }
                Effect::CreatedDir(path) => {
                    let result = self.directories.remove(&path).and_then(|removed| {
                        if matches!(removed, super::history_directories::Removal::NotEmpty) {
                            Err(io::Error::other(
                                "Created journal directory could not be removed",
                            ))
                        } else {
                            Ok(())
                        }
                    });
                    (path, result)
                }
                Effect::RemovedDir {
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
                                "Undo ancestor could not be restored; replacement preserved",
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
                self.recovery_locations.push(
                    self.directories
                        .resolved(&path)
                        .unwrap_or_else(|_| path.clone()),
                );
                errors.push(format!("{}: {error}", path.display()));
            }
        }
        errors.extend(self.clean_empty_staging());
        self.finished = true;
        errors
    }
    fn commit(&mut self) {
        self.finished = true;
        self.effects.clear();
        for (path, expected, identity) in self.discarded.drain(..) {
            let result = self.directories.file_parents(&path).and_then(|parents| {
                super::file_copy::remove_verified_output_checked(
                    &path, &expected, &parents, identity,
                )
            });
            if let Err(error) = result {
                log::warn!(
                    "Undo completed; staged output retained at {}: {error}",
                    path.display()
                );
            }
        }
        for error in self.clean_empty_staging() {
            log::warn!("Undo completed; {error}");
        }
    }
}
impl Drop for Journal {
    fn drop(&mut self) {
        if !self.finished {
            for error in self.rollback() {
                log::error!("Undo file effects unwind rollback: {error}");
            }
        }
    }
}
pub(crate) fn undo(details: &TransactionDetails) -> Result<(), CommandError> {
    undo_with(&NativeIo, details).map_err(|error| {
        match CommandError::from(error) {
            // Partial Move outputs use Copy's fixed-identity staging primitive,
            // while their user-facing explanation still concerns the move.
            CommandError::HistoryIdentity(failure) => {
                CommandError::HistoryMove(crate::models::error::HistoryMoveFailure {
                    issue: match failure.issue {
                        crate::models::error::HistoryIdentityIssue::Changed => {
                            crate::models::error::HistoryMoveIssue::Changed
                        }
                        crate::models::error::HistoryIdentityIssue::Unverified => {
                            crate::models::error::HistoryMoveIssue::Unverified
                        }
                    },
                    detail: failure.path,
                })
            }
            other => other,
        }
    })
}
pub(crate) fn undo_copy(details: &TransactionDetails) -> Result<(), CommandError> {
    apply_plan(&NativeIo, Plan::copy(details)?).map_err(CommandError::from)
}
fn undo_with(ops: &impl UndoIo, details: &TransactionDetails) -> io::Result<()> {
    apply_plan(ops, Plan::new(details)?)
}
fn apply_plan(ops: &impl UndoIo, plan: Plan) -> io::Result<()> {
    let mut paths = plan.source_dirs.clone();
    paths.extend(
        plan.files
            .iter()
            .filter_map(|effect| effect.to.parent().map(Path::to_path_buf)),
    );
    paths.extend(
        plan.files
            .iter()
            .filter_map(|effect| effect.from.parent().map(Path::to_path_buf)),
    );
    for (path, _) in &plan.discard {
        paths.push(staging_parent(path, &plan.remove_dirs)?.to_path_buf());
    }
    let mut directories =
        super::history_directories::Directories::prepare(&paths, &plan.remove_dirs)?;
    for path in &plan.source_dirs {
        match directories.directory(path) {
            Ok(_) => {
                let expected = &plan.directory_identities[path];
                if expected.snapshot().is_none() {
                    return Err(super::history_move::move_identity_failure(path, true));
                }
                directories.verify_history_identity(path, expected)?;
            }
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
    }
    for path in &plan.remove_dirs {
        if let Some(expected) = plan.directory_identities.get(path) {
            directories.verify_history_identity(path, expected)?;
        }
    }
    let mut journal = Journal::default();
    journal.directories = directories;
    journal.directory_identities = plan.directory_identities;
    let result = (|| {
        for effect in plan.files {
            if let Some(parent) = effect.to.parent() {
                journal.ensure_dir(ops, parent)?;
            }
            journal.transfer(ops, effect)?;
        }
        for directory in plan.source_dirs {
            journal.ensure_dir(ops, &directory)?;
        }
        for (path, hash) in plan.discard {
            let identity = plan.identities.get(&path).copied();
            journal.stage(ops, path, hash, &plan.remove_dirs, identity)?;
        }
        for directory in plan.remove_dirs {
            journal.remove_dir(ops, &directory)?;
        }
        Ok::<_, io::Error>(())
    })();
    if let Err(error) = result {
        let errors = journal.rollback();
        return if errors.is_empty() {
            Err(error)
        } else {
            journal.recovery_locations.sort();
            journal.recovery_locations.dedup();
            let locations = journal
                .recovery_locations
                .iter()
                .map(|path| path.to_string_lossy().into_owned())
                .collect::<Vec<_>>()
                .join("\n");
            let technical_detail = format!("{error}; rollback incomplete: {}", errors.join("; "));
            log::error!("Undo recovery incomplete: {technical_detail}");
            Err(io::Error::new(
                error.kind(),
                crate::models::error::HistoryUndoRecoveryFailure {
                    locations,
                    technical_detail,
                },
            ))
        };
    }
    journal.commit();
    Ok(())
}

fn staging_parent<'a>(path: &'a Path, remove_dirs: &[PathBuf]) -> io::Result<&'a Path> {
    let mut parent = path
        .parent()
        .ok_or_else(|| io::Error::other("Output has no parent"))?;
    while remove_dirs
        .iter()
        .any(|directory| parent.starts_with(directory))
    {
        parent = parent
            .parent()
            .ok_or_else(|| io::Error::other("Cannot stage output outside removed directories"))?;
    }
    Ok(parent)
}

#[cfg(test)]
#[path = "history_file_undo/tests/tests.rs"]
mod tests;
