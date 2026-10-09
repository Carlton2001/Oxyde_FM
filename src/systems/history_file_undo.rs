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
    ) -> io::Result<()> {
        if let Some(identity) = identity {
            super::history_move::move_file_with_identity(
                from, to, hash, identity, parents.0, parents.1,
            )
        } else {
            super::history_move::move_file_with_parents(from, to, hash, parents.0, parents.1)
        }
    }
}

struct FileMove {
    identity: Option<HistoryFileIdentity>,
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
            moved_targets.insert(destination.clone());
            files.push(FileMove {
                identity: None,
                from: destination,
                to: original,
                hash,
            });
        }
        let discard = verified_outputs(created, &hashes, &moved_targets)?;
        let mut source_dirs: Vec<_> = moved_dirs
            .iter()
            .map(|(source, _)| PathBuf::from(source))
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        source_dirs.sort_by_key(|path| path.components().count());
        let remove_dirs = sorted_directories(created_dirs);
        Ok(Self {
            directory_identities: HashMap::new(),
            identities: HashMap::new(),
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
}
impl Journal {
    fn ensure_dir(&mut self, ops: &impl UndoIo, path: &Path) -> io::Result<()> {
        let mut missing = Vec::new();
        let mut current = path;
        loop {
            match self.directories.directory(current) {
                Ok(_) => break,
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
            self.effects.push(Effect::CreatedDir(path));
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
                    errors.push(format!("staging retained at {}: {error}", path.display()));
                }
            }
        }
        errors
    }
    fn rollback(&mut self) -> Vec<String> {
        let mut errors = Vec::new();
        while let Some(effect) = self.effects.pop() {
            let (path, result) = match effect {
                Effect::Move(effect) => {
                    let result = (|| {
                        let source = self.directories.file_parents(&effect.to)?;
                        let destination = self.directories.file_parents(&effect.from)?;
                        NativeIo.move_file(
                            &effect.to,
                            &effect.from,
                            &effect.hash,
                            (&source, &destination),
                            effect.identity,
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
                        let directory = self.directories.create(&path, hidden)?;
                        if let Some(expected) = self.directory_identities.get(&path) {
                            expected.refresh_from_file(directory)?;
                        }
                        platform().restore_journal_directory(directory, &path, permissions, hidden)
                    })();
                    (path, result)
                }
            };
            if let Err(error) = result {
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
    undo_with(&NativeIo, details).map_err(CommandError::from)
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
    let directories = super::history_directories::Directories::prepare(&paths, &plan.remove_dirs)?;
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
            Err(io::Error::new(
                error.kind(),
                format!("{error}; rollback incomplete: {}", errors.join("; ")),
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
            ) -> io::Result<()> {
                if from == self.selected {
                    fs::rename(from, &self.saved)?;
                    fs::write(from, b"moved content")?;
                }
                NativeIo.move_file(from, to, hash, parents, identity)
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

    struct Fixture {
        root: tempfile::TempDir,
        original: PathBuf,
        destination: PathBuf,
        partial: PathBuf,
        details: TransactionDetails,
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
        ) -> io::Result<()> {
            NativeIo.move_file(from, to, hash, parents, identity)
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
            ) -> io::Result<()> {
                NativeIo.move_file(from, to, hash, parents, identity)?;
                if !self.0.exists() {
                    fs::create_dir(&self.0)?;
                }
                Ok(())
            }
        }
        let mut fixture = Fixture::new();
        let arrival = fixture.root.path().join("new-empty");
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
            ) -> io::Result<()> {
                NativeIo.move_file(from, to, hash, parents, identity)
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
            ) -> io::Result<()> {
                NativeIo.move_file(from, to, hash, parents, identity)
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
        assert_eq!(fs::read(&fixture.destination).unwrap(), b"moved content");
        assert!(!fixture.original.exists());
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
