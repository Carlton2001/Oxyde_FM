//! Cleanup follows only identities captured at creation; never enumerate to claim ownership.
use super::{destination_tree::DestinationTree, path_parents::HeldParents};
use crate::platform::{platform, FileIdentity};
use std::{
    collections::HashMap,
    fs::File,
    io,
    path::{Path, PathBuf},
};
#[cfg(target_os = "windows")]
use std::sync::{Arc, Mutex};

struct Owned {
    path: PathBuf,
    parent: File,
    tracking: File,
    identity: FileIdentity,
}

pub(crate) struct OwnedTree {
    requested: PathBuf,
    tree: Option<DestinationTree>,
    root: Owned,
    directories: HashMap<PathBuf, Owned>,
    files: HashMap<PathBuf, Owned>,
    name_bytes: usize,
    cleanup: bool,
    remove_root: bool,
    #[cfg(target_os = "windows")]
    recovery: Option<Arc<Mutex<super::archive_open_cache::recovery::Journal>>>,
}

impl std::fmt::Debug for OwnedTree {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OwnedTree")
            .field("path", &self.path())
            .field("files", &self.files.len())
            .field("directories", &self.directories.len())
            .finish()
    }
}

impl OwnedTree {
    pub fn new(path: &Path, remove_root: bool) -> io::Result<Self> {
        Self::new_expected(path, remove_root, None)
    }
    fn new_expected(
        path: &Path,
        remove_root: bool,
        expected: Option<FileIdentity>,
    ) -> io::Result<Self> {
        let requested = std::path::absolute(path)?;
        let parents = HeldParents::new(path)?;
        let path = parents.resolved_path(path)?;
        let tracking = platform().track_operation_directory(parents.directory(), &path)?;
        let identity = platform().file_identity(&tracking)?;
        if expected.is_some_and(|expected| expected != identity) {
            return Err(io::Error::other(
                "Created archive root replaced; preserving it",
            ));
        }
        let tree = DestinationTree::new(&path)?;
        if tree.retained_identity(&path)? != identity {
            return Err(io::Error::other("Archive destination changed"));
        }
        let path = tree.path().to_path_buf();
        Ok(Self {
            requested,
            tree: Some(tree),
            root: Owned {
                path,
                parent: parents.directory().try_clone()?,
                tracking,
                identity,
            },
            directories: HashMap::new(),
            files: HashMap::new(),
            name_bytes: 0,
            cleanup: true,
            remove_root,
            #[cfg(target_os = "windows")]
            recovery: None,
        })
    }
    pub fn created(path: &Path, file: &File) -> io::Result<Self> {
        Self::new_expected(path, true, Some(platform().file_identity(file)?))
    }
    pub fn temporary(parent: &Path) -> io::Result<Self> {
        Self::temporary_prefix(parent, ".oxyde-work-")
    }
    pub fn temporary_prefix(parent: &Path, prefix: &str) -> io::Result<Self> {
        let parents = HeldParents::new(&parent.join(".oxyde-work-root"))?;
        let path = parents
            .resolved_directory()
            .join(format!("{prefix}{}", uuid::Uuid::new_v4()));
        let file = platform().create_journal_directory(parents.directory(), &path, false)?;
        match Self::created(&path, &file) {
            Ok(tree) => Ok(tree),
            Err(error) => {
                if platform()
                    .remove_empty_operation_directory(
                        parents.directory(),
                        &path,
                        platform().file_identity(&file)?,
                    )
                    .is_err()
                {
                    super::archive_recovery::retained(
                        &platform()
                            .retained_file_path(&file, &path)
                            .unwrap_or(path.clone()),
                    );
                }
                Err(error)
            }
        }
    }
    pub fn path(&self) -> &Path {
        &self.root.path
    }
    fn resolved(&self, path: &Path) -> io::Result<PathBuf> {
        let relative = path
            .strip_prefix(self.path())
            .or_else(|_| path.strip_prefix(&self.requested))
            .map_err(|_| io::Error::other("Archive output is outside its root"))?;
        if relative
            .components()
            .any(|part| !matches!(part, std::path::Component::Normal(_)))
        {
            return Err(io::Error::other("Archive output is not a relative child"));
        }
        Ok(self.path().join(relative))
    }
    fn parent(&self, path: &Path) -> io::Result<&File> {
        let parent = path
            .parent()
            .ok_or_else(|| io::Error::other("Missing archive parent"))?;
        if parent == self.path() {
            Ok(&self.root.tracking)
        } else {
            self.directories
                .get(parent)
                .map(|entry| &entry.tracking)
                .ok_or_else(|| io::Error::other("Archive parent is not owned"))
        }
    }
    fn capture_directory(&mut self, path: &Path) -> io::Result<()> {
        let parent = self.parent(path)?.try_clone()?;
        let tracking = platform().track_operation_directory(&parent, path)?;
        let identity = platform().file_identity(&tracking)?;
        if self.tree.as_ref().unwrap().retained_identity(path)? != identity {
            return Err(io::Error::other("Created archive directory changed"));
        }
        self.directories.insert(
            path.into(),
            Owned {
                path: path.into(),
                parent,
                tracking,
                identity,
            },
        );
        self.name_bytes = self.name_bytes.saturating_add(path.as_os_str().len());
        self.check_journal_path(path)?;
        self.check_journal_budget()?;
        #[cfg(target_os = "windows")]
        if let Some(journal) = &self.recovery {
            journal
                .lock()
                .map_err(|_| io::Error::other("Cache proof poisoned"))?
                .created(path, &self.directories[path].tracking, true)?;
        }
        Ok(())
    }
    pub fn ensure_directory(&mut self, path: &Path) -> io::Result<()> {
        let path = self.resolved(path)?;
        let mut created = Vec::new();
        let result =
            self.tree
                .as_ref()
                .unwrap()
                .ensure_directory(&path, &mut || Ok(()), &mut |path| {
                    created.push(path.to_path_buf())
                });
        // Record creations even if a later component fails.
        let mut capture_error = None;
        for path in created {
            if let Err(error) = self.capture_directory(&path) {
                capture_error.get_or_insert(error);
            }
        }
        drop(result?);
        if let Some(error) = capture_error {
            return Err(error);
        }
        Ok(())
    }
    pub fn create_file(&mut self, path: &Path) -> io::Result<File> {
        let path = self.resolved(path)?;
        let path = path.as_path();
        self.ensure_directory(
            path.parent()
                .ok_or_else(|| io::Error::other("Missing archive parent"))?,
        )?;
        let parents =
            self.tree
                .as_ref()
                .unwrap()
                .file_parents(path, &mut || Ok(()), &mut |_| {})?;
        let file = platform().create_operation_output(parents.directory(), path)?;
        let result = self.adopt_created_file(path, &file);
        if let Err(error) = result {
            let _ = platform().remove_copied_file(&file, path);
            return Err(error);
        }
        Ok(file)
    }
    pub fn adopt_created_file(&mut self, path: &Path, file: &File) -> io::Result<()> {
        let path = self.resolved(path)?;
        let parent = self.parent(&path)?.try_clone()?;
        let tracking = platform().track_source_item(&parent, &path)?;
        let identity = platform().file_identity(&tracking)?;
        if platform().file_identity(file)? != identity {
            return Err(io::Error::other("Created archive output changed"));
        }
        let old = self.files.insert(
            path.clone(),
            Owned {
                path: path.clone(),
                parent,
                tracking,
                identity,
            },
        );
        self.name_bytes = self
            .name_bytes
            .saturating_sub(old.map_or(0, |entry| entry.path.as_os_str().len()))
            .saturating_add(path.as_os_str().len());
        self.check_journal_path(&path)?;
        self.check_journal_budget()?;
        #[cfg(target_os = "windows")]
        if let Some(journal) = &self.recovery {
            journal
                .lock()
                .map_err(|_| io::Error::other("Cache proof poisoned"))?
                .created(&path, file, false)?;
        }
        Ok(())
    }
    #[cfg(target_os = "windows")]
    pub fn adopt_created_directory(&mut self, path: &Path, file: &File) -> io::Result<()> {
        let path = self.resolved(path)?;
        let parent = self.parent(&path)?.try_clone()?;
        let tracking = platform().track_operation_directory(&parent, &path)?;
        let identity = platform().file_identity(&tracking)?;
        if platform().file_identity(file)? != identity {
            return Err(io::Error::other("Created archive directory changed"));
        }
        let old = self.directories.insert(
            path.clone(),
            Owned {
                path: path.clone(),
                parent,
                tracking,
                identity,
            },
        );
        self.name_bytes = self
            .name_bytes
            .saturating_sub(old.map_or(0, |entry| entry.path.as_os_str().len()))
            .saturating_add(path.as_os_str().len());
        self.check_journal_path(&path)?;
        self.check_journal_budget()
    }
    fn check_journal_budget(&self) -> io::Result<()> {
        use crate::models::error::{ArchiveLimitFailure, ArchiveLimitKind};
        let limits = crate::utils::archive_limits::ArchiveLimits::WORKSPACE;
        if self.files.len().saturating_add(self.directories.len()) > limits.max_entries {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                ArchiveLimitFailure {
                    kind: ArchiveLimitKind::Entries,
                    limit: limits.max_entries as u64,
                },
            ));
        }
        if self.name_bytes > 64 * 1024 * 1024 {
            return Err(io::Error::other("Archive workspace journal names exceeded"));
        }
        Ok(())
    }
    fn check_journal_path(&self, path: &Path) -> io::Result<()> {
        let relative = path.strip_prefix(self.path()).map_err(io::Error::other)?;
        if relative.components().count()
            > crate::utils::archive_limits::ArchiveLimits::WORKSPACE.max_depth
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                crate::models::error::ArchiveLimitFailure {
                    kind: crate::models::error::ArchiveLimitKind::Depth,
                    limit: crate::utils::archive_limits::ArchiveLimits::WORKSPACE.max_depth as u64,
                },
            ));
        }
        if relative.to_string_lossy().len() > crate::utils::archive_io_protocol::MAX_NAME {
            return Err(io::Error::other("Archive workspace name exceeded"));
        }
        Ok(())
    }
    #[cfg(target_os = "windows")]
    pub fn identities(&self) -> HashMap<PathBuf, FileIdentity> {
        self.files
            .values()
            .chain(self.directories.values())
            .map(|entry| (entry.path.clone(), entry.identity))
            .collect()
    }
    #[cfg(target_os = "windows")]
    pub(super) fn recovery_root(&self) -> &File {
        &self.root.tracking
    }
    #[cfg(target_os = "windows")]
    pub(super) fn recovery_records(&self) -> impl Iterator<Item = (&Path, &File, bool)> {
        self.directories
            .values()
            .map(|entry| (entry.path.as_path(), &entry.tracking, true))
            .chain(
                self.files
                    .values()
                    .map(|entry| (entry.path.as_path(), &entry.tracking, false)),
            )
    }
    #[cfg(target_os = "windows")]
    pub(super) fn attach_recovery(
        &mut self,
        journal: Arc<Mutex<super::archive_open_cache::recovery::Journal>>,
    ) {
        self.recovery = Some(journal);
    }
    pub(super) fn before_cache_write(
        &self,
        _path: &Path,
        _file: &File,
        _data: &[u8],
    ) -> io::Result<()> {
        #[cfg(target_os = "windows")]
        if let Some(journal) = &self.recovery {
            journal
                .lock()
                .map_err(|_| io::Error::other("Cache proof poisoned"))?
                .before_write(_path, _file, _data)?;
        }
        Ok(())
    }
    pub(super) fn complete_cache_write(&self, _path: &Path, _file: &File) -> io::Result<()> {
        #[cfg(target_os = "windows")]
        if let Some(journal) = &self.recovery {
            journal
                .lock()
                .map_err(|_| io::Error::other("Cache proof poisoned"))?
                .complete(_path, _file)?;
        }
        Ok(())
    }
    #[cfg(not(test))]
    pub(super) fn cache_write_observed(&self, _path: &Path) -> io::Result<()> {
        #[cfg(target_os = "windows")]
        if self.recovery.is_some() {
            crate::utils::archive_mutation_test::checkpoint(
                crate::utils::archive_mutation_test::Stage::CacheWritten,
                _path,
                self.path(),
            )?;
        }
        Ok(())
    }
    #[cfg(target_os = "windows")]
    pub fn verify_owned(&self, path: &Path, file: &File) -> io::Result<()> {
        let expected = if path == self.path() {
            Some(self.root.identity)
        } else {
            self.files
                .get(path)
                .or_else(|| self.directories.get(path))
                .map(|entry| entry.identity)
        };
        if expected != Some(platform().file_identity(file)?) {
            return Err(crate::models::error::SourceSelectionFailure::changed(path));
        }
        Ok(())
    }
    #[cfg(target_os = "windows")]
    pub fn forget_removed(&mut self, path: &Path, identity: FileIdentity) -> io::Result<()> {
        let selected = self
            .files
            .get(path)
            .or_else(|| self.directories.get(path))
            .ok_or_else(|| io::Error::other("Removed archive source was not owned"))?;
        if selected.identity != identity {
            return Err(crate::models::error::SourceSelectionFailure::changed(path));
        }
        self.name_bytes = self.name_bytes.saturating_sub(path.as_os_str().len());
        self.files.remove(path);
        self.directories.remove(path);
        Ok(())
    }
    #[cfg(target_os = "windows")]
    pub fn root_identity(&self) -> FileIdentity {
        self.root.identity
    }
    #[cfg(target_os = "windows")]
    pub fn create_directory(&mut self, path: &Path) -> io::Result<()> {
        let path = self.resolved(path)?;
        let parent = self.parent(&path)?.try_clone()?;
        let file = platform().create_journal_directory(&parent, &path, false)?;
        self.adopt_created_directory(&path, &file)
    }
    #[cfg(target_os = "windows")]
    pub fn validate_rename(
        &self,
        source: &Path,
        destination: &Path,
        checkpoint: &mut dyn FnMut() -> io::Result<()>,
    ) -> io::Result<()> {
        let source = self.resolved(source)?;
        let destination = self.resolved(destination)?;
        let selected = self
            .files
            .get(&source)
            .or_else(|| self.directories.get(&source))
            .ok_or_else(|| io::Error::other("Archive rename source is not owned"))?;
        let parent = self.parent(&source)?;
        let destination_parent = self.parent(&destination)?;
        let file = platform()
            .open_rename_item(parent, &source)
            .map_err(|e| io::Error::new(e.kind(), format!("Archive rename open: {e}")))?;
        if platform().file_identity(&file)? != selected.identity {
            return Err(io::Error::other(
                "Archive rename source replaced; preserving it",
            ));
        }
        checkpoint()?;
        match platform().track_source_item(destination_parent, &destination) {
            Ok(_) => {
                return Err(io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    "Rename target already exists",
                ))
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
        self.check_journal_path(&destination)?;
        Ok(())
    }
    #[cfg(target_os = "windows")]
    pub fn remove_selected(
        &mut self,
        path: &Path,
        checkpoint: &mut dyn FnMut() -> io::Result<()>,
    ) -> io::Result<()> {
        let path = self.resolved(path)?;
        let files: Vec<_> = self
            .files
            .keys()
            .filter(|key| key.starts_with(&path))
            .cloned()
            .collect();
        for key in files {
            checkpoint()?;
            let entry = self.files.remove(&key).unwrap();
            let result = (|| {
                let file = platform().open_copied_file(&entry.parent, &entry.path)?;
                if platform().file_identity(&file)? != entry.identity {
                    return Err(io::Error::other(
                        "Archive deletion source replaced; preserving it",
                    ));
                }
                platform().remove_copied_file(&file, &entry.path)
            })();
            if let Err(error) = result {
                self.files.insert(key, entry);
                return Err(error);
            }
            self.name_bytes = self.name_bytes.saturating_sub(entry.path.as_os_str().len());
            drop(entry); // Release the last tracker before deleting an empty ancestor.
        }
        let mut directories: Vec<_> = self
            .directories
            .keys()
            .filter(|key| key.starts_with(&path))
            .cloned()
            .collect();
        directories.sort_by_key(|key| std::cmp::Reverse(key.components().count()));
        for key in directories {
            checkpoint()?;
            let entry = self.directories.remove(&key).unwrap();
            if let Err(error) = platform().remove_empty_operation_directory(
                &entry.parent,
                &entry.path,
                entry.identity,
            ) {
                self.directories.insert(key, entry);
                return Err(error);
            }
            self.name_bytes = self.name_bytes.saturating_sub(entry.path.as_os_str().len());
            drop(entry);
        }
        self.name_bytes = self
            .files
            .keys()
            .chain(self.directories.keys())
            .map(|path| path.as_os_str().len())
            .sum();
        Ok(())
    }
    #[cfg(test)]
    pub fn check(&self) -> io::Result<()> {
        self.tree.as_ref().unwrap().check_all(&mut || Ok(()))
    }
    pub fn check_at(&self, directory: &Path) -> io::Result<()> {
        self.tree
            .as_ref()
            .unwrap()
            .check_directory(directory, &mut || Ok(()))
    }
    pub fn check_controlled(
        &self,
        checkpoint: &mut dyn FnMut() -> io::Result<()>,
    ) -> io::Result<()> {
        self.tree.as_ref().unwrap().check_all(checkpoint)
    }
    pub fn mark(&self, path: &Path, file: &File, data: &[u8]) -> io::Result<()> {
        let parents =
            self.tree
                .as_ref()
                .unwrap()
                .file_parents(path, &mut || Ok(()), &mut |_| {})?;
        platform().write_archive_output_provenance(parents.directory(), file, path, data)
    }
    /// Release rename guards for workspace editing; identity trackers remain alive.
    pub fn release(&mut self) {
        self.tree.take();
    }
    pub fn keep(&mut self) {
        super::archive_recovery::published(self.path());
        self.cleanup = false;
        self.release();
    }
}

impl Drop for OwnedTree {
    fn drop(&mut self) {
        self.release();
        if !self.cleanup {
            return;
        }
        let mut preserved = false;
        for (_, entry) in self.files.drain() {
            let result = (|| {
                let file = platform().open_copied_file(&entry.parent, &entry.path)?;
                if platform().file_identity(&file)? != entry.identity {
                    return Err(io::Error::other("Archive output replaced; preserving it"));
                }
                platform().remove_copied_file(&file, &entry.path)
            })();
            if let Err(error) = result {
                if error.kind() != io::ErrorKind::NotFound {
                    preserved = true;
                    log::warn!(
                        "Archive cleanup preserved {}: {error}",
                        entry.path.display()
                    );
                }
            }
        }
        let mut directories: Vec<_> = self.directories.drain().map(|(_, entry)| entry).collect();
        directories.sort_by_key(|entry| std::cmp::Reverse(entry.path.components().count()));
        for entry in directories {
            if let Err(error) = platform().remove_empty_operation_directory(
                &entry.parent,
                &entry.path,
                entry.identity,
            ) {
                if error.kind() != io::ErrorKind::NotFound {
                    preserved = true;
                    log::warn!(
                        "Archive cleanup preserved {}: {error}",
                        entry.path.display()
                    );
                }
            }
        }
        if self.remove_root {
            if let Err(error) = platform().remove_empty_operation_directory(
                &self.root.parent,
                self.path(),
                self.root.identity,
            ) {
                if error.kind() == io::ErrorKind::NotFound {
                    preserved |= platform()
                        .retained_file_path(&self.root.tracking, self.path())
                        .is_ok_and(|path| path != self.path());
                }
                if error.kind() != io::ErrorKind::NotFound {
                    preserved = true;
                    log::warn!(
                        "Archive recovery retained at {}: {error}",
                        self.path().display()
                    );
                }
            } else {
                preserved = false;
            }
        }
        if preserved {
            let path = platform()
                .retained_file_path(&self.root.tracking, self.path())
                .unwrap_or_else(|_| self.path().to_path_buf());
            super::archive_recovery::retained(&path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    #[test]
    fn recovery_reports_the_current_name_of_a_retained_root() {
        let temp = tempfile::tempdir().unwrap();
        let moved = temp.path().join("moved");
        let result = super::super::archive_recovery::scope::<(), String>(|| {
            let mut tree = OwnedTree::temporary(temp.path()).unwrap();
            let original = tree.path().to_path_buf();
            tree.release();
            std::fs::rename(original, &moved).unwrap();
            drop(tree);
            Ok(())
        })
        .unwrap_err();
        let report = crate::models::error::ArchiveRecoveryFailure::from_message(&result).unwrap();
        assert!(report.completed);
        assert_eq!(
            Path::new(&report.locations[0]),
            std::fs::canonicalize(&moved).unwrap()
        );
        assert!(moved.is_dir());
    }
    #[test]
    fn business_mutations_keep_creation_proofs_and_preserve_unknown_children() {
        let temp = tempfile::tempdir().unwrap();
        let mut tree = OwnedTree::temporary(temp.path()).unwrap();
        let root = tree.path().to_path_buf();
        assert_eq!(
            tree.root_identity(),
            platform().file_identity(&tree.root.tracking).unwrap()
        );
        drop(tree.create_file(&root.join("folder/owned")).unwrap());
        tree.release();
        tree.create_directory(&root.join("added")).unwrap();
        let selected = platform()
            .track_operation_directory(&tree.root.tracking, &root.join("added"))
            .unwrap();
        tree.verify_owned(&root.join("added"), &selected).unwrap();
        let identity = platform().file_identity(&selected).unwrap();
        drop(selected);
        platform()
            .remove_empty_operation_directory(&tree.root.tracking, &root.join("added"), identity)
            .unwrap();
        tree.forget_removed(&root.join("added"), identity).unwrap();
        tree.create_directory(&root.join("added")).unwrap();
        tree.validate_rename(&root.join("folder"), &root.join("renamed"), &mut || Ok(()))
            .unwrap();
        assert!(tree.identities().contains_key(&root.join("folder/owned")));
        std::fs::write(root.join("folder/foreign"), b"keep").unwrap();
        assert!(tree
            .remove_selected(&root.join("folder"), &mut || Ok(()))
            .is_err());
        assert_eq!(std::fs::read(root.join("folder/foreign")).unwrap(), b"keep");
        assert!(!root.join("folder/owned").exists());
        tree.remove_selected(&root.join("added"), &mut || Ok(()))
            .unwrap();
        drop(tree);
        assert!(!root.join("added").exists());
        assert_eq!(std::fs::read(root.join("folder/foreign")).unwrap(), b"keep");
    }
    #[test]
    fn cleanup_preserves_foreign_children_and_removes_only_owned_outputs() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("extracted");
        std::fs::create_dir(&root).unwrap();
        let mut tree = OwnedTree::new(&root, true).unwrap();
        let path = root.join("new/owned");
        let mut output = tree.create_file(&path).unwrap();
        output.write_all(b"owned").unwrap();
        let marked = tree.path().join("new/owned");
        tree.mark(&marked, &output, b"[ZoneTransfer]\r\nZoneId=3\r\n")
            .unwrap();
        drop(output);
        tree.check().unwrap();
        tree.check_at(tree.path()).unwrap();
        tree.check_controlled(&mut || Ok(())).unwrap();
        std::fs::write(root.join("new/foreign"), b"keep").unwrap();
        drop(tree);
        assert!(!path.exists());
        assert_eq!(std::fs::read(root.join("new/foreign")).unwrap(), b"keep");
    }
    #[test]
    fn cleanup_preserves_replacement_with_reused_name() {
        let temp = tempfile::tempdir().unwrap();
        let mut tree = OwnedTree::new(temp.path(), false).unwrap();
        let path = temp.path().join("owned");
        drop(tree.create_file(&path).unwrap());
        tree.release();
        std::fs::remove_file(&path).unwrap();
        std::fs::write(&path, b"foreign").unwrap();
        drop(tree);
        assert_eq!(std::fs::read(path).unwrap(), b"foreign");
    }
    #[test]
    fn successful_outputs_are_kept_and_failed_empty_roots_are_removed() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("failed");
        std::fs::create_dir(&root).unwrap();
        let mut tree = OwnedTree::new(&root, true).unwrap();
        tree.ensure_directory(&root.join("a/b")).unwrap();
        drop(tree);
        assert!(!root.exists());
        let mut tree = OwnedTree::new(temp.path(), false).unwrap();
        let path = temp.path().join("kept");
        drop(tree.create_file(&path).unwrap());
        tree.keep();
        drop(tree);
        assert!(path.exists());
        let tree = OwnedTree::temporary(temp.path()).unwrap();
        let empty = tree.path().to_path_buf();
        drop(tree);
        assert!(!empty.exists());
    }
}
