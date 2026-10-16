//! Validate the complete transfer namespace before any destination is created.
//! Directory merges are allowed; files must have distinct publication names.
use crate::systems::path_parents::HeldParents;
use crate::{
    models::error::TransferDestinationCollision,
    platform::{platform, FileIdentity, FileNameRules},
};
use std::{
    collections::HashMap,
    ffi::OsString,
    fs::File,
    io,
    path::{Component, Path, PathBuf},
};

struct Node {
    directory: Option<File>,
    rules: FileNameRules,
    children: HashMap<OsString, usize>,
    file: bool,
}

struct Destinations {
    root: HeldParents,
    nodes: Vec<Node>,
    directories: HashMap<FileIdentity, usize>,
}

fn collision(path: &Path) -> io::Error {
    io::Error::other(TransferDestinationCollision {
        path: path.to_string_lossy().into_owned(),
    })
}

impl Destinations {
    fn new(path: &Path) -> io::Result<Self> {
        let root = HeldParents::new(&path.join(".oxyde-plan-root"))?;
        let directory = root.directory().try_clone()?;
        let rules = platform().file_name_rules(&directory)?;
        let identity = platform().file_identity(&directory)?;
        Ok(Self {
            root,
            nodes: vec![Node {
                directory: Some(directory),
                rules,
                children: HashMap::new(),
                file: false,
            }],
            directories: HashMap::from([(identity, 0)]),
        })
    }

    fn directory(
        &mut self,
        path: &Path,
        checkpoint: &mut dyn FnMut() -> io::Result<()>,
    ) -> io::Result<usize> {
        let relative = path
            .strip_prefix(self.root.resolved_directory())
            .map_err(|_| io::Error::other("Planned destination is outside its retained root"))?;
        let mut index = 0;
        let mut current = self.root.resolved_directory().to_path_buf();
        for component in relative.components() {
            checkpoint()?;
            let Component::Normal(name) = component else {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "Invalid planned destination component",
                ));
            };
            current.push(name);
            let key = self.nodes[index].rules.key(name);
            if let Some(&child) = self.nodes[index].children.get(&key) {
                if self.nodes[child].file {
                    return Err(collision(&current));
                }
                index = child;
                continue;
            }
            let directory = self.nodes[index]
                .directory
                .as_ref()
                .map(
                    |parent| match platform().open_source_directory(parent, &current) {
                        Ok(file) => Ok(Some(file)),
                        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
                        Err(error) => Err(error),
                    },
                )
                .transpose()?
                .flatten();
            let identity = directory
                .as_ref()
                .map(|file| platform().file_identity(file))
                .transpose()?;
            let canonical_key = directory
                .as_ref()
                .map(|file| {
                    let resolved = platform().retained_file_path(file, &current)?;
                    let name = resolved
                        .file_name()
                        .ok_or_else(|| io::Error::other("Retained directory has no name"))?;
                    Ok::<_, io::Error>(self.nodes[index].rules.key(name))
                })
                .transpose()?;
            let child = if let Some(existing) =
                identity.and_then(|id| self.directories.get(&id)).copied()
            {
                // Different spellings (including Windows short names) may select
                // the same existing parent. Its children share one namespace.
                existing
            } else {
                let rules = directory
                    .as_ref()
                    .map(|file| platform().file_name_rules(file))
                    .transpose()?
                    .unwrap_or(self.nodes[index].rules);
                let child = self.nodes.len();
                self.nodes.push(Node {
                    directory,
                    rules,
                    children: HashMap::new(),
                    file: false,
                });
                if let Some(identity) = identity {
                    self.directories.insert(identity, child);
                }
                child
            };
            self.nodes[index].children.insert(key, child);
            if let Some(key) = canonical_key {
                self.nodes[index].children.insert(key, child);
            }
            index = child;
        }
        Ok(index)
    }

    fn file(
        &mut self,
        path: &Path,
        checkpoint: &mut dyn FnMut() -> io::Result<()>,
    ) -> io::Result<()> {
        let parent = path
            .parent()
            .ok_or_else(|| io::Error::other("Planned destination has no parent"))?;
        let index = self.directory(parent, checkpoint)?;
        checkpoint()?;
        let mut name = path
            .file_name()
            .ok_or_else(|| io::Error::other("Planned destination has no name"))?
            .to_os_string();
        if let Some(directory) = &self.nodes[index].directory {
            match platform().track_source_item(directory, path) {
                Ok(file) => {
                    let resolved = platform().retained_file_path(&file, path)?;
                    name = resolved
                        .file_name()
                        .ok_or_else(|| io::Error::other("Retained destination has no name"))?
                        .to_os_string();
                }
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => return Err(error),
            }
        }
        let key = self.nodes[index].rules.key(&name);
        if self.nodes[index].children.contains_key(&key) {
            return Err(collision(path));
        }
        let child = self.nodes.len();
        self.nodes.push(Node {
            directory: None,
            rules: self.nodes[index].rules,
            children: HashMap::new(),
            file: true,
        });
        self.nodes[index].children.insert(key, child);
        Ok(())
    }
}

pub(super) fn validate<'a>(
    root: &Path,
    directories: &[PathBuf],
    files: impl Iterator<Item = &'a PathBuf>,
    checkpoint: &mut dyn FnMut() -> io::Result<()>,
) -> io::Result<()> {
    let mut destinations = Destinations::new(root)?;
    for path in directories {
        destinations.directory(path, checkpoint)?;
    }
    for path in files {
        destinations.file(path, checkpoint)?;
    }
    checkpoint()
}
