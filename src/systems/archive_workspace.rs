//! Temporary filesystem views used to edit archives without touching the original.
use crate::commands::archive::extract_archive_with_checkpoint;
use crate::systems::file_ops::{FileOpType, FileOperation, FileOperationManager};
use crate::utils::archive::{split_virtual_path, ArchiveFormat};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock, Mutex, MutexGuard, Weak};

type MutationMutex = Arc<Mutex<()>>;
static MUTATION_LOCKS: LazyLock<Mutex<HashMap<PathBuf, Weak<Mutex<()>>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

// Stable lock ordering protects transfers between archives without blocking unrelated archives.
pub(crate) fn mutation_locks(
    paths: impl IntoIterator<Item = PathBuf>,
) -> Result<Vec<MutationMutex>, String> {
    let mut paths = paths
        .into_iter()
        .map(std::fs::canonicalize)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    paths.sort();
    paths.dedup();
    let mut registry = MUTATION_LOCKS.lock().map_err(|e| e.to_string())?;
    registry.retain(|_, lock| lock.strong_count() > 0);
    Ok(paths
        .into_iter()
        .map(|path| {
            if let Some(lock) = registry.get(&path).and_then(Weak::upgrade) {
                lock
            } else {
                let lock = Arc::new(Mutex::new(()));
                registry.insert(path, Arc::downgrade(&lock));
                lock
            }
        })
        .collect())
}

pub(crate) fn lock_for_operation<'a>(
    lock: &'a MutationMutex,
    operation: &Arc<Mutex<FileOperation>>,
) -> Result<MutexGuard<'a, ()>, String> {
    loop {
        FileOperationManager::archive_checkpoint(operation)?;
        match lock.try_lock() {
            Ok(guard) => return Ok(guard),
            Err(std::sync::TryLockError::WouldBlock) => {
                std::thread::sleep(std::time::Duration::from_millis(20))
            }
            Err(error) => return Err(error.to_string()),
        }
    }
}

pub(crate) fn virtual_source(path: &Path) -> Option<(PathBuf, String)> {
    split_virtual_path(&path.to_string_lossy())
        .filter(|(archive, inner)| archive.is_file() && !inner.is_empty())
}

pub(crate) fn virtual_destination(path: &Path) -> Option<(PathBuf, String)> {
    split_virtual_path(&path.to_string_lossy()).filter(|(archive, _)| archive.is_file())
}

pub(crate) fn writable(path: &Path) -> bool {
    matches!(
        ArchiveFormat::from_path(path),
        Some(
            ArchiveFormat::Zip
                | ArchiveFormat::SevenZip
                | ArchiveFormat::Tar
                | ArchiveFormat::TarZst
        )
    )
}

pub(crate) struct ArchiveWorkspace {
    pub archive: PathBuf,
    directory: tempfile::TempDir,
    length: u64,
    modified: Option<std::time::SystemTime>,
}

impl ArchiveWorkspace {
    pub fn open(archive: PathBuf) -> Result<Self, String> {
        Self::open_with_checkpoint(archive, &|| Ok(()))
    }
    pub fn open_for_operation(
        archive: PathBuf,
        op: &Arc<Mutex<FileOperation>>,
    ) -> Result<Self, String> {
        Self::open_with_checkpoint(archive, &|| {
            FileOperationManager::archive_checkpoint(op).map_err(std::io::Error::other)
        })
    }
    fn open_with_checkpoint(
        archive: PathBuf,
        checkpoint: &dyn Fn() -> std::io::Result<()>,
    ) -> Result<Self, String> {
        let metadata = std::fs::metadata(&archive).map_err(|e| e.to_string())?;
        let directory = tempfile::tempdir().map_err(|e| e.to_string())?;
        extract_archive_with_checkpoint(
            archive.to_string_lossy().into_owned(),
            directory.path().to_string_lossy().into_owned(),
            checkpoint,
        )
        .map_err(|e| e.to_string())?;
        // Never follow archive-provided links when copying or rebuilding a workspace.
        for entry in walkdir::WalkDir::new(directory.path()).follow_links(false) {
            if entry.map_err(|e| e.to_string())?.file_type().is_symlink() {
                return Err("Archive links are not supported for file operations".into());
            }
        }
        Ok(Self {
            archive,
            directory,
            length: metadata.len(),
            modified: metadata.modified().ok(),
        })
    }

    pub fn root(&self) -> &Path {
        self.directory.path()
    }

    pub fn path(&self, internal: &str) -> Result<PathBuf, String> {
        let relative = crate::utils::archive_security::relative_archive_path(internal)
            .map_err(|error| error.to_string())?;
        Ok(self.root().join(relative))
    }

    pub fn commit(&self, op: &Arc<Mutex<FileOperation>>) -> Result<(), String> {
        if !writable(&self.archive) {
            return Err("This archive format is read-only".into());
        }
        let parent = self.archive.parent().ok_or("Invalid archive path")?;
        let output = tempfile::tempdir_in(parent).map_err(|e| e.to_string())?;
        let destination = output
            .path()
            .join(self.archive.file_name().ok_or("Invalid archive name")?);
        let sources = std::fs::read_dir(self.root())
            .map_err(|e| e.to_string())?
            .map(|e| e.map(|e| e.path()))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        let original_destination = op.lock().unwrap().destination.clone();
        let original_quality = op.lock().unwrap().compression_quality.clone();
        let extension = self
            .archive
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("zip")
            .to_ascii_lowercase();
        let quality = op
            .lock()
            .unwrap()
            .archive_qualities
            .get(&extension)
            .cloned()
            .unwrap_or_else(|| original_quality.clone());
        op.lock().unwrap().compression_quality = quality;
        let result = FileOperationManager::perform_compress(op, sources, Some(destination.clone()));
        op.lock().unwrap().destination = original_destination;
        op.lock().unwrap().compression_quality = original_quality;
        result?;
        if let Some(data) = crate::platform::platform().read_file_provenance(&self.archive).map_err(|e| e.to_string())? {
            crate::platform::platform().write_file_provenance(&destination, &data).map_err(|e| e.to_string())?;
        }
        FileOperationManager::archive_checkpoint(op)?;
        let metadata = std::fs::metadata(&self.archive).map_err(|e| e.to_string())?;
        if metadata.len() != self.length || metadata.modified().ok() != self.modified {
            return Err("Archive changed during the operation; original preserved".into());
        }
        let backup = parent.join(format!(".oxyde-{}.archive-backup", uuid::Uuid::new_v4()));
        std::fs::rename(&self.archive, &backup).map_err(|e| e.to_string())?;
        if let Err(error) = std::fs::rename(&destination, &self.archive) {
            return match std::fs::rename(&backup, &self.archive) {
                Ok(()) => Err(error.to_string()),
                Err(restore) => Err(format!(
                    "{error}; original kept at {}: {restore}",
                    backup.display()
                )),
            };
        }
        let _ = std::fs::remove_file(backup);
        Ok(())
    }
}

#[derive(Default)]
pub(crate) struct ArchiveWorkspaces(pub HashMap<PathBuf, ArchiveWorkspace>);
impl ArchiveWorkspaces {
    pub fn resolve(&mut self, path: &Path, destination: bool) -> Result<PathBuf, String> {
        self.resolve_controlled(path, destination, None)
    }
    pub fn resolve_for_operation(
        &mut self,
        path: &Path,
        destination: bool,
        op: &Arc<Mutex<FileOperation>>,
    ) -> Result<PathBuf, String> {
        self.resolve_controlled(path, destination, Some(op))
    }
    fn resolve_controlled(
        &mut self,
        path: &Path,
        destination: bool,
        op: Option<&Arc<Mutex<FileOperation>>>,
    ) -> Result<PathBuf, String> {
        let virtual_path = if destination {
            virtual_destination(path)
        } else {
            virtual_source(path)
        };
        let Some((archive, internal)) = virtual_path else {
            return Ok(path.to_path_buf());
        };
        let key = std::fs::canonicalize(&archive).map_err(|e| e.to_string())?;
        if !self.0.contains_key(&key) {
            let workspace = if let Some(op) = op {
                ArchiveWorkspace::open_for_operation(archive.clone(), op)?
            } else {
                ArchiveWorkspace::open(archive.clone())?
            };
            self.0.insert(key.clone(), workspace);
        }
        self.0[&key].path(&internal)
    }
    pub fn display(&self, path: &Path) -> PathBuf {
        for workspace in self.0.values() {
            if let Ok(relative) = path.strip_prefix(workspace.root()) {
                return workspace.archive.join(relative);
            }
        }
        path.to_path_buf()
    }
}

pub(crate) fn mutation_operation(quality: &str) -> Arc<Mutex<FileOperation>> {
    let mut operation = FileOperation::new(FileOpType::Compress, vec![], None, None);
    operation.compression_quality = quality.into();
    Arc::new(Mutex::new(operation))
}

pub fn create_directory(path: &Path, quality: &str) -> Result<(), std::io::Error> {
    let Some((archive, internal)) = virtual_source(path) else {
        return std::fs::create_dir(path);
    };
    let locks = mutation_locks([archive.clone()]).map_err(std::io::Error::other)?;
    let _lock = locks[0]
        .lock()
        .map_err(|e| std::io::Error::other(e.to_string()))?;
    if !writable(&archive) {
        return Err(std::io::Error::other("This archive format is read-only"));
    }
    let workspace = ArchiveWorkspace::open(archive).map_err(std::io::Error::other)?;
    let destination = workspace.path(&internal).map_err(std::io::Error::other)?;
    std::fs::create_dir(destination)?;
    workspace
        .commit(&mutation_operation(quality))
        .map_err(std::io::Error::other)
}

/// Opens a file from an archive using a cache that survives the shell launch.
/// A new extraction gets its own directory; applications can keep files open.
pub fn external_file(path: &Path) -> Result<PathBuf, String> {
    let Some((archive, internal)) = virtual_source(path) else {
        return Ok(path.to_path_buf());
    };
    let workspace = ArchiveWorkspace::open(archive)?;
    let source = workspace.path(&internal)?;
    if !source.is_file() {
        return Err("Archive item is not a file".into());
    }
    let cache = std::env::temp_dir().join("oxyde-archive-open");
    std::fs::create_dir_all(&cache).map_err(|e| e.to_string())?;
    let directory = tempfile::tempdir_in(cache).map_err(|e| e.to_string())?;
    let destination = directory
        .path()
        .join(source.file_name().ok_or("Invalid file name")?);
    std::fs::copy(source, &destination).map_err(|e| e.to_string())?;
    if let Some(data) = crate::platform::platform().read_file_provenance(&workspace.archive).map_err(|e| e.to_string())? {
        crate::platform::platform().write_file_provenance(&destination, &data).map_err(|e| e.to_string())?;
    }
    let _ = directory.keep();
    Ok(destination)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mutation_locks_are_shared_only_for_the_same_archive() {
        let directory = tempfile::tempdir().unwrap();
        let first = directory.path().join("first.zip");
        let second = directory.path().join("second.zip");
        std::fs::write(&first, b"first").unwrap();
        std::fs::write(&second, b"second").unwrap();
        let first_locks = mutation_locks([first.clone()]).unwrap();
        let same_locks = mutation_locks([first]).unwrap();
        let independent_locks = mutation_locks([second]).unwrap();
        assert!(Arc::ptr_eq(&first_locks[0], &same_locks[0]));
        assert!(!Arc::ptr_eq(&first_locks[0], &independent_locks[0]));
        let _guard = first_locks[0].lock().unwrap();
        assert!(independent_locks[0].try_lock().is_ok());
        let operation = mutation_operation("normal");
        operation
            .lock()
            .unwrap()
            .cancel_flag
            .store(true, std::sync::atomic::Ordering::Relaxed);
        assert!(lock_for_operation(&same_locks[0], &operation).is_err());
    }

    #[test]
    fn workspace_rejects_escaping_paths_and_preserves_externally_changed_archives() {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("source.txt");
        std::fs::write(&source, b"source").unwrap();
        let archive = directory.path().join("test.tar.zst");
        FileOperationManager::perform_compress(
            &mutation_operation("normal"),
            vec![source],
            Some(archive.clone()),
        )
        .unwrap();
        let workspace = ArchiveWorkspace::open(archive.clone()).unwrap();
        for path in [
            "../escape",
            "/absolute",
            "C:/absolute",
            "folder/../../escape",
        ] {
            assert!(workspace.path(path).is_err());
        }
        std::fs::write(&archive, b"changed externally").unwrap();
        assert!(workspace.commit(&mutation_operation("normal")).is_err());
        assert_eq!(std::fs::read(&archive).unwrap(), b"changed externally");
    }

    #[test]
    fn cancelled_rebuild_keeps_original_archive() {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("source.txt");
        std::fs::write(&source, b"source").unwrap();
        let archive = directory.path().join("test.7z");
        FileOperationManager::perform_compress(
            &mutation_operation("normal"),
            vec![source],
            Some(archive.clone()),
        )
        .unwrap();
        let original = std::fs::read(&archive).unwrap();
        let workspace = ArchiveWorkspace::open(archive.clone()).unwrap();
        std::fs::write(workspace.root().join("added.txt"), b"added").unwrap();
        let operation = mutation_operation("normal");
        operation
            .lock()
            .unwrap()
            .cancel_flag
            .store(true, std::sync::atomic::Ordering::Relaxed);
        assert!(workspace.commit(&operation).is_err());
        assert_eq!(std::fs::read(&archive).unwrap(), original);
    }
}
