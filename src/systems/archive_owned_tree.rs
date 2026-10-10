//! Cleanup follows only identities captured at creation; never enumerate to claim ownership.
use super::{destination_tree::DestinationTree, path_parents::HeldParents};
use crate::platform::{platform, FileIdentity};
use std::{
    collections::HashMap,
    fs::File,
    io,
    path::{Path, PathBuf},
};

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
    files: Vec<Owned>,
    cleanup: bool,
    remove_root: bool,
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
            files: Vec::new(),
            cleanup: true,
            remove_root,
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
                let _ = platform().remove_empty_operation_directory(
                    parents.directory(),
                    &path,
                    platform().file_identity(&file)?,
                );
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
        for path in created {
            self.capture_directory(&path)?;
        }
        drop(result?);
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
        self.files.push(Owned {
            path,
            parent,
            tracking,
            identity,
        });
        Ok(())
    }
    pub fn check(&self) -> io::Result<()> {
        self.tree.as_ref().unwrap().check_all(&mut || Ok(()))
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
        for entry in self.files.drain(..).rev() {
            let result = (|| {
                let file = platform().open_copied_file(&entry.parent, &entry.path)?;
                if platform().file_identity(&file)? != entry.identity {
                    return Err(io::Error::other("Archive output replaced; preserving it"));
                }
                platform().remove_copied_file(&file, &entry.path)
            })();
            if let Err(error) = result {
                if error.kind() != io::ErrorKind::NotFound {
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
                if error.kind() != io::ErrorKind::NotFound {
                    log::warn!(
                        "Archive recovery retained at {}: {error}",
                        self.path().display()
                    );
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
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
