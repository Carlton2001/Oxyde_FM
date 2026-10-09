//! Directory identities for one undo transaction, including its rollback.
use super::path_parents::HeldParents;
use crate::platform::{FileIdentity, platform};
use std::{
    collections::HashMap,
    fs::File,
    io,
    path::{Path, PathBuf},
};

struct Node {
    file: File,
    identity: FileIdentity,
}

pub(crate) enum Removal {
    Deleted,
    Missing,
    NotEmpty,
}

#[derive(Default)]
pub(crate) struct Directories {
    nodes: HashMap<PathBuf, Node>,
    // Only ancestors outside the removal plan deny deletion throughout undo.
    roots: HashMap<PathBuf, HeldParents>,
}

impl Directories {
    pub fn prepare(paths: &[PathBuf], removed: &[PathBuf]) -> io::Result<Self> {
        let mut result = Self::default();
        for path in paths.iter().chain(removed) {
            let mut root = path.as_path();
            loop {
                if !removed.iter().any(|directory| root.starts_with(directory)) {
                    // A pre-existing junction parent is resolved once by HeldParents.
                    // Children in the removal plan are opened separately without reparse.
                    match std::fs::metadata(root) {
                        Ok(metadata) if metadata.is_dir() => break,
                        Ok(_) => {
                            return Err(io::Error::other(format!(
                                "Not a journal directory: {}",
                                root.display()
                            )));
                        }
                        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                        Err(error) => return Err(error),
                    }
                }
                root = root
                    .parent()
                    .ok_or_else(|| io::Error::other("Journal has no stable parent"))?;
            }
            if result.nodes.contains_key(root) {
                continue;
            }
            let parents = HeldParents::new(&root.join(".oxyde-parent"))?;
            let file = parents.directory().try_clone()?;
            let identity = platform().file_identity(&file)?;
            result
                .nodes
                .insert(root.to_path_buf(), Node { file, identity });
            result.roots.insert(root.to_path_buf(), parents);
        }
        let mut directories = removed.to_vec();
        directories.sort_by_key(|path| path.components().count());
        for path in directories {
            if let Err(error) = result.directory(&path) {
                if error.kind() != io::ErrorKind::NotFound {
                    return Err(error);
                }
            }
        }
        Ok(result)
    }

    fn ordinary(file: &File) -> io::Result<()> {
        let metadata = file.metadata()?;
        if !metadata.is_dir() || platform().file_attributes(&metadata, "").2 {
            return Err(io::Error::other(
                "Journal directory is no longer an ordinary directory",
            ));
        }
        Ok(())
    }

    pub fn directory(&mut self, path: &Path) -> io::Result<&File> {
        let mut missing = Vec::new();
        let mut ancestor = path;
        while !self.nodes.contains_key(ancestor) {
            missing.push(ancestor);
            ancestor = ancestor
                .parent()
                .ok_or_else(|| io::Error::other("Unbound journal root"))?;
        }
        Self::ordinary(&self.nodes.get(ancestor).unwrap().file)?;
        for path in missing.into_iter().rev() {
            let parent = &self.nodes.get(path.parent().unwrap()).unwrap().file;
            Self::ordinary(parent)?;
            let file = platform().track_operation_directory(parent, path)?;
            let identity = platform().file_identity(&file)?;
            self.nodes
                .insert(path.to_path_buf(), Node { file, identity });
        }
        let node = self.nodes.get(path).unwrap();
        Self::ordinary(&node.file)?;
        Ok(&node.file)
    }

    pub fn verify_history_identity(
        &self,
        path: &Path,
        expected: &crate::models::transaction::HistoryDirectoryIdentity,
    ) -> io::Result<()> {
        let Some(node) = self.nodes.get(path) else {
            return Ok(());
        };
        let snapshot = expected.snapshot().ok_or_else(|| {
            io::Error::other(crate::models::error::HistoryIdentityFailure {
                issue: crate::models::error::HistoryIdentityIssue::Unverified,
                path: path.to_string_lossy().into_owned(),
            })
        })?;
        if !snapshot.matches(&node.file)? {
            return Err(super::file_copy::identity_changed(path));
        }
        Ok(())
    }

    pub fn create(&mut self, path: &Path, hidden: bool) -> io::Result<&File> {
        let parent = path
            .parent()
            .ok_or_else(|| io::Error::other("Directory has no parent"))?;
        let file = platform().create_journal_directory(self.directory(parent)?, path, hidden)?;
        let identity = platform().file_identity(&file)?;
        self.nodes
            .insert(path.to_path_buf(), Node { file, identity });
        Ok(&self.nodes.get(path).unwrap().file)
    }

    pub fn resolved(&self, path: &Path) -> io::Result<PathBuf> {
        let (root, parents) = self
            .roots
            .iter()
            .filter(|(root, _)| path.starts_with(root))
            .max_by_key(|(root, _)| root.components().count())
            .ok_or_else(|| io::Error::other("Path is outside journal roots"))?;
        Ok(parents
            .resolved_directory()
            .join(path.strip_prefix(root).unwrap()))
    }

    pub fn file_parents(&mut self, path: &Path) -> io::Result<HeldParents> {
        let parent = path
            .parent()
            .ok_or_else(|| io::Error::other("File has no parent"))?;
        let directory = self.directory(parent)?.try_clone()?;
        let resolved = self.resolved(parent)?;
        let (_, root) = self
            .roots
            .iter()
            .filter(|(root, _)| parent.starts_with(root))
            .max_by_key(|(root, _)| root.components().count())
            .ok_or_else(|| io::Error::other("Unbound file parent"))?;
        root.with_bound_directory(directory, vec![], resolved)
    }

    pub fn stage(&mut self, path: &Path) -> io::Result<File> {
        let parent = path
            .parent()
            .ok_or_else(|| io::Error::other("Staging has no parent"))?;
        let parent = self.directory(parent)?;
        let (directory, pin) = platform().create_copy_staging(parent, path)?;
        let identity = match platform().file_identity(&directory) {
            Ok(identity) => identity,
            Err(error) => {
                let unpin = platform().remove_open_file(&pin, &path.join(".pin"));
                drop(pin);
                if let Err(cleanup) =
                    unpin.and_then(|_| platform().remove_open_file(&directory, path))
                {
                    log::warn!(
                        "Empty journal staging retained at {}: {cleanup}",
                        path.display()
                    );
                }
                return Err(error);
            }
        };
        // Reopen without DELETE access so history's parent readers can coexist.
        // The original identity remains alive through this change of access.
        let tracked = match platform().track_operation_directory(parent, path) {
            Ok(tracked) => tracked,
            Err(error) => {
                let unpin = platform().remove_open_file(&pin, &path.join(".pin"));
                drop(pin);
                if let Err(cleanup) =
                    unpin.and_then(|_| platform().remove_open_file(&directory, path))
                {
                    log::warn!(
                        "Empty journal staging retained at {}: {cleanup}",
                        path.display()
                    );
                }
                return Err(error);
            }
        };
        drop(directory);
        let prepared = (|| {
            let (directory, created) = platform().ensure_operation_directory(parent, path)?;
            if created || platform().file_identity(&directory)? != identity {
                return Err(io::Error::other("Journal staging changed during creation"));
            }
            Ok((directory, identity))
        })();
        let (directory, identity) = match prepared {
            Ok(result) => result,
            Err(error) => {
                // No output has entered staging yet. Unpin and remove only the
                // created empty object; preserve a substituted name or new child.
                let unpin = platform().remove_open_file(&pin, &path.join(".pin"));
                drop(pin);
                let cleanup = unpin.and_then(|_| {
                    platform().remove_empty_operation_directory(parent, path, identity)
                });
                if let Err(cleanup) = cleanup {
                    log::warn!(
                        "Empty journal staging retained at {}: {cleanup}",
                        path.display()
                    );
                }
                return Err(error);
            }
        };
        drop(tracked);
        self.nodes.insert(
            path.to_path_buf(),
            Node {
                file: directory,
                identity,
            },
        );
        Ok(pin)
    }

    pub fn clean_stage(&mut self, path: &Path, pin: File) -> io::Result<()> {
        let pin_path = self.resolved(path)?.join(".pin");
        let unpin = platform().remove_open_file(&pin, &pin_path);
        drop(pin);
        unpin?;
        let parent = path
            .parent()
            .ok_or_else(|| io::Error::other("Staging has no parent"))?;
        let file = platform().track_operation_directory(self.directory(parent)?, path)?;
        let identity = platform().file_identity(&file)?;
        let node = self
            .nodes
            .get_mut(path)
            .ok_or_else(|| io::Error::other("Untracked journal staging"))?;
        if identity != node.identity {
            return Err(io::Error::other(
                "Journal staging replacement was preserved",
            ));
        }
        node.file = file; // Release the directory reader before its DELETE open.
        if matches!(self.remove(path)?, Removal::NotEmpty) {
            return Err(io::Error::new(
                io::ErrorKind::DirectoryNotEmpty,
                "Recovery content was preserved",
            ));
        }
        Ok(())
    }

    pub fn metadata(&self, path: &Path) -> io::Result<Option<std::fs::Metadata>> {
        self.nodes
            .get(path)
            .map(|node| {
                Self::ordinary(&node.file)?;
                node.file.metadata()
            })
            .transpose()
    }

    pub fn remove(&mut self, path: &Path) -> io::Result<Removal> {
        // Missing during capture must not make a later new directory removable.
        let Some(node) = self.nodes.get(path) else {
            return Ok(Removal::Missing);
        };
        let identity = node.identity;
        let parent = path
            .parent()
            .ok_or_else(|| io::Error::other("Directory has no parent"))?;
        match platform().remove_empty_operation_directory(self.directory(parent)?, path, identity) {
            Ok(()) => {
                // Close this object before deleting its containing directory.
                self.nodes.remove(path);
                Ok(Removal::Deleted)
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                self.nodes.remove(path);
                Ok(Removal::Missing)
            }
            Err(error) if error.kind() == io::ErrorKind::DirectoryNotEmpty => Ok(Removal::NotEmpty),
            Err(error) => Err(error),
        }
    }
}

#[cfg(all(test, target_os = "windows"))]
mod tests {
    use super::*;
    use crate::platform::windows_reparse_tests::{Retarget, junction};

    #[test]
    fn restoring_attributes_changes_the_created_object_after_a_name_replacement() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("created");
        let relocated = root.path().join("relocated");
        let mut directories = Directories::prepare(std::slice::from_ref(&path), &[]).unwrap();
        let directory = directories
            .create(&path, false)
            .unwrap()
            .try_clone()
            .unwrap();
        std::fs::rename(&path, &relocated).unwrap();
        std::fs::create_dir(&path).unwrap();
        let mut permissions = directory.metadata().unwrap().permissions();
        permissions.set_readonly(true);
        platform()
            .restore_journal_directory(&directory, &path, permissions, true)
            .unwrap();
        assert!(
            std::fs::metadata(&relocated)
                .unwrap()
                .permissions()
                .readonly()
        );
        assert!(
            platform()
                .file_attributes(&std::fs::metadata(&relocated).unwrap(), "")
                .0
        );
        assert!(!std::fs::metadata(&path).unwrap().permissions().readonly());
        assert!(
            !platform()
                .file_attributes(&std::fs::metadata(&path).unwrap(), "")
                .0
        );
        let mut permissions = directory.metadata().unwrap().permissions();
        permissions.set_readonly(false);
        platform()
            .restore_journal_directory(&directory, &path, permissions, false)
            .unwrap();
    }

    #[test]
    fn undo_staging_pins_its_name_and_refuses_a_reparse_tag() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join(".oxyde-undo-test");
        let outside = root.path().join("outside");
        std::fs::create_dir(&outside).unwrap();
        let template = root.path().join("template");
        junction(&template, &outside);
        let mut directories =
            Directories::prepare(std::slice::from_ref(&root.path().to_path_buf()), &[]).unwrap();
        let pin = directories.stage(&path).unwrap();
        assert!(std::fs::rename(&path, root.path().join("renamed")).is_err());
        let writer = Retarget::new(&path, &template);
        assert!(
            writer.try_apply().is_err(),
            "staging must not become a junction while pinned"
        );
        drop(writer);
        directories.clean_stage(&path, pin).unwrap();
        assert!(!path.exists());
        assert!(outside.is_dir());
        std::fs::remove_dir(template).unwrap();
    }

    #[test]
    fn undo_staging_cleanup_preserves_recovery_output_and_unknown_children() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join(".oxyde-undo-test");
        let mut directories =
            Directories::prepare(std::slice::from_ref(&root.path().to_path_buf()), &[]).unwrap();
        let pin = directories.stage(&path).unwrap();
        std::fs::write(path.join("output"), b"recovery content").unwrap();
        std::fs::create_dir(path.join("foreign-empty")).unwrap();
        let error = directories.clean_stage(&path, pin).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::DirectoryNotEmpty);
        assert_eq!(
            std::fs::read(path.join("output")).unwrap(),
            b"recovery content"
        );
        assert!(path.join("foreign-empty").is_dir());
        assert!(!path.join(".pin").exists());
    }
}
