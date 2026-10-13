//! Temporary filesystem views used to edit archives without touching the original.
#[cfg(all(target_os = "windows", not(test)))]
#[path = "archive_workspace/native_commit.rs"]
mod native_commit;
#[cfg(any(not(target_os = "windows"), test))]
use crate::commands::archive::{
    extract_iso_file_for_open, extract_seven_file_for_open,
    extract_tar_file_for_open, extract_zip_file_for_open,
};
#[cfg(any(not(target_os = "windows"), test))]
use crate::commands::archive::extract_open_archive_with_budget;
use crate::systems::file_ops::{FileOpType, FileOperation, FileOperationManager};
use crate::utils::archive::{split_virtual_path, ArchiveFormat};
use crate::utils::archive_limits::{ArchiveBudget, ArchiveLimits};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock, Mutex, MutexGuard, Weak};

type MutationMutex = Arc<Mutex<()>>;
pub(crate) fn commit_group(workspaces: &ArchiveWorkspaces, archives: &[PathBuf], op: &Arc<Mutex<FileOperation>>, retained: &HashMap<PathBuf, std::fs::File>) -> Result<(), String> {
    #[cfg(all(target_os = "windows", not(test)))]
    {
        let mut prepared = archives.iter().map(|archive| workspaces.0[archive].prepare_native(op, retained)).collect::<Result<Vec<_>, _>>()?;
        native_commit::publish_all(&mut prepared, op)
    }
    #[cfg(any(not(target_os = "windows"), test))]
    {
        for archive in archives { workspaces.0[archive].commit_with_retained(op, retained)?; }
        Ok(())
    }
}
#[cfg(any(not(target_os = "windows"), test))]
type WorkspaceDirectory = tempfile::TempDir;
#[cfg(all(target_os = "windows", not(test)))]
struct WorkspaceDirectory {
    path: PathBuf,
    owner: Arc<Mutex<super::archive_owned_tree::OwnedTree>>,
}
#[cfg(all(target_os = "windows", not(test)))]
impl WorkspaceDirectory {
    fn new(owner: super::archive_owned_tree::OwnedTree) -> Self { Self { path: owner.path().into(), owner: Arc::new(Mutex::new(owner)) } }
    fn path(&self) -> &Path { &self.path }
}
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
    directory: WorkspaceDirectory,
    length: u64,
    modified: Option<std::time::SystemTime>,
    original_identity: crate::platform::FileIdentity,
    original_hash: blake3::Hash,
    original_provenance: Option<Vec<u8>>,
    // Keep the original object's identity alive without blocking later edits.
    _original_file: std::fs::File,
    _parents: super::path_parents::HeldParents,
    #[cfg(all(target_os = "windows", not(test)))]
    rename: Mutex<Option<(PathBuf, PathBuf)>>,
}

impl ArchiveWorkspace {
    pub fn open(archive: PathBuf) -> Result<Self, String> {
        Self::open_with_checkpoint(archive, &|| Ok(()), ArchiveLimits::WORKSPACE)
    }
    pub fn open_for_operation(
        archive: PathBuf,
        op: &Arc<Mutex<FileOperation>>,
    ) -> Result<Self, String> {
        Self::open_controlled(
            archive,
            &|| FileOperationManager::archive_checkpoint(op).map_err(std::io::Error::other),
            ArchiveLimits::WORKSPACE,
            Some(op),
        )
    }
    fn open_with_checkpoint(
        archive: PathBuf,
        checkpoint: &dyn Fn() -> std::io::Result<()>,
        limits: ArchiveLimits,
    ) -> Result<Self, String> {
        Self::open_controlled(archive, checkpoint, limits, None)
    }
    fn open_controlled(archive: PathBuf, checkpoint: &dyn Fn() -> std::io::Result<()>, limits: ArchiveLimits, op: Option<&Arc<Mutex<FileOperation>>>) -> Result<Self, String> {
        #[cfg(any(not(target_os = "windows"), test))]
        let directory = tempfile::tempdir().map_err(|e| e.to_string())?;
        #[cfg(all(target_os = "windows", not(test)))]
        let directory = WorkspaceDirectory::new(super::archive_owned_tree::OwnedTree::temporary(&std::env::temp_dir()).map_err(|e| e.to_string())?);
        Self::open_in_directory_controlled(archive, checkpoint, limits, directory, op)
    }

    #[cfg(any(not(target_os = "windows"), test))]
    fn open_in_directory(
        archive: PathBuf,
        checkpoint: &dyn Fn() -> std::io::Result<()>,
        limits: ArchiveLimits,
        directory: WorkspaceDirectory,
    ) -> Result<Self, String> {
        Self::open_in_directory_controlled(archive, checkpoint, limits, directory, None)
    }
    fn open_in_directory_controlled(archive: PathBuf, checkpoint: &dyn Fn() -> std::io::Result<()>, limits: ArchiveLimits, directory: WorkspaceDirectory, _op: Option<&Arc<Mutex<FileOperation>>>) -> Result<Self, String> {
        let archive = crate::utils::path_security::validate_path(&archive.to_string_lossy()).map_err(|e| e.to_string())?;
        let display_archive = archive.clone();
        let parents = super::path_parents::HeldParents::new(&archive).map_err(|e| e.to_string())?;
        let archive = parents.resolved_path(&archive).map_err(|e| e.to_string())?;
        let budget = ArchiveBudget::new(limits);
        budget.checkpoint(checkpoint).map_err(|e| e.to_string())?;
        let mut source = crate::platform::platform().open_archive_source(&archive, false).map_err(|e| e.to_string())?;
        let metadata = source.metadata().map_err(|e| e.to_string())?;
        let original_identity = crate::platform::platform().file_identity(&source).map_err(|e| e.to_string())?;
        let original_hash = fingerprint(&mut source, checkpoint, &budget).map_err(|e| e.to_string())?;
        let original_provenance = crate::platform::platform().read_archive_output_provenance(parents.directory(), &source, &archive).map_err(|e| e.to_string())?;
        use std::io::{Seek, SeekFrom};
        source.seek(SeekFrom::Start(0)).map_err(|e| e.to_string())?;
        #[cfg(any(not(target_os = "windows"), test))]
        extract_open_archive_with_budget(
            &archive,
            source.try_clone().map_err(|e| e.to_string())?,
            &directory.path().to_string_lossy(),
            checkpoint,
            &budget,
        )
        .map_err(|e| e.to_string())?;
        #[cfg(all(target_os = "windows", not(test)))]
        super::archive_io::decode(&archive, source.try_clone().map_err(|e| e.to_string())?, &mut directory.owner.lock().unwrap(), None, &budget, _op).map_err(|e| e.to_string())?;
        // Never follow archive-provided links when copying or rebuilding a workspace.
        for entry in walkdir::WalkDir::new(directory.path()).follow_links(false) {
            if entry.map_err(|e| e.to_string())?.file_type().is_symlink() {
                return Err("Archive links are not supported for file operations".into());
            }
        }
        let original_file = crate::platform::platform().open_archive_source(&archive, true).map_err(|e| e.to_string())?;
        // The decoder used the held source, even if a parent was retargeted.
        if crate::platform::platform().file_identity(&original_file).map_err(|e| e.to_string())? != original_identity {
            return Err(crate::models::error::ArchiveSourceConflict.to_string());
        }
        // Windows protects source data for the initial read. On other systems
        // validate it again before accepting the temporary view.
        if fingerprint(&mut source, checkpoint, &budget).map_err(|e| e.to_string())? != original_hash
            || crate::platform::platform().read_archive_output_provenance(parents.directory(), &source, &archive).map_err(|e| e.to_string())? != original_provenance {
            return Err(crate::models::error::ArchiveSourceConflict.to_string());
        }
        #[cfg(all(target_os = "windows", not(test)))]
        directory.owner.lock().unwrap().release();
        Ok(Self {
            archive: display_archive,
            directory,
            length: metadata.len(),
            modified: metadata.modified().ok(),
            original_identity,
            original_hash,
            original_provenance,
            _original_file: original_file,
            _parents: parents,
            #[cfg(all(target_os = "windows", not(test)))]
            rename: Mutex::new(None),
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
    #[cfg(all(target_os = "windows", not(test)))]
    pub fn cleanup_owner(&self) -> std::sync::Weak<Mutex<super::archive_owned_tree::OwnedTree>> { Arc::downgrade(&self.directory.owner) }
    #[cfg(all(target_os = "windows", not(test)))]
    pub fn forget_removed(&self, path: &Path, identity: crate::platform::FileIdentity) -> std::io::Result<()> { self.directory.owner.lock().unwrap().forget_removed(path, identity) }
    pub fn create_directory(&self, internal: &str) -> Result<(), String> {
        let path = self.path(internal)?;
        #[cfg(all(target_os = "windows", not(test)))]
        { self.directory.owner.lock().unwrap().create_directory(&path).map_err(|e| e.to_string()) }
        #[cfg(any(not(target_os = "windows"), test))]
        { std::fs::create_dir(path).map_err(|e| e.to_string()) }
    }
    pub fn rename(&self, source: &Path, destination: &Path, checkpoint: &mut dyn FnMut() -> std::io::Result<()>) -> std::io::Result<()> {
        #[cfg(all(target_os = "windows", not(test)))]
        {
            self.directory.owner.lock().unwrap().validate_rename(source, destination, checkpoint)?;
            let mut rename = self.rename.lock().unwrap();
            if rename.is_some() { return Err(std::io::Error::other("Workspace rename already prepared")); }
            *rename = Some((source.strip_prefix(self.root()).map_err(std::io::Error::other)?.into(), destination.strip_prefix(self.root()).map_err(std::io::Error::other)?.into()));
            Ok(())
        }
        #[cfg(any(not(target_os = "windows"), test))]
        { super::item_rename::rename_with_checkpoint(source, destination, checkpoint) }
    }
    pub fn remove(&self, path: &Path, checkpoint: &mut dyn FnMut() -> std::io::Result<()>) -> std::io::Result<()> {
        #[cfg(all(target_os = "windows", not(test)))]
        { self.directory.owner.lock().unwrap().remove_selected(path, checkpoint) }
        #[cfg(any(not(target_os = "windows"), test))]
        { checkpoint()?; if path.is_dir() { std::fs::remove_dir_all(path) } else if path.exists() { std::fs::remove_file(path) } else { Ok(()) } }
    }

    pub fn commit(&self, op: &Arc<Mutex<FileOperation>>) -> Result<(), String> {
        self.commit_with_retained(op, &HashMap::new())
    }
    pub fn commit_with_retained(&self, op: &Arc<Mutex<FileOperation>>, retained: &HashMap<PathBuf, std::fs::File>) -> Result<(), String> {
        #[cfg(all(target_os = "windows", not(test)))]
        { return self.commit_native(op, retained); }
        #[cfg(any(not(target_os = "windows"), test))]
        {
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
        let result = FileOperationManager::perform_compress_retained(op, sources, Some(destination.clone()), retained);
        op.lock().unwrap().destination = original_destination;
        op.lock().unwrap().compression_quality = original_quality;
        result?;
        FileOperationManager::archive_checkpoint(op)?;
        let mut original = crate::platform::platform()
            .open_file_for_move(&self.archive)
            .map_err(|e| e.to_string())?;
        let metadata = original.metadata().map_err(|e| e.to_string())?;
        if !metadata.is_file() || crate::platform::platform().file_attributes(&metadata, "").2
            || metadata.len() != self.length || metadata.modified().ok() != self.modified
            || crate::platform::platform().file_identity(&original).map_err(|e| e.to_string())? != self.original_identity
            || fingerprint(&mut original, &|| FileOperationManager::archive_checkpoint(op).map_err(std::io::Error::other), &ArchiveBudget::new(ArchiveLimits::WORKSPACE)).map_err(|e| e.to_string())? != self.original_hash
            || crate::platform::platform().read_file_provenance(&self.archive).map_err(|e| e.to_string())? != self.original_provenance {
            return Err(crate::models::error::ArchiveSourceConflict.to_string());
        }
        if let Some(data) = crate::platform::platform()
            .read_file_provenance(&self.archive)
            .map_err(|e| e.to_string())?
        {
            crate::platform::platform()
                .write_file_provenance(&destination, &data)
                .map_err(|e| e.to_string())?;
        }
        FileOperationManager::archive_checkpoint(op)?;
        let backup = parent.join(format!(".oxyde-{}.archive-backup", uuid::Uuid::new_v4()));
        let rebuilt = crate::platform::platform()
            .open_file_for_move(&destination)
            .map_err(|e| e.to_string())?;
        crate::platform::platform()
            .rename_open_file_exclusive(&original, &self.archive, &backup)
            .map_err(|e| e.to_string())?;
        if let Err(error) = crate::platform::platform().rename_open_file_exclusive(
            &rebuilt,
            &destination,
            &self.archive,
        ) {
            return match crate::platform::platform().rename_open_file_exclusive(
                &original,
                &backup,
                &self.archive,
            ) {
                Ok(()) => Err(error.to_string()),
                Err(restore) => Err(format!(
                    "{error}; original kept at {}: {restore}",
                    backup.display()
                )),
            };
        }
        if let Err(error) = crate::platform::platform().remove_open_file(&original, &backup) {
            log::warn!(
                "Archive updated, original backup retained at {}: {error}",
                backup.display()
            );
        }
        Ok(())
        }
    }


}

/// Streaming, cancellable fingerprinting on the same opened file used for
/// verification. Hashing never allocates proportionally to archive size.
fn fingerprint(
    file: &mut std::fs::File,
    checkpoint: &dyn Fn() -> std::io::Result<()>,
    budget: &ArchiveBudget,
) -> std::io::Result<blake3::Hash> {
    use std::io::{Read, Seek, SeekFrom};
    file.seek(SeekFrom::Start(0))?;
    let mut hasher = blake3::Hasher::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        budget.checkpoint(checkpoint)?;
        let count = file.read(&mut buffer)?;
        budget.check_time()?;
        if count == 0 {return Ok(hasher.finalize());}
        hasher.update(&buffer[..count]);
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
    super::archive_recovery::scope(|| create_directory_inner(path, quality))
}
fn create_directory_inner(path: &Path, quality: &str) -> Result<(), std::io::Error> {
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
    workspace.create_directory(&internal).map_err(std::io::Error::other)?;
    workspace
        .commit(&mutation_operation(quality))
        .map_err(std::io::Error::other)
}

/// Opens a file from an archive using a cache that survives the shell launch.
/// A new extraction gets its own directory; applications can keep files open.
pub fn external_file(path: &Path) -> Result<PathBuf, crate::models::CommandError> {
    super::archive_recovery::scope(|| external_file_inner(path))
}
fn external_file_inner(path: &Path) -> Result<PathBuf, crate::models::CommandError> {
    let Some((archive, internal)) = virtual_source(path) else { return Ok(path.to_path_buf()); };
    let cache = super::archive_open_cache::CacheEntry::new()?;
    #[cfg(all(target_os = "windows", not(test)))]
    let result = {
        let mut owner = super::archive_owned_tree::OwnedTree::new(cache.payload(), false)?;
        cache.attach_recovery(&mut owner);
        let destination = prepare_selection_owned(archive, &internal, &mut owner, ArchiveLimits::OPEN_FILE).map_err(crate::models::CommandError::ArchiveError)?;
        cache.finish_owned(destination, owner)
    };
    #[cfg(any(not(target_os = "windows"), test))]
    let result = {
    let destination = prepare_selection(archive, &internal, cache.payload()).map_err(crate::models::CommandError::ArchiveError)?;
    cache.finish(destination)
    };
    result.map_err(|error| {
        log::warn!("Archive cache admission failed: {error}");
        let issue = if error.kind() == std::io::ErrorKind::WouldBlock {
            crate::models::error::ArchiveCacheIssue::Busy
        } else {
            error.get_ref().and_then(|e| e.downcast_ref::<crate::models::error::ArchiveCacheIssue>()).copied()
                .unwrap_or(crate::models::error::ArchiveCacheIssue::Unavailable)
        };
        crate::models::CommandError::ArchiveCache(issue)
    })
}

#[cfg(any(not(target_os = "windows"), test))]
fn prepare_selection(archive: PathBuf, internal: &str, target: &Path) -> Result<PathBuf, String> {
    prepare_selection_with_limits(archive, internal, target, ArchiveLimits::OPEN_FILE)
}

#[cfg(any(not(target_os = "windows"), test))]
fn prepare_selection_with_limits(
    archive: PathBuf,
    internal: &str,
    target: &Path,
    limits: ArchiveLimits,
) -> Result<PathBuf, String> {
    Ok(if ArchiveFormat::from_path(&archive) == Some(ArchiveFormat::Zip) {
        extract_zip_file_for_open(&archive, internal, target, limits).map_err(|e| e.to_string())?
    } else if ArchiveFormat::from_path(&archive) == Some(ArchiveFormat::Iso) {
        extract_iso_file_for_open(&archive, internal, target, limits).map_err(|e| e.to_string())?
    } else if ArchiveFormat::from_path(&archive) == Some(ArchiveFormat::SevenZip) {
        extract_seven_file_for_open(&archive, internal, target, limits).map_err(|e| e.to_string())?
    } else if matches!(
        ArchiveFormat::from_path(&archive),
        Some(
            ArchiveFormat::Tar
                | ArchiveFormat::TarGz
                | ArchiveFormat::TarXz
                | ArchiveFormat::TarBz2
                | ArchiveFormat::TarZst
        )
    ) {
        extract_tar_file_for_open(&archive, internal, target, limits).map_err(|e| e.to_string())?
    } else {
        #[cfg(any(not(target_os = "windows"), test))]
        let directory = tempfile::Builder::new().prefix(".oxyde-work-").tempdir_in(target).map_err(|e| e.to_string())?;
        #[cfg(all(target_os = "windows", not(test)))]
        let directory = super::archive_owned_tree::OwnedTree::temporary(target).map_err(|e| e.to_string())?;
        let workspace = ArchiveWorkspace::open_in_directory(archive, &|| Ok(()), limits, directory)?;
        let source = workspace.path(&internal)?;
        if !source.is_file() {
            return Err("Archive item is not a file".into());
        }
        let root = crate::utils::archive_security::ExtractionRoot::new(target)
            .map_err(|e| e.to_string())?;
        let name = source
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or("Invalid file name")?;
        let destination = root.destination(name, false).map_err(|e| e.to_string())?;
        let mut input = std::fs::File::open(source).map_err(|e| e.to_string())?;
        let mut output = crate::platform::platform()
            .create_archive_file(&destination.path)
            .map_err(|e| e.to_string())?;
        ArchiveBudget::new(limits)
            .copy(&mut input, &mut output, &|| Ok(()))
            .map_err(|e| e.to_string())?;
        if let Some(data) = crate::platform::platform()
            .read_file_provenance(&workspace.archive)
            .map_err(|e| e.to_string())?
        {
            crate::platform::platform()
                .write_file_provenance(&destination.path, &data)
                .map_err(|e| e.to_string())?;
        }
        destination.path.clone()
    })
}

/// Explicit fallback chosen in Oxyde: publish a permanent copy without using
/// the managed cache or overwriting an existing destination.
#[cfg(all(target_os = "windows", not(test)))]
fn prepare_selection_owned(archive: PathBuf, internal: &str, target: &mut super::archive_owned_tree::OwnedTree, limits: ArchiveLimits) -> Result<PathBuf, String> {
    let requested = crate::utils::archive_security::relative_archive_path(internal).map_err(|e| e.to_string())?;
    if requested.as_os_str().is_empty() { return Err("Archive item is not a file".into()); }
    let parents = super::path_parents::HeldParents::new(&archive).map_err(|e| e.to_string())?;
    let source = parents.resolved_path(&archive).map_err(|e| e.to_string())?;
    let file = crate::platform::platform().open_archive_source(&source, false).map_err(|e| e.to_string())?;
    super::archive_io::decode(&source, file, target, Some(internal), &ArchiveBudget::new(limits), None).map_err(|e| e.to_string())?;
    Ok(target.path().join(requested))
}

pub fn export_file(path: &Path, folder: &Path) -> Result<PathBuf, crate::models::CommandError> {
    super::archive_recovery::scope(|| export_file_inner(path, folder))
}
fn export_file_inner(path: &Path, folder: &Path) -> Result<PathBuf, crate::models::CommandError> {
    use crate::models::CommandError;
    let (archive, internal) = virtual_source(path).ok_or_else(|| CommandError::ArchiveError("Archive file not found".into()))?;
    let _folder_guard = super::path_parents::HeldParents::new(&folder.join(".oxyde-extract"))?;
    #[cfg(any(not(target_os = "windows"), test))]
    let staging = tempfile::Builder::new().prefix(".oxyde-extract-").tempdir_in(folder)?;
    #[cfg(all(target_os = "windows", not(test)))]
    let mut staging = super::archive_owned_tree::OwnedTree::temporary(folder)?;
    #[cfg(all(target_os = "windows", not(test)))]
    let selected = prepare_selection_owned(archive, &internal, &mut staging, ArchiveLimits::OPEN_FILE).map_err(CommandError::ArchiveError)?;
    #[cfg(any(not(target_os = "windows"), test))]
    let selected = prepare_selection(archive, &internal, staging.path()).map_err(CommandError::ArchiveError)?;
    let name = selected.file_name().ok_or_else(|| CommandError::PathError("File has no name".into()))?;
    let stem = selected.file_stem().unwrap_or(name).to_string_lossy();
    for index in 0..1000 {
        let destination = if index == 0 { folder.join(name) } else {
            let mut name = format!("{stem} ({index})");
            if let Some(extension) = selected.extension() { name.push('.'); name.push_str(&extension.to_string_lossy()); }
            folder.join(name)
        };
        match super::file_copy::copy(&selected, &destination, false, &mut |_| Ok(())) {
            Ok(_) => return Ok(destination),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error.into()),
        }
    }
    Err(CommandError::PathError("No unused destination name was available".into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn targeted_seven_selection_bypasses_full_workspace_and_exports_without_overwriting() {
        use sevenz_rust2::{ArchiveEntry, ArchiveWriter, SourceReader};
        let fixture = tempfile::tempdir().unwrap();
        for solid in [false, true] {
            let archive = fixture.path().join(format!("selection-{solid}.7z"));
            let mut writer = ArchiveWriter::create(&archive).unwrap();
            if solid {
                writer.push_archive_entries(
                    vec![ArchiveEntry::new_file("selected.txt"), ArchiveEntry::new_file("other.txt")],
                    vec![SourceReader::new(&b"safe"[..]), SourceReader::new(&b"unselected file exceeds four bytes"[..])],
                ).unwrap();
            } else {
                writer.push_archive_entry(ArchiveEntry::new_file("selected.txt"), Some(&b"safe"[..])).unwrap();
                writer.push_archive_entry(ArchiveEntry::new_file("other.txt"), Some(&b"unselected file exceeds four bytes"[..])).unwrap();
            }
            writer.finish().unwrap();
            let limits = ArchiveLimits { max_bytes: 4, ..ArchiveLimits::OPEN_FILE };
            let full = tempfile::tempdir().unwrap();
            assert!(ArchiveWorkspace::open_in_directory(archive.clone(), &|| Ok(()), limits, full).is_err());
            let target = tempfile::tempdir().unwrap();
            let selected = prepare_selection_with_limits(archive.clone(), "selected.txt", target.path(), limits).unwrap();
            assert_eq!(std::fs::read(selected).unwrap(), b"safe");
            assert_eq!(std::fs::read_dir(target.path()).unwrap().count(), 1);
            let output = tempfile::tempdir().unwrap();
            std::fs::write(output.path().join("selected.txt"), b"foreign").unwrap();
            let exported = export_file(&archive.join("selected.txt"), output.path()).unwrap();
            assert_eq!(exported.file_name().unwrap(), "selected (1).txt");
            assert_eq!(std::fs::read(exported).unwrap(), b"safe");
            assert_eq!(std::fs::read(output.path().join("selected.txt")).unwrap(), b"foreign");
        }
    }

    #[test]
    fn targeted_iso_selection_bypasses_full_workspace_and_exports_without_overwriting() {
        let fixture = tempfile::tempdir().unwrap();
        let archive = fixture.path().join("disc.iso");
        crate::commands::archive::test_iso_image(&archive);
        let limits = ArchiveLimits { max_bytes: 4, ..ArchiveLimits::OPEN_FILE };
        let full = tempfile::tempdir().unwrap();
        assert!(ArchiveWorkspace::open_in_directory(archive.clone(), &|| Ok(()), limits, full).is_err());
        let target = tempfile::tempdir().unwrap();
        let selected = prepare_selection_with_limits(archive.clone(), "ROOT.TXT", target.path(), limits).unwrap();
        assert_eq!(std::fs::read(selected).unwrap(), b"root");
        assert_eq!(std::fs::read_dir(target.path()).unwrap().count(), 1);
        let output = tempfile::tempdir().unwrap();
        std::fs::write(output.path().join("ROOT.TXT"), b"foreign").unwrap();
        let exported = export_file(&archive.join("ROOT.TXT"), output.path()).unwrap();
        assert_eq!(exported.file_name().unwrap(), "ROOT (1).TXT");
        assert_eq!(std::fs::read(exported).unwrap(), b"root");
        assert_eq!(std::fs::read(output.path().join("ROOT.TXT")).unwrap(), b"foreign");
    }

    #[test]
    fn targeted_tar_selection_bypasses_full_workspace_and_exports_without_overwriting() {
        let fixture = tempfile::tempdir().unwrap();
        let selected = fixture.path().join("selected.txt");
        let other = fixture.path().join("other.txt");
        std::fs::write(&selected, b"safe").unwrap();
        std::fs::write(&other, b"unselected file exceeds four bytes").unwrap();
        let limits = ArchiveLimits {
            max_bytes: 4,
            max_entries: 4,
            ..ArchiveLimits::OPEN_FILE
        };
        let plain_archive = fixture.path().join("source.tar");
        FileOperationManager::perform_compress(
            &mutation_operation("fast"),
            vec![selected.clone(), other.clone()],
            Some(plain_archive.clone()),
        ).unwrap();
        let plain_bytes = std::fs::read(&plain_archive).unwrap();
        for extension in ["tar", "tar.zst", "tar.gz", "tar.xz", "tar.bz2", "tgz", "txz", "tbz2"] {
            let archive = fixture.path().join(format!("selection.{extension}"));
            let mut reader: Box<dyn std::io::Read + '_> = match ArchiveFormat::from_path(&archive).unwrap() {
                ArchiveFormat::Tar => Box::new(&plain_bytes[..]),
                ArchiveFormat::TarGz => Box::new(flate2::read::GzEncoder::new(
                    &plain_bytes[..], flate2::Compression::fast(),
                )),
                ArchiveFormat::TarXz => Box::new(xz2::read::XzEncoder::new(&plain_bytes[..], 1)),
                ArchiveFormat::TarBz2 => Box::new(bzip2::read::BzEncoder::new(
                    &plain_bytes[..], bzip2::Compression::fast(),
                )),
                ArchiveFormat::TarZst => Box::new(std::io::Cursor::new(
                    zstd::stream::encode_all(&plain_bytes[..], 1).unwrap(),
                )),
                _ => unreachable!(),
            };
            let mut contents = Vec::new();
            reader.read_to_end(&mut contents).unwrap();
            std::fs::write(&archive, contents).unwrap();
            let full = tempfile::tempdir().unwrap();
            assert!(ArchiveWorkspace::open_in_directory(
                archive.clone(), &|| Ok(()), limits, full,
            ).is_err());
            let target = tempfile::tempdir().unwrap();
            let result = prepare_selection_with_limits(
                archive.clone(), "selected.txt", target.path(), limits,
            ).unwrap();
            assert_eq!(std::fs::read(&result).unwrap(), b"safe");
            assert_eq!(std::fs::read_dir(target.path()).unwrap().count(), 1);
            let output = tempfile::tempdir().unwrap();
            std::fs::write(output.path().join("selected.txt"), b"foreign content").unwrap();
            let permanent = export_file(&archive.join("selected.txt"), output.path()).unwrap();
            assert_eq!(permanent.file_name().unwrap(), "selected (1).txt");
            assert_eq!(std::fs::read(permanent).unwrap(), b"safe");
            assert_eq!(std::fs::read(output.path().join("selected.txt")).unwrap(), b"foreign content");
            assert_eq!(std::fs::read_dir(output.path()).unwrap().count(), 2);
        }
    }

    fn zip_bytes(contents: &[u8]) -> Vec<u8> {
        use std::io::Write;
        let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        zip.start_file("document.txt", zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored)).unwrap();
        zip.write_all(contents).unwrap();
        zip.finish().unwrap().into_inner()
    }

    #[test]
    fn fingerprinting_is_cancellable_after_a_partial_read() {
        use std::io::Seek;
        let mut file = tempfile::tempfile().unwrap();
        use std::io::Write;
        file.write_all(&vec![42u8; 128 * 1024]).unwrap();
        let calls = std::cell::Cell::new(0);
        let error = fingerprint(&mut file, &|| {
            calls.set(calls.get() + 1);
            if calls.get() > 1 {Err(std::io::Error::new(std::io::ErrorKind::Interrupted, "cancelled"))} else {Ok(())}
        }, &ArchiveBudget::new(ArchiveLimits::WORKSPACE)).unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::Interrupted);
        assert_eq!(file.stream_position().unwrap(), 64 * 1024);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn initial_archive_read_blocks_writers_and_replacement_until_the_view_is_ready() {
        let base = tempfile::tempdir().unwrap();
        let archive = base.path().join("archive.zip");
        std::fs::write(&archive, zip_bytes(b"before")).unwrap();
        let attempted = std::cell::Cell::new(false);
        let workspace = ArchiveWorkspace::open_with_checkpoint(archive.clone(), &|| {
            // The first checkpoint precedes opening the source. Test once the
            // source is protected, during the first fingerprint read.
            if !attempted.get() && std::fs::File::options().write(true).open(&archive).is_err() {
                attempted.set(true);
                assert!(std::fs::rename(&archive, base.path().join("redirected.zip")).is_err());
            }
            Ok(())
        }, ArchiveLimits::WORKSPACE).unwrap();
        assert!(attempted.get());
        assert_eq!(std::fs::read(workspace.root().join("document.txt")).unwrap(), b"before");
        std::fs::write(&archive, zip_bytes(b"edited")).unwrap();
    }

    #[test]
    fn extraction_decodes_the_held_file_after_its_path_is_replaced() {
        let base = tempfile::tempdir().unwrap();
        let source = base.path().join("document.txt");
        std::fs::write(&source, b"held source").unwrap();
        for extension in ["zip", "7z", "tar", "tar.zst"] {
            let archive = base.path().join(format!("archive.{extension}"));
            FileOperationManager::perform_compress(&mutation_operation("fast"), vec![source.clone()], Some(archive.clone())).unwrap();
            let file = crate::platform::platform().open_archive_source(&archive, true).unwrap();
            std::fs::rename(&archive, base.path().join(format!("moved.{extension}"))).unwrap();
            std::fs::write(&archive, b"replacement is not an archive").unwrap();
            let target = base.path().join(format!("target-{extension}"));
            extract_open_archive_with_budget(&archive, file, &target.to_string_lossy(), &|| Ok(()), &ArchiveBudget::new(ArchiveLimits::WORKSPACE)).unwrap();
            assert_eq!(std::fs::read(target.join("document.txt")).unwrap(), b"held source");
        }
    }

    #[test]
    fn publication_preserves_content_edits_with_original_length_and_timestamp() {
        let base = tempfile::tempdir().unwrap();
        let archive = base.path().join("archive.zip");
        let original = zip_bytes(b"before");
        let edited = zip_bytes(b"edited");
        assert_eq!(original.len(), edited.len());
        std::fs::write(&archive, &original).unwrap();
        let modified = std::fs::metadata(&archive).unwrap().modified().unwrap();
        let workspace = ArchiveWorkspace::open(archive.clone()).unwrap();
        std::fs::write(workspace.root().join("added.txt"), b"local addition").unwrap();
        std::fs::write(&archive, &edited).unwrap();
        std::fs::File::options().write(true).open(&archive).unwrap().set_modified(modified).unwrap();
        let error = workspace.commit(&mutation_operation("fast")).unwrap_err();
        assert!(error.contains("Archive changed"), "{error}");
        assert_eq!(std::fs::read(&archive).unwrap(), edited);
    }

    #[test]
    fn publication_preserves_replacement_with_identical_bytes_and_timestamp() {
        let base = tempfile::tempdir().unwrap();
        let archive = base.path().join("archive.zip");
        let original = zip_bytes(b"before");
        std::fs::write(&archive, &original).unwrap();
        let modified = std::fs::metadata(&archive).unwrap().modified().unwrap();
        let workspace = ArchiveWorkspace::open(archive.clone()).unwrap();
        std::fs::write(workspace.root().join("added.txt"), b"local addition").unwrap();
        let replacement = base.path().join("replacement.zip");
        std::fs::write(&replacement, &original).unwrap();
        std::fs::File::options().write(true).open(&replacement).unwrap().set_modified(modified).unwrap();
        let moved_original = base.path().join("moved-original.zip");
        std::fs::rename(&archive, &moved_original).unwrap();
        std::fs::rename(&replacement, &archive).unwrap();
        let error = workspace.commit(&mutation_operation("fast")).unwrap_err();
        assert!(error.contains("Archive changed"), "{error}");
        assert_eq!(std::fs::read(&archive).unwrap(), original);
        assert_eq!(std::fs::read(&moved_original).unwrap(), original);
    }

    #[test]
    fn explicit_export_preserves_existing_names_and_original_archives() {
        let base = tempfile::tempdir().unwrap();
        let source = base.path().join("document.txt");
        std::fs::write(&source, b"archive contents").unwrap();
        let folder = base.path().join("chosen-folder");
        std::fs::create_dir(&folder).unwrap();
        std::fs::write(folder.join("document.txt"), b"existing document").unwrap();
        for extension in ["zip", "tar.zst"] {
            let archive = base.path().join(format!("archive.{extension}"));
            FileOperationManager::perform_compress(&mutation_operation("fast"), vec![source.clone()], Some(archive.clone())).unwrap();
            let original = std::fs::read(&archive).unwrap();
            #[cfg(target_os = "windows")]
            crate::platform::platform().write_file_provenance(&archive, b"[ZoneTransfer]\r\nZoneId=3\r\n").unwrap();
            let virtual_path = archive.join("document.txt");
            let exported = export_file(&virtual_path, &folder).unwrap();
            assert_eq!(exported.parent(), Some(folder.as_path()));
            assert_ne!(exported, folder.join("document.txt"));
            assert_eq!(std::fs::read(&exported).unwrap(), b"archive contents");
            assert_eq!(std::fs::read(folder.join("document.txt")).unwrap(), b"existing document");
            assert_eq!(std::fs::read(&archive).unwrap(), original);
            assert!(std::fs::read_dir(&folder).unwrap().all(|entry| entry.unwrap().file_type().unwrap().is_file()));
            #[cfg(target_os = "windows")]
            assert_eq!(crate::platform::platform().read_file_provenance(&exported).unwrap().unwrap(), b"[ZoneTransfer]\r\nZoneId=3\r\n");
        }
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn archive_parent_remains_fixed_until_workspace_publication_finishes() {
        let base = tempfile::tempdir().unwrap();
        let parent = base.path().join("parent");
        std::fs::create_dir(&parent).unwrap();
        let source = base.path().join("file.txt");
        std::fs::write(&source, b"safe").unwrap();
        let archive = parent.join("archive.zip");
        FileOperationManager::perform_compress(&mutation_operation("fast"), vec![source], Some(archive.clone())).unwrap();
        let workspace = ArchiveWorkspace::open(archive.clone()).unwrap();
        assert!(std::fs::rename(&parent, base.path().join("replaced")).is_err());
        std::fs::write(workspace.root().join("added.txt"), b"added").unwrap();
        workspace.commit(&mutation_operation("fast")).unwrap();
        drop(workspace);
        let listing = crate::commands::archive::list_archive_contents(archive.to_string_lossy().into_owned(), String::new()).unwrap();
        assert!(listing.iter().any(|entry| entry.name == "added.txt"));
        std::fs::rename(parent, base.path().join("released")).unwrap();
    }

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
