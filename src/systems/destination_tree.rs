//! Retain a copy destination before planning, and bind every child creation.
use super::path_parents::HeldParents;
use crate::platform::platform;
use std::{
    collections::HashMap,
    fs::File,
    io,
    path::{Component, Path, PathBuf},
    sync::Mutex,
};

pub(super) struct DestinationTree {
    root: HeldParents,
    directories: Mutex<HashMap<PathBuf, File>>,
}

impl DestinationTree {
    pub fn new(path: &Path) -> io::Result<Self> {
        // The placeholder is never created: HeldParents binds its directory.
        let root = HeldParents::new(&path.join(".oxyde-destination-root"))?;
        Ok(Self {
            root,
            directories: Mutex::new(HashMap::new()),
        })
    }

    pub fn directory_identity(
        &self,
        path: &Path,
    ) -> io::Result<crate::models::transaction::HistoryDirectoryIdentity> {
        let directories = self.directories.lock().unwrap();
        let file = directories
            .get(path)
            .ok_or_else(|| io::Error::other("Created directory was not retained"))?;
        crate::models::transaction::HistoryDirectoryIdentity::from_file(file)
    }

    pub fn path(&self) -> &Path {
        self.root.resolved_directory()
    }

    pub fn check_all(&self, checkpoint: &mut dyn FnMut() -> io::Result<()>) -> io::Result<()> {
        checkpoint()?;
        ordinary(self.root.directory())?;
        let directories = self.directories.lock().unwrap();
        for (path, directory) in directories.iter() {
            checkpoint()?;
            ordinary(directory).map_err(|error| {
                io::Error::new(
                    error.kind(),
                    format!("Copy destination directory {:?} changed: {error}", path),
                )
            })?;
        }
        Ok(())
    }

    /// Check only the already retained destination: never recreate a missing child.
    pub fn check_directory(
        &self,
        path: &Path,
        checkpoint: &mut dyn FnMut() -> io::Result<()>,
    ) -> io::Result<()> {
        let relative = path
            .strip_prefix(self.path())
            .map_err(|_| io::Error::other("Directory is outside the retained copy destination"))?;
        checkpoint()?;
        ordinary(self.root.directory())?;
        let mut resolved = self.path().to_path_buf();
        for component in relative.components() {
            let Component::Normal(name) = component else {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "Copy destination contains a non-normal component",
                ));
            };
            checkpoint()?;
            resolved.push(name);
            let directories = self.directories.lock().unwrap();
            let directory = directories
                .get(&resolved)
                .ok_or_else(|| io::Error::other("Copy destination directory was not retained"))?;
            ordinary(directory)?;
        }
        Ok(())
    }

    pub fn ensure_directory(
        &self,
        path: &Path,
        checkpoint: &mut dyn FnMut() -> io::Result<()>,
        created: &mut dyn FnMut(&Path),
    ) -> io::Result<HeldParents> {
        let relative = path
            .strip_prefix(self.path())
            .map_err(|_| io::Error::other("Directory is outside the retained copy destination"))?;
        let names = relative
            .components()
            .map(|component| match component {
                Component::Normal(name) => Ok(name),
                _ => Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "Copy destination contains a non-normal component",
                )),
            })
            .collect::<io::Result<Vec<_>>>()?;
        let mut directory = self.root.directory().try_clone()?;
        let mut resolved = self.path().to_path_buf();
        let mut ancestors = Vec::with_capacity(names.len());
        for name in names {
            checkpoint()?;
            ordinary(&directory)?;
            resolved.push(name);
            let (child, is_new) = {
                // Serialise first opens only. All copy data stays outside this lock.
                let mut directories = self.directories.lock().unwrap();
                if let Some(held) = directories.get(&resolved) {
                    (held.try_clone(), false)
                } else {
                    let (held, is_new) =
                        platform().ensure_operation_directory(&directory, &resolved)?;
                    let child = held.try_clone();
                    directories.insert(resolved.clone(), held);
                    (child, is_new)
                }
            };
            if is_new {
                created(&resolved);
            }
            // Creation belongs in the journal even if duplicating its handle
            // fails. The original retained handle already resides in the cache.
            let child = child?;
            ancestors.push(directory);
            directory = child;
        }
        checkpoint()?;
        ordinary(&directory)?;
        self.root
            .with_bound_directory(directory, ancestors, resolved)
    }

    /// A same-folder copy root must be new; never merge into a racing creation.
    pub fn reserve_copy_directory(&self, path: &Path, created: &mut dyn FnMut(&Path)) -> io::Result<()> {
        if path.parent() != Some(self.path()) {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "Copy root is outside destination"));
        }
        ordinary(self.root.directory())?;
        let (file, is_new) = platform().ensure_operation_directory(self.root.directory(), path)?;
        if !is_new {
            return Err(io::Error::new(io::ErrorKind::AlreadyExists, "New copy directory already exists"));
        }
        self.directories.lock().unwrap().insert(path.to_path_buf(), file);
        created(path);
        Ok(())
    }

    pub fn file_parents(
        &self,
        path: &Path,
        checkpoint: &mut dyn FnMut() -> io::Result<()>,
        created: &mut dyn FnMut(&Path),
    ) -> io::Result<HeldParents> {
        let parent = path
            .parent()
            .ok_or_else(|| io::Error::other("Copy destination has no parent"))?;
        self.ensure_directory(parent, checkpoint, created)
    }
}

fn ordinary(directory: &File) -> io::Result<()> {
    let metadata = directory.metadata()?;
    if !metadata.is_dir() || platform().file_attributes(&metadata, "").2 {
        return Err(io::Error::other(
            "Copy destination directory changed during the operation",
        ));
    }
    Ok(())
}

#[cfg(all(test, target_os = "windows"))]
mod tests {
    use super::*;
    use crate::platform::windows_reparse_tests::{junction, Retarget};
    use std::{fs, sync::Arc};

    #[test]
    fn new_copy_root_refuses_an_existing_directory_instead_of_merging_into_it() {
        let root = tempfile::tempdir().unwrap();
        let tree = DestinationTree::new(root.path()).unwrap();
        let copy = tree.path().join("Dossier - Copie");
        fs::create_dir(&copy).unwrap();
        fs::write(copy.join("witness.txt"), "keep").unwrap();
        assert_eq!(tree.reserve_copy_directory(&copy, &mut |_| panic!("existing root journaled as new"))
            .unwrap_err().kind(), io::ErrorKind::AlreadyExists);
        assert_eq!(fs::read(copy.join("witness.txt")).unwrap(), b"keep");
    }

    #[test]
    fn destination_tree_records_only_atomic_creations_across_workers() {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir(root.path().join("existing")).unwrap();
        let tree = DestinationTree::new(root.path()).unwrap();
        let directory = tree.path().join("existing/Données 日本語/empty");
        let created = Mutex::new(Vec::new());
        std::thread::scope(|scope| {
            for _ in 0..8 {
                let (tree, directory, created) = (&tree, &directory, &created);
                scope.spawn(move || {
                    tree.ensure_directory(directory, &mut || Ok(()), &mut |path| {
                        created.lock().unwrap().push(path.to_path_buf());
                    })
                    .unwrap();
                });
            }
        });
        let paths = created.into_inner().unwrap();
        assert_eq!(paths.len(), 2);
        assert!(paths.contains(&directory));
        assert!(paths.contains(&directory.parent().unwrap().to_path_buf()));
        assert!(fs::rename(&directory, directory.with_file_name("replaced")).is_err());
        drop(tree);
        fs::rename(&directory, directory.with_file_name("released")).unwrap();
    }

    #[test]
    fn destination_tree_refuses_new_reparse_points_between_creation_steps() {
        let root = tempfile::tempdir().unwrap();
        let selected = root.path().join("selected");
        let outside = root.path().join("outside");
        fs::create_dir(&selected).unwrap();
        fs::create_dir(&outside).unwrap();
        fs::write(outside.join("witness"), b"unchanged").unwrap();
        let template = root.path().join("template");
        junction(&template, &outside);
        let tree = DestinationTree::new(&selected).unwrap();
        let first = tree.path().join("new");
        let target = first.join("nested");
        let mut mutation = None;
        let mut created = Vec::new();
        let result = tree.ensure_directory(&target, &mut || Ok(()), &mut |path| {
            created.push(path.to_path_buf());
            let retarget = Retarget::new(path, &template);
            retarget.apply();
            mutation = Some(retarget);
        });
        assert!(result.is_err());
        let retarget = mutation.take().unwrap();
        retarget.clear();
        assert_eq!(created, vec![first.clone()]);
        assert!(!first.join("nested").exists());
        assert_eq!(fs::read(outside.join("witness")).unwrap(), b"unchanged");
        assert_eq!(fs::read_dir(&outside).unwrap().count(), 1);
        drop((retarget, tree));
        fs::rename(first, selected.join("released")).unwrap();
        fs::remove_dir(template).unwrap();
    }

    #[test]
    fn destination_tree_keeps_an_initial_selected_junction_target_for_copy_and_move() {
        let root = tempfile::tempdir().unwrap();
        let selected = root.path().join("selected");
        let outside = root.path().join("outside");
        fs::create_dir(&selected).unwrap();
        fs::create_dir(&outside).unwrap();
        fs::write(outside.join("witness"), b"unchanged").unwrap();
        let link = root.path().join("link");
        let template = root.path().join("template");
        junction(&link, &selected);
        junction(&template, &outside);
        let retarget = Retarget::new(&link, &template);
        let tree = DestinationTree::new(&link).unwrap();
        let source = root.path().join("copy-source");
        let move_source = root.path().join("move-source");
        fs::write(&source, b"copy data").unwrap();
        fs::write(&move_source, b"move data").unwrap();
        retarget.apply();
        let copied_path = tree.path().join("new/nested/copy");
        let parents = tree
            .file_parents(&copied_path, &mut || Ok(()), &mut |_| {})
            .unwrap();
        let output = super::super::file_copy::copy_with_parents(
            &source,
            &copied_path,
            false,
            None,
            parents,
            &mut |_| Ok(()),
        )
        .unwrap();
        drop(output);
        let moved_path = tree.path().join("new/nested/move");
        let parents = tree
            .file_parents(&moved_path, &mut || Ok(()), &mut |_| {})
            .unwrap();
        super::super::path_parents::move_file_with_parents(
            &move_source,
            &moved_path,
            false,
            parents,
            &mut || Ok(()),
        )
        .unwrap();
        assert_eq!(
            fs::read(selected.join("new/nested/copy")).unwrap(),
            b"copy data"
        );
        assert_eq!(
            fs::read(selected.join("new/nested/move")).unwrap(),
            b"move data"
        );
        assert!(!move_source.exists());
        assert_eq!(fs::read_dir(&outside).unwrap().count(), 1);
        assert_eq!(fs::read(outside.join("witness")).unwrap(), b"unchanged");
        drop((retarget, tree));
        fs::remove_dir(link).unwrap();
        fs::remove_dir(template).unwrap();
    }

    #[test]
    fn destination_tree_rejects_outside_paths_and_parent_traversal() {
        let root = tempfile::tempdir().unwrap();
        let selected = root.path().join("selected");
        fs::create_dir(&selected).unwrap();
        let tree = DestinationTree::new(&selected).unwrap();
        for path in [
            root.path().join("outside/new"),
            tree.path().join("../outside/new"),
        ] {
            let mut created = Vec::new();
            assert!(tree
                .ensure_directory(&path, &mut || Ok(()), &mut |path| created
                    .push(path.to_path_buf()))
                .is_err());
            assert!(created.is_empty());
        }
        assert!(!root.path().join("outside").exists());
    }

    #[test]
    fn destination_tree_cancellation_records_partial_creation_and_releases_handles() {
        let root = tempfile::tempdir().unwrap();
        let tree = DestinationTree::new(root.path()).unwrap();
        let first = tree.path().join("first");
        let target = first.join("never");
        let mut calls = 0;
        let mut created = Vec::new();
        let error = tree
            .ensure_directory(
                &target,
                &mut || {
                    calls += 1;
                    if calls == 2 {
                        Err(io::Error::new(io::ErrorKind::Interrupted, "test stop"))
                    } else {
                        Ok(())
                    }
                },
                &mut |path| created.push(path.to_path_buf()),
            )
            .err()
            .unwrap();
        assert_eq!(error.kind(), io::ErrorKind::Interrupted);
        assert_eq!(created, vec![first.clone()]);
        assert!(!first.join("never").exists());
        drop(tree);
        fs::rename(&first, first.with_file_name("released")).unwrap();
    }

    #[test]
    fn destination_tree_cached_handles_refuse_replacement_until_operation_end() {
        let root = tempfile::tempdir().unwrap();
        let tree = Arc::new(DestinationTree::new(root.path()).unwrap());
        let selected = tree.path().join("held");
        let path = selected.join("output");
        drop(
            tree.file_parents(&path, &mut || Ok(()), &mut |_| {})
                .unwrap(),
        );
        let renamed = tree.path().join("renamed");
        assert!(fs::rename(&selected, &renamed).is_err());
        let other = tree.clone();
        std::thread::spawn(move || {
            assert!(fs::remove_dir(other.path().join("held")).is_err());
        })
        .join()
        .unwrap();
        let parents = tree
            .file_parents(&path, &mut || Ok(()), &mut |_| {})
            .unwrap();
        drop(tree);
        assert!(fs::rename(&selected, &renamed).is_err());
        drop(parents);
        fs::rename(selected, renamed).unwrap();
    }
}
