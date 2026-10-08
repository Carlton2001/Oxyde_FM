//! Reversible Copy/Move Undo effects, retained until every filesystem phase succeeds.
use super::history_move::{hash_file, move_file};
use crate::models::{CommandError, TransactionDetails};
use crate::platform::platform;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::{self, ErrorKind};
use std::path::{Path, PathBuf};

trait UndoIo {
    fn move_file(&self, from: &Path, to: &Path, hash: &str) -> io::Result<()>;
    fn create_dir(&self, path: &Path) -> io::Result<()>;
    fn remove_dir(&self, path: &Path) -> io::Result<()>;
}
struct NativeIo;
impl UndoIo for NativeIo {
    fn move_file(&self, from: &Path, to: &Path, hash: &str) -> io::Result<()> {
        move_file(from, to, Some(hash))
    }
    fn create_dir(&self, path: &Path) -> io::Result<()> {
        fs::create_dir(path)
    }
    fn remove_dir(&self, path: &Path) -> io::Result<()> {
        fs::remove_dir(path)
    }
}

struct FileMove {
    from: PathBuf,
    to: PathBuf,
    hash: String,
}
struct Plan {
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
        let hashes: HashMap<_, _> = hashes
            .iter()
            .map(|(path, hash)| (PathBuf::from(path), hash))
            .collect();
        Ok(Self {
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
                    ))
                }
                Err(error) if error.kind() == ErrorKind::NotFound => {}
                Err(error) => return Err(error),
            }
            let hash = verified_file(&destination, hashes.get(&destination).copied())?;
            moved_targets.insert(destination.clone());
            files.push(FileMove {
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
    effects: Vec<Effect>,
    staging: Vec<PathBuf>,
    discarded: Vec<(PathBuf, String)>,
    finished: bool,
}
impl Journal {
    fn ensure_dir(&mut self, ops: &impl UndoIo, path: &Path) -> io::Result<()> {
        let mut missing = Vec::new();
        let mut current = path;
        loop {
            match fs::symlink_metadata(current) {
                Ok(metadata) if metadata.is_dir() => break,
                Ok(_) => {
                    return Err(io::Error::new(
                        ErrorKind::NotADirectory,
                        format!("Not a directory: {}", current.display()),
                    ))
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
            ops.create_dir(&path)?;
            self.effects.push(Effect::CreatedDir(path));
        }
        Ok(())
    }
    fn transfer(&mut self, ops: &impl UndoIo, effect: FileMove) -> io::Result<()> {
        ops.move_file(&effect.from, &effect.to, &effect.hash)?;
        self.effects.push(Effect::Move(effect));
        Ok(())
    }
    fn stage(
        &mut self,
        ops: &impl UndoIo,
        path: PathBuf,
        hash: String,
        remove_dirs: &[PathBuf],
    ) -> io::Result<()> {
        let mut parent = path
            .parent()
            .ok_or_else(|| io::Error::other("Output has no parent"))?;
        while remove_dirs
            .iter()
            .any(|directory| parent.starts_with(directory))
        {
            parent = parent.parent().ok_or_else(|| {
                io::Error::other("Cannot stage output outside removed directories")
            })?;
        }
        // Once it can contain user data, this directory must not be recursively
        // deleted by RAII: a failed rollback retains it for recovery.
        let directory = tempfile::Builder::new()
            .prefix(".oxyde-undo-")
            .tempdir_in(parent)?;
        platform()
            .set_hidden_attribute(directory.path(), true)
            .map_err(io::Error::other)?;
        let directory = directory.keep();
        self.staging.push(directory.clone());
        let saved = directory.join("output");
        self.transfer(
            ops,
            FileMove {
                from: path,
                to: saved.clone(),
                hash: hash.clone(),
            },
        )?;
        self.discarded.push((saved, hash));
        Ok(())
    }
    fn remove_dir(&mut self, ops: &impl UndoIo, path: &Path) -> io::Result<()> {
        let metadata = match fs::symlink_metadata(path) {
            Ok(metadata)
                if metadata.is_dir()
                    && !metadata.file_type().is_symlink()
                    && !platform().file_attributes(&metadata, "").2 =>
            {
                metadata
            }
            Ok(_) => {
                return Err(io::Error::new(
                    ErrorKind::NotADirectory,
                    format!("Cannot remove journal directory: {}", path.display()),
                ))
            }
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(error),
        };
        match ops.remove_dir(path) {
            Ok(()) => self.effects.push(Effect::RemovedDir {
                path: path.to_path_buf(),
                permissions: metadata.permissions(),
                hidden: platform().file_attributes(&metadata, "").0,
            }),
            Err(error)
                if matches!(
                    error.kind(),
                    ErrorKind::NotFound | ErrorKind::DirectoryNotEmpty
                ) => {}
            Err(error) => return Err(error),
        }
        Ok(())
    }
    fn clean_empty_staging(&mut self) -> Vec<String> {
        let mut errors = Vec::new();
        for path in self.staging.drain(..) {
            if let Err(error) = fs::remove_dir(&path) {
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
                    let result = move_file(&effect.to, &effect.from, Some(&effect.hash));
                    (effect.to, result)
                }
                Effect::CreatedDir(path) => {
                    let result = fs::remove_dir(&path).or_else(|error| {
                        if error.kind() == ErrorKind::NotFound {
                            Ok(())
                        } else {
                            Err(error)
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
                        fs::create_dir(&path)?;
                        fs::set_permissions(&path, permissions)?;
                        platform()
                            .set_hidden_attribute(&path, hidden)
                            .map_err(io::Error::other)
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
        for (path, expected) in self.discarded.drain(..) {
            let result = super::file_copy::remove_verified_output(&path, &expected);
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
    let mut journal = Journal::default();
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
            journal.stage(ops, path, hash, &plan.remove_dirs)?;
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

#[cfg(test)]
mod tests {
    use super::*;

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
        fn move_file(&self, from: &Path, to: &Path, hash: &str) -> io::Result<()> {
            NativeIo.move_file(from, to, hash)
        }
        fn create_dir(&self, path: &Path) -> io::Result<()> {
            NativeIo.create_dir(path)
        }
        fn remove_dir(&self, path: &Path) -> io::Result<()> {
            (self.remove)(path)
        }
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
                    fs::remove_dir(path)
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
                fs::remove_dir(path)
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
            fn move_file(&self, from: &Path, to: &Path, hash: &str) -> io::Result<()> {
                NativeIo.move_file(from, to, hash)
            }
            fn create_dir(&self, path: &Path) -> io::Result<()> {
                if path == self.0 {
                    Err(io::Error::new(
                        ErrorKind::PermissionDenied,
                        "injected directory creation error",
                    ))
                } else {
                    fs::create_dir(path)
                }
            }
            fn remove_dir(&self, path: &Path) -> io::Result<()> {
                fs::remove_dir(path)
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
                    fs::remove_dir(path)
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
                        fs::remove_dir(path)
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
        let writer = fs::File::options()
            .write(true)
            .open(&fixture.partial)
            .unwrap();
        let error = apply_plan(&NativeIo, Plan::copy(&fixture.details).unwrap()).unwrap_err();
        assert_eq!(error.raw_os_error(), Some(32));
        fixture.unchanged();
        drop(writer);
        apply_plan(&NativeIo, Plan::copy(&fixture.details).unwrap()).unwrap();
        assert!(!fixture.destination.exists());
        assert!(!fixture.partial.exists());
        assert!(fixture.staging().is_empty());
    }
}
