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
mod tests {
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
}
