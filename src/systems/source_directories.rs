//! Capture source directories before moving contents; never rediscover cleanup by name.
use super::path_parents::HeldParents;
use crate::platform::{FileIdentity, platform};
use std::{
    collections::HashMap,
    fs::File,
    io,
    path::{Path, PathBuf},
    sync::Arc,
};

struct Entry {
    path: PathBuf,
    destination: PathBuf,
    parent: Arc<File>,
    // Retain the object until removal so its ID cannot be reused after deletion.
    _directory: Arc<File>,
    identity: FileIdentity,
}

#[derive(Default)]
pub(super) struct SourceDirectories {
    entries: Vec<Entry>,
    directories: HashMap<PathBuf, Arc<File>>,
    roots: Vec<HeldParents>,
}

impl SourceDirectories {
    #[cfg(test)]
    pub fn capture(&mut self, path: &Path, destination: &Path) -> io::Result<()> {
        if self.directories.contains_key(path) {
            return Ok(());
        }
        let parent_path = path
            .parent()
            .ok_or_else(|| io::Error::other("Source directory has no parent"))?;
        let parent = if let Some(parent) = self.directories.get(parent_path) {
            Arc::clone(parent)
        } else {
            let parents = HeldParents::new(path)?;
            let parent = Arc::new(parents.directory().try_clone()?);
            self.roots.push(parents);
            parent
        };
        let directory = Arc::new(platform().track_operation_directory(&parent, path)?);
        let identity = platform().file_identity(&directory)?;
        self.entries.push(Entry {
            path: path.to_path_buf(),
            destination: destination.to_path_buf(),
            parent,
            _directory: Arc::clone(&directory),
            identity,
        });
        self.directories.insert(path.to_path_buf(), directory);
        Ok(())
    }

    pub fn capture_bound(
        &mut self,
        path: &Path,
        destination: &Path,
        parents: HeldParents,
        selected: &File,
    ) -> io::Result<()> {
        if self.directories.contains_key(path) {
            return Ok(());
        }
        let parent = if let Some(parent) = path.parent().and_then(|path| self.directories.get(path))
        {
            Arc::clone(parent)
        } else {
            let parent = Arc::new(parents.directory().try_clone()?);
            self.roots.push(parents);
            parent
        };
        // Reopen only the selected child for tracking, permitting later deletion.
        let directory = Arc::new(platform().track_operation_directory(&parent, path)?);
        let identity = platform().file_identity(&directory)?;
        if identity != platform().file_identity(selected)? {
            return Err(crate::models::error::SourceSelectionFailure::changed(path));
        }
        self.entries.push(Entry {
            path: path.into(),
            destination: destination.into(),
            parent,
            _directory: Arc::clone(&directory),
            identity,
        });
        self.directories.insert(path.into(), directory);
        Ok(())
    }

    pub fn remove_empty(
        mut self,
        checkpoint: &mut dyn FnMut() -> io::Result<()>,
        removed: &mut dyn FnMut(&Path, &Path),
    ) -> io::Result<()> {
        self.remove_matching(&|_| true, checkpoint, &mut |_, _| Ok(()), removed)
    }

    pub fn remove_matching(
        &mut self,
        matches: &dyn Fn(&Path) -> bool,
        checkpoint: &mut dyn FnMut() -> io::Result<()>,
        before_remove: &mut dyn FnMut(&Path, &mut dyn FnMut() -> io::Result<()>) -> io::Result<()>,
        removed: &mut dyn FnMut(&Path, &Path),
    ) -> io::Result<()> {
        self.remove_matching_observed(matches, checkpoint, before_remove, &mut |source, destination, _| { removed(source, destination); Ok(()) })
    }
    pub fn remove_matching_observed(
        &mut self,
        matches: &dyn Fn(&Path) -> bool,
        checkpoint: &mut dyn FnMut() -> io::Result<()>,
        before_remove: &mut dyn FnMut(&Path, &mut dyn FnMut() -> io::Result<()>) -> io::Result<()>,
        removed: &mut dyn FnMut(&Path, &Path, FileIdentity) -> io::Result<()>,
    ) -> io::Result<()> {
        self.entries
            .sort_by_key(|entry| std::cmp::Reverse(entry.path.components().count()));
        // Shared directory references must disappear bottom-up, including those
        // held by the lookup table, before the containing directory can be deleted.
        self.directories.clear();
        self.roots.clear();
        let entries = std::mem::take(&mut self.entries);
        for entry in entries {
            if !matches(&entry.path) {
                self.entries.push(entry);
                continue;
            }
            checkpoint()?;
            before_remove(&entry.destination, checkpoint).map_err(|error| {
                io::Error::new(
                    error.kind(),
                    format!(
                        "Cannot validate copied directory {:?} before source cleanup: {error}",
                        entry.destination
                    ),
                )
            })?;
            match platform().remove_empty_operation_directory(
                &entry.parent,
                &entry.path,
                entry.identity,
            ) {
                Ok(()) => removed(&entry.path, &entry.destination, entry.identity)?,
                Err(error)
                    if matches!(
                        error.kind(),
                        io::ErrorKind::DirectoryNotEmpty | io::ErrorKind::NotFound
                    ) => {}
                Err(error) => {
                    return Err(io::Error::new(
                        error.kind(),
                        format!(
                            "Moved contents, but cannot remove source directory {:?}: {error}",
                            entry.path
                        ),
                    ));
                }
            }
            // Drop the selected handle (and its retained parent) before the next entry.
        }
        Ok(())
    }
}

#[cfg(all(test, target_os = "windows"))]
mod tests {
    use super::*;
    use crate::platform::windows_reparse_tests::{Retarget, junction};

    #[test]
    fn source_directory_cleanup_separates_groups_and_drops_descendant_handles() {
        let root = tempfile::tempdir().unwrap();
        let staged = root.path().join("staged");
        let real = root.path().join("real");
        std::fs::create_dir_all(staged.join("nested/empty")).unwrap();
        std::fs::create_dir_all(real.join("nested/empty")).unwrap();
        let mut directories = SourceDirectories::default();
        for source in [&staged, &real] {
            for suffix in ["", "nested", "nested/empty"] {
                let path = source.join(suffix);
                directories
                    .capture(&path, &root.path().join("destination").join(suffix))
                    .unwrap();
            }
        }
        let mut removed = Vec::new();
        directories
            .remove_matching(
                &|path| path.starts_with(&staged),
                &mut || Ok(()),
                &mut |_, _| Ok(()),
                &mut |source, _| removed.push(source.to_path_buf()),
            )
            .unwrap();
        assert!(!staged.exists());
        assert!(real.join("nested/empty").is_dir());
        assert_eq!(removed.len(), 3);
        directories
            .remove_matching(
                &|path| path.starts_with(&real),
                &mut || Ok(()),
                &mut |_, _| Ok(()),
                &mut |source, _| removed.push(source.to_path_buf()),
            )
            .unwrap();
        assert!(!real.exists());
        assert_eq!(removed.len(), 6);
    }

    #[test]
    fn source_directory_cleanup_preserves_a_replacement_identity() {
        let root = tempfile::tempdir().unwrap();
        let selected = root.path().join("selected");
        let original = root.path().join("original");
        std::fs::create_dir(&selected).unwrap();
        let mut directories = SourceDirectories::default();
        directories
            .capture(&selected, &root.path().join("destination"))
            .unwrap();
        let mut journal = Vec::new();
        let error = directories
            .remove_empty(
                &mut || {
                    std::fs::rename(&selected, &original)?;
                    std::fs::create_dir(&selected)
                },
                &mut |source, _| journal.push(source.to_path_buf()),
            )
            .unwrap_err();
        assert!(
            error.to_string().contains("replacement was preserved"),
            "{error}"
        );
        assert!(selected.is_dir());
        assert!(original.is_dir());
        assert!(journal.is_empty());
    }

    #[test]
    fn source_directory_cleanup_stays_in_the_initial_junction_parent() {
        let root = tempfile::tempdir().unwrap();
        let chosen = root.path().join("chosen");
        let outside = root.path().join("outside");
        let link = root.path().join("link");
        let template = root.path().join("template");
        std::fs::create_dir_all(chosen.join("empty")).unwrap();
        std::fs::create_dir_all(outside.join("empty")).unwrap();
        std::fs::write(outside.join("witness"), b"outside witness").unwrap();
        junction(&link, &chosen);
        junction(&template, &outside);
        let retarget = Retarget::new(&link, &template);
        let mut directories = SourceDirectories::default();
        directories
            .capture(&link.join("empty"), &root.path().join("destination"))
            .unwrap();
        let mut journal = Vec::new();
        directories
            .remove_empty(
                &mut || {
                    retarget.apply();
                    Ok(())
                },
                &mut |source, _| journal.push(source.to_path_buf()),
            )
            .unwrap();
        assert!(!chosen.join("empty").exists());
        assert!(outside.join("empty").is_dir());
        assert_eq!(
            std::fs::read(outside.join("witness")).unwrap(),
            b"outside witness"
        );
        assert_eq!(journal, vec![link.join("empty")]);
        drop(retarget);
        std::fs::remove_dir(&link).unwrap();
        std::fs::remove_dir(&template).unwrap();
    }

    #[test]
    fn source_directory_cleanup_rejects_a_new_reparse_object() {
        let root = tempfile::tempdir().unwrap();
        let selected = root.path().join("selected");
        let outside = root.path().join("outside");
        let template = root.path().join("template");
        std::fs::create_dir(&selected).unwrap();
        std::fs::create_dir(&outside).unwrap();
        junction(&template, &outside);
        let retarget = Retarget::new(&selected, &template);
        let mut directories = SourceDirectories::default();
        directories
            .capture(&selected, &root.path().join("destination"))
            .unwrap();
        let mut journal = Vec::new();
        let result = directories.remove_empty(
            &mut || {
                retarget.apply();
                Ok(())
            },
            &mut |source, _| journal.push(source.to_path_buf()),
        );
        retarget.clear();
        drop(retarget);
        std::fs::remove_dir(&template).unwrap();
        assert!(result.is_err());
        assert!(selected.is_dir());
        assert!(outside.is_dir());
        assert!(journal.is_empty());
    }

    #[test]
    fn source_directory_cleanup_preserves_new_children() {
        let root = tempfile::tempdir().unwrap();
        let selected = root.path().join("selected");
        std::fs::create_dir(&selected).unwrap();
        let mut directories = SourceDirectories::default();
        directories
            .capture(&selected, &root.path().join("destination"))
            .unwrap();
        let mut journal = Vec::new();
        directories
            .remove_empty(
                &mut || std::fs::write(selected.join("new"), b"arrived"),
                &mut |source, _| journal.push(source.to_path_buf()),
            )
            .unwrap();
        assert_eq!(std::fs::read(selected.join("new")).unwrap(), b"arrived");
        assert!(journal.is_empty());
    }

    #[test]
    fn source_directory_cleanup_does_not_discover_and_delete_new_empty_directories() {
        let root = tempfile::tempdir().unwrap();
        let selected = root.path().join("selected");
        std::fs::create_dir(&selected).unwrap();
        let mut directories = SourceDirectories::default();
        directories
            .capture(&selected, &root.path().join("destination"))
            .unwrap();
        let mut journal = Vec::new();
        directories
            .remove_empty(
                &mut || std::fs::create_dir(selected.join("new")),
                &mut |source, _| journal.push(source.to_path_buf()),
            )
            .unwrap();
        assert!(selected.join("new").is_dir());
        assert!(journal.is_empty());
    }

    #[test]
    fn source_directory_cleanup_is_bottom_up_and_keeps_skipped_contents() {
        let root = tempfile::tempdir().unwrap();
        let selected = root.path().join("source");
        let empty = selected.join("écriture/空");
        let skipped = selected.join("skipped");
        let target = root.path().join("target");
        std::fs::create_dir_all(&empty).unwrap();
        std::fs::create_dir(&skipped).unwrap();
        std::fs::write(skipped.join("keep"), b"skipped content").unwrap();
        let mut directories = SourceDirectories::default();
        for directory in [&selected, &selected.join("écriture"), &empty, &skipped] {
            directories
                .capture(
                    directory,
                    &target.join(directory.strip_prefix(&selected).unwrap()),
                )
                .unwrap();
        }
        let mut journal = Vec::new();
        directories
            .remove_empty(&mut || Ok(()), &mut |source, destination| {
                journal.push((source.to_path_buf(), destination.to_path_buf()))
            })
            .unwrap();
        assert_eq!(
            journal,
            vec![
                (empty, target.join("écriture/空")),
                (selected.join("écriture"), target.join("écriture"))
            ]
        );
        assert_eq!(
            std::fs::read(skipped.join("keep")).unwrap(),
            b"skipped content"
        );
        assert!(selected.is_dir());
    }

    #[test]
    fn source_directory_cleanup_cancellation_releases_handles() {
        let root = tempfile::tempdir().unwrap();
        let selected = root.path().join("source");
        std::fs::create_dir(&selected).unwrap();
        let mut directories = SourceDirectories::default();
        directories
            .capture(&selected, &root.path().join("destination"))
            .unwrap();
        let mut removed = false;
        let error = directories
            .remove_empty(
                &mut || Err(io::Error::new(io::ErrorKind::Interrupted, "cancelled")),
                &mut |_, _| removed = true,
            )
            .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::Interrupted);
        assert!(!removed);
        std::fs::rename(&selected, root.path().join("released")).unwrap();
    }
}
