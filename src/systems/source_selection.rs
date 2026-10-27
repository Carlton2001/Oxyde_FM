//! Capture sources before traversal and bind every child to its opened parent.
use super::path_parents::HeldParents;
use crate::{
    models::{error::SourceSelectionFailure, transaction::HistoryFileIdentity},
    platform::platform,
};
use std::{
    fs::{File, Metadata},
    io,
    path::{Path, PathBuf},
    sync::Arc,
};

struct BoundDirectory {
    file: File,
    resolved: PathBuf,
    root: Arc<HeldParents>,
    ancestor: Option<Arc<BoundDirectory>>,
}
impl Drop for BoundDirectory {
    fn drop(&mut self) {
        // Avoid recursive Arc destruction on very deep source trees.
        let mut ancestor = self.ancestor.take();
        while let Some(parent) = ancestor {
            match Arc::try_unwrap(parent) {
                Ok(mut parent) => ancestor = parent.ancestor.take(),
                Err(_) => break,
            }
        }
    }
}
impl BoundDirectory {
    fn parents(&self) -> io::Result<HeldParents> {
        let mut ancestors = Vec::new();
        let mut ancestor = self.ancestor.as_deref();
        while let Some(parent) = ancestor {
            ancestors.push(parent.file.try_clone()?);
            ancestor = parent.ancestor.as_deref();
        }
        self.root
            .with_bound_directory(self.file.try_clone()?, ancestors, self.resolved.clone())
    }
}

#[derive(Clone)]
pub(super) struct SelectedFile {
    path: PathBuf,
    parent: Arc<BoundDirectory>,
    identity: HistoryFileIdentity,
    pub size: u64,
}
impl SelectedFile {
    pub fn path(&self) -> &Path {
        &self.path
    }
    pub fn parents(&self) -> io::Result<HeldParents> {
        self.parent.parents()
    }
    pub fn resolved_path(&self) -> PathBuf {
        self.parent.resolved.join(self.path.file_name().unwrap())
    }
    pub fn verify(&self, file: &File) -> io::Result<()> {
        let metadata = file.metadata()?;
        if !metadata.is_file() || platform().file_attributes(&metadata, "").2 {
            return Err(SourceSelectionFailure::changed(&self.path));
        }
        match self.identity.matches(file) {
            Ok(true) => Ok(()),
            Ok(false) => Err(SourceSelectionFailure::changed(&self.path)),
            Err(_) => Err(SourceSelectionFailure::unverified(&self.path)),
        }
    }
    pub fn open_copy(&self) -> io::Result<(HeldParents, File)> {
        let parents = self.parents()?;
        let file = platform()
            .open_copy_source(parents.directory(), &self.resolved_path())
            .map_err(|error| self.open_error(error))?;
        self.verify(&file)?;
        Ok((parents, file))
    }
    pub fn open_error(&self, error: io::Error) -> io::Error {
        if error.kind() == io::ErrorKind::NotFound {
            SourceSelectionFailure::changed(&self.path)
        } else {
            // Native no-follow opening can reject a newly installed directory or
            // link before returning a handle. Classify only a verified replacement;
            // otherwise preserve the actual access/sharing/network error.
            if let Ok(current) = platform().track_source_item(&self.parent.file, &self.path) {
                if let Err(changed) = self.verify(&current) {
                    return changed;
                }
            }
            error
        }
    }
}

/// A symbolic link transferred as a link: its own identity and target text,
/// captured without following it.
#[derive(Clone)]
pub(super) struct SelectedLink {
    path: PathBuf,
    parent: Arc<BoundDirectory>,
    identity: HistoryFileIdentity,
    target: PathBuf,
}
impl SelectedLink {
    pub fn path(&self) -> &Path {
        &self.path
    }
    pub fn target(&self) -> &Path {
        &self.target
    }
    /// Confirm through the held parent that the same link still has the same target.
    pub fn verify(&self) -> io::Result<()> {
        let current = platform()
            .track_source_item(&self.parent.file, &self.path)
            .map_err(|error| match error.kind() {
                io::ErrorKind::NotFound => SourceSelectionFailure::changed(&self.path),
                _ => error,
            })?;
        self.matches(&current)
    }
    fn matches(&self, current: &File) -> io::Result<()> {
        let metadata = current.metadata()?;
        let target = platform().source_link_target(current, &metadata);
        match (self.identity.matches(current), target) {
            (Ok(true), Some(Ok(target))) if target == self.target => Ok(()),
            (Ok(_), Some(Ok(_)) | None) => Err(SourceSelectionFailure::changed(&self.path)),
            _ => Err(SourceSelectionFailure::unverified(&self.path)),
        }
    }
    /// Move this same link into the held destination parent and return it.
    pub fn move_to(&self, parent: &File, destination: &Path, replace: bool) -> io::Result<File> {
        super::link_moves::move_link(
            &self.parent.file,
            &self.path,
            parent,
            destination,
            &self.target,
            replace,
            &mut |current| self.matches(current),
        )
        .map_err(|error| match error.kind() {
            io::ErrorKind::NotFound => SourceSelectionFailure::changed(&self.path),
            _ => error,
        })
    }
}

#[derive(Clone)]
pub(super) struct SelectedEntry {
    path: PathBuf,
    parent: Arc<BoundDirectory>,
    directory: Option<Arc<BoundDirectory>>,
    file: Option<SelectedFile>,
    link: Option<SelectedLink>,
}
impl SelectedEntry {
    pub fn select(path: &Path) -> io::Result<Self> {
        let root = Arc::new(HeldParents::new(path)?);
        let parent = Arc::new(BoundDirectory {
            file: root.directory().try_clone()?,
            resolved: root.resolved_directory().to_path_buf(),
            root,
            ancestor: None,
        });
        Self::child(path.to_path_buf(), parent)
    }
    fn child(path: PathBuf, parent: Arc<BoundDirectory>) -> io::Result<Self> {
        let captured = platform().track_source_item(&parent.file, &path)?;
        let metadata: Metadata = captured.metadata()?;
        if let Some(target) = platform().source_link_target(&captured, &metadata) {
            let link = SelectedLink {
                path: path.clone(),
                parent: Arc::clone(&parent),
                identity: HistoryFileIdentity::from_file(&captured)
                    .map_err(|_| SourceSelectionFailure::unverified(&path))?,
                target: target?,
            };
            return Ok(Self {
                path,
                parent,
                directory: None,
                file: None,
                link: Some(link),
            });
        }
        if platform().file_attributes(&metadata, "").2
            || (!metadata.is_dir() && !metadata.is_file())
        {
            return Err(SourceSelectionFailure::changed(&path));
        }
        let identity = HistoryFileIdentity::from_file(&captured)
            .map_err(|_| SourceSelectionFailure::unverified(&path))?;
        let (directory, file) = if metadata.is_dir() {
            let reader = platform().open_source_directory(&parent.file, &path)?;
            if !identity.matches(&reader)? {
                return Err(SourceSelectionFailure::changed(&path));
            }
            (
                Some(Arc::new(BoundDirectory {
                    file: reader,
                    resolved: parent.resolved.join(path.file_name().unwrap()),
                    root: Arc::clone(&parent.root),
                    ancestor: Some(Arc::clone(&parent)),
                })),
                None,
            )
        } else {
            (
                None,
                Some(SelectedFile {
                    path: path.clone(),
                    parent: Arc::clone(&parent),
                    identity,
                    size: metadata.len(),
                }),
            )
        };
        Ok(Self {
            path,
            parent,
            directory,
            file,
            link: None,
        })
    }
    pub fn path(&self) -> &Path {
        &self.path
    }
    pub fn resolved_path(&self) -> PathBuf {
        self.parent.resolved.join(self.path.file_name().unwrap())
    }
    pub fn is_dir(&self) -> bool {
        self.directory.is_some()
    }
    pub fn link(&self) -> Option<&SelectedLink> {
        self.link.as_ref()
    }
    /// Mode and dates of the held directory, read before its listing changes its access date.
    pub fn directory_metadata(&self) -> io::Result<Metadata> {
        self.directory
            .as_ref()
            .ok_or_else(|| io::Error::other("Selected source is not a directory"))?
            .file
            .metadata()
    }
    pub fn into_file(self) -> io::Result<SelectedFile> {
        self.file
            .ok_or_else(|| io::Error::other("Selected source is not a file"))
    }
    pub fn size(&self) -> u64 {
        self.file.as_ref().map_or(0, |file| file.size)
    }
    /// Keep the same object and its namespace held through encoder consumption.
    pub fn open_archive(&self) -> io::Result<(HeldParents, File)> {
        if let Some(file) = &self.file {
            return file.open_copy();
        }
        let directory = self
            .directory
            .as_ref()
            .ok_or_else(|| SourceSelectionFailure::unverified(&self.path))?;
        let metadata = directory.file.metadata()?;
        if !metadata.is_dir() || platform().file_attributes(&metadata, "").2 {
            return Err(SourceSelectionFailure::changed(&self.path));
        }
        Ok((self.parent.parents()?, directory.file.try_clone()?))
    }
    pub fn open_retained_archive(&self, input: &File) -> io::Result<(HeldParents, File)> {
        use std::io::{Seek, SeekFrom};
        let selected = self
            .file
            .as_ref()
            .ok_or_else(|| SourceSelectionFailure::changed(&self.path))?;
        selected.verify(input)?;
        let mut file = input.try_clone()?;
        file.seek(SeekFrom::Start(0))?;
        Ok((selected.parents()?, file))
    }
    pub fn capture_directory(
        &self,
        destination: &Path,
        sources: &mut super::source_directories::SourceDirectories,
    ) -> io::Result<()> {
        let directory = self
            .directory
            .as_ref()
            .ok_or_else(|| io::Error::other("Selected source is not a directory"))?;
        sources.capture_bound(
            &self.path,
            destination,
            self.parent.parents()?,
            &directory.file,
        )
    }
}

pub(super) struct SourceWalk {
    stack: Vec<SelectedEntry>,
    pending: Option<SelectedEntry>,
    capacity: Option<(usize, usize)>,
}
impl SourceWalk {
    pub fn new(root: SelectedEntry) -> Self {
        Self {
            stack: vec![root],
            pending: None,
            capacity: None,
        }
    }
    pub fn skip_current_dir(&mut self) {
        self.pending = None;
    }
    pub fn bound_pending(&mut self, remaining: usize, limit: usize) { self.capacity = Some((remaining, limit)); }
    pub fn next(
        &mut self,
        checkpoint: &mut dyn FnMut() -> io::Result<()>,
    ) -> io::Result<Option<SelectedEntry>> {
        checkpoint()?;
        if let Some(entry) = self.pending.take() {
            let directory = entry.directory.unwrap();
            let metadata = directory.file.metadata()?;
            if !metadata.is_dir() || platform().file_attributes(&metadata, "").2 {
                return Err(SourceSelectionFailure::changed(&entry.path));
            }
            let mut children = Vec::new();
            platform().enumerate_source_directory(&directory.file, &mut |name| {
                checkpoint()?;
                if let Some((remaining, limit)) = self.capacity {
                    if self.stack.len().saturating_add(children.len()) >= remaining {
                        return Err(io::Error::new(io::ErrorKind::InvalidData, crate::models::error::ArchiveLimitFailure { kind: crate::models::error::ArchiveLimitKind::Entries, limit: limit as u64 }));
                    }
                }
                children.push(SelectedEntry::child(
                    entry.path.join(name),
                    Arc::clone(&directory),
                )?);
                Ok(())
            })?;
            self.stack.extend(children.into_iter().rev());
        }
        let entry = self.stack.pop();
        if let Some(entry) = entry.as_ref().filter(|entry| entry.is_dir()) {
            self.pending = Some(entry.clone());
        }
        Ok(entry)
    }
}

#[cfg(all(test, target_os = "windows"))]
mod tests {
    use super::*;
    use crate::platform::windows_reparse_tests::{Retarget, junction};
    use std::fs;

    #[test]
    fn planned_source_replacement_is_refused_by_copy_and_move_before_publication() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("source");
        let original = root.path().join("original");
        let destination = root.path().join("destination");
        fs::write(&path, b"same bytes").unwrap();
        fs::write(&destination, b"destination witness").unwrap();
        let selected = SelectedEntry::select(&path).unwrap().into_file().unwrap();
        fs::rename(&path, &original).unwrap();
        fs::write(&path, b"same bytes").unwrap();
        let error = selected.open_copy().err().unwrap();
        assert_eq!(
            SourceSelectionFailure::from_message(&error.to_string()),
            Some(true)
        );
        let error = super::super::path_parents::move_selected_file_with_parents(
            &selected,
            &destination,
            true,
            HeldParents::new(&destination).unwrap(),
            &mut || Ok(()),
        )
        .unwrap_err();
        assert_eq!(
            SourceSelectionFailure::from_message(&error.to_string()),
            Some(true)
        );
        assert_eq!(fs::read(&path).unwrap(), b"same bytes");
        assert_eq!(fs::read(&original).unwrap(), b"same bytes");
        assert_eq!(fs::read(&destination).unwrap(), b"destination witness");
        fs::remove_file(&path).unwrap();
        fs::rename(&original, &path).unwrap();
        let (_, file) = selected.open_copy().unwrap();
        selected.verify(&file).unwrap();
    }

    #[test]
    fn planned_source_replaced_by_directory_or_junction_has_explained_failure() {
        for link in [false, true] {
            let root = tempfile::tempdir().unwrap();
            let path = root.path().join("source");
            let original = root.path().join("original");
            let outside = root.path().join("outside");
            fs::write(&path, b"original").unwrap();
            fs::create_dir(&outside).unwrap();
            fs::write(outside.join("witness"), b"outside witness").unwrap();
            let selected = SelectedEntry::select(&path).unwrap().into_file().unwrap();
            fs::rename(&path, &original).unwrap();
            if link {
                junction(&path, &outside);
            } else {
                fs::create_dir(&path).unwrap();
            }
            let error = selected.open_copy().err().unwrap();
            assert_eq!(
                SourceSelectionFailure::from_message(&error.to_string()),
                Some(true)
            );
            let destination = root.path().join("destination");
            let error = super::super::path_parents::move_selected_file_with_parents(
                &selected,
                &destination,
                false,
                HeldParents::new(&destination).unwrap(),
                &mut || Ok(()),
            )
            .unwrap_err();
            assert_eq!(
                SourceSelectionFailure::from_message(&error.to_string()),
                Some(true)
            );
            assert!(!destination.exists());
            assert_eq!(fs::read(&original).unwrap(), b"original");
            assert_eq!(
                fs::read(outside.join("witness")).unwrap(),
                b"outside witness"
            );
            drop(selected);
            fs::remove_dir(path).unwrap();
        }
    }

    #[test]
    fn planned_copy_and_move_use_bound_parent_after_junction_retargeting() {
        for is_move in [false, true] {
            let root = tempfile::tempdir().unwrap();
            let source_dir = root.path().join("chosen");
            let outside = root.path().join("outside");
            fs::create_dir(&source_dir).unwrap();
            fs::create_dir(&outside).unwrap();
            fs::write(source_dir.join("source"), b"chosen content").unwrap();
            fs::write(outside.join("source"), b"outside witness").unwrap();
            let link = root.path().join("link");
            let template = root.path().join("template");
            junction(&link, &source_dir);
            junction(&template, &outside);
            let retarget = Retarget::new(&link, &template);
            let selected = SelectedEntry::select(&link.join("source"))
                .unwrap()
                .into_file()
                .unwrap();
            retarget.apply();
            let destination = root.path().join("destination");
            if is_move {
                super::super::path_parents::move_selected_file_with_parents(
                    &selected,
                    &destination,
                    false,
                    HeldParents::new(&destination).unwrap(),
                    &mut || Ok(()),
                )
                .unwrap();
                assert!(!source_dir.join("source").exists());
            } else {
                let (_parents, file) = selected.open_copy().unwrap();
                super::super::file_copy::copy_with_parents(
                    &selected.resolved_path(),
                    &destination,
                    false,
                    Some(&file),
                    HeldParents::new(&destination).unwrap(),
                    &mut |_| Ok(()),
                )
                .unwrap();
                assert_eq!(
                    fs::read(source_dir.join("source")).unwrap(),
                    b"chosen content"
                );
            }
            assert_eq!(fs::read(destination).unwrap(), b"chosen content");
            assert_eq!(
                fs::read(outside.join("source")).unwrap(),
                b"outside witness"
            );
            drop(selected);
            drop(retarget);
            fs::remove_dir(link).unwrap();
            fs::remove_dir(template).unwrap();
        }
    }

    #[test]
    fn planned_source_walk_never_traverses_reparse_added_to_selected_empty_directory() {
        let root = tempfile::tempdir().unwrap();
        let chosen = root.path().join("chosen");
        let outside = root.path().join("outside");
        let template = root.path().join("template");
        fs::create_dir(&chosen).unwrap();
        fs::create_dir(&outside).unwrap();
        fs::write(outside.join("witness"), b"outside witness").unwrap();
        junction(&template, &outside);
        let retarget = Retarget::new(&chosen, &template);
        let mut walk = SourceWalk::new(SelectedEntry::select(&chosen).unwrap());
        assert!(walk.next(&mut || Ok(())).unwrap().unwrap().is_dir());
        retarget.apply();
        assert!(walk.next(&mut || Ok(())).is_err());
        assert_eq!(
            fs::read(outside.join("witness")).unwrap(),
            b"outside witness"
        );
        retarget.clear();
        drop(walk);
        drop(retarget);
        fs::remove_dir(template).unwrap();
    }

    #[test]
    fn planned_source_directory_names_remain_fixed_until_plan_release() {
        let root = tempfile::tempdir().unwrap();
        let chosen = root.path().join("chosen");
        fs::create_dir_all(chosen.join("nested")).unwrap();
        fs::write(chosen.join("nested/data"), b"chosen").unwrap();
        let mut walk = SourceWalk::new(SelectedEntry::select(&chosen).unwrap());
        let mut file = None;
        while let Some(entry) = walk.next(&mut || Ok(())).unwrap() {
            if !entry.is_dir() {
                file = Some(entry.into_file().unwrap());
            }
        }
        assert!(fs::rename(&chosen, root.path().join("replacement")).is_err());
        assert!(fs::rename(chosen.join("nested"), chosen.join("other")).is_err());
        drop(walk);
        assert!(fs::rename(&chosen, root.path().join("replacement")).is_err());
        drop(file);
        fs::rename(&chosen, root.path().join("released")).unwrap();
    }

    #[test]
    fn native_source_walk_enumerates_wide_deep_unicode_tree_and_restarts() {
        let root = tempfile::tempdir().unwrap();
        let chosen = root.path().join("chosen");
        fs::create_dir(&chosen).unwrap();
        for index in 0..300 {
            fs::write(chosen.join(format!("{index}-écriture-空")), b"data").unwrap();
        }
        let mut deepest = chosen.clone();
        for _ in 0..80 {
            deepest.push("d");
            fs::create_dir(&deepest).unwrap();
        }
        fs::write(deepest.join("last"), b"deep").unwrap();
        let selected = SelectedEntry::select(&chosen).unwrap();
        for _ in 0..2 {
            let mut walk = SourceWalk::new(selected.clone());
            let (mut directories, mut files) = (0, 0);
            while let Some(entry) = walk.next(&mut || Ok(())).unwrap() {
                if entry.is_dir() {
                    directories += 1;
                } else {
                    files += 1;
                }
            }
            assert_eq!((directories, files), (81, 301));
        }
    }

    #[test]
    fn planned_source_skip_does_not_enumerate_unselected_descendants() {
        let root = tempfile::tempdir().unwrap();
        let chosen = root.path().join("chosen");
        let outside = root.path().join("outside");
        fs::create_dir_all(chosen.join("skipped")).unwrap();
        fs::create_dir(&outside).unwrap();
        fs::write(outside.join("witness"), b"outside witness").unwrap();
        junction(&chosen.join("skipped/link"), &outside);
        fs::write(chosen.join("kept"), b"selected").unwrap();
        let mut walk = SourceWalk::new(SelectedEntry::select(&chosen).unwrap());
        let mut files = 0;
        while let Some(entry) = walk.next(&mut || Ok(())).unwrap() {
            if entry.path().ends_with("skipped") {
                walk.skip_current_dir();
            } else if !entry.is_dir() {
                files += 1;
            }
        }
        assert_eq!(files, 1);
        drop(walk);
        fs::remove_dir(chosen.join("skipped/link")).unwrap();
        assert_eq!(
            fs::read(outside.join("witness")).unwrap(),
            b"outside witness"
        );
    }

    #[test]
    fn native_source_enumeration_cancellation_releases_parent_pins() {
        let root = tempfile::tempdir().unwrap();
        let chosen = root.path().join("chosen");
        fs::create_dir(&chosen).unwrap();
        for index in 0..30 {
            fs::write(chosen.join(index.to_string()), b"data").unwrap();
        }
        let mut walk = SourceWalk::new(SelectedEntry::select(&chosen).unwrap());
        walk.next(&mut || Ok(())).unwrap();
        let mut calls = 0;
        let error = walk
            .next(&mut || {
                calls += 1;
                if calls > 10 {
                    Err(io::ErrorKind::Interrupted.into())
                } else {
                    Ok(())
                }
            })
            .err()
            .unwrap();
        assert_eq!(error.kind(), io::ErrorKind::Interrupted);
        drop(walk);
        fs::rename(&chosen, root.path().join("released")).unwrap();
    }

    #[test]
    fn planned_copied_move_source_is_verified_on_its_deletion_handle() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("source");
        let destination = root.path().join("destination");
        fs::write(&path, b"selected").unwrap();
        fs::write(root.path().join("source:stream"), b"alternate").unwrap();
        let selected = SelectedEntry::select(&path).unwrap().into_file().unwrap();
        let source = super::super::copied_source::CopiedSource::open_selected(&selected).unwrap();
        let output = super::super::file_copy::copy_with_parents(
            &selected.resolved_path(),
            &destination,
            false,
            Some(source.file()),
            HeldParents::new(&destination).unwrap(),
            &mut |_| Ok(()),
        )
        .unwrap();
        source.finish(output, &mut || Ok(())).unwrap();
        assert!(!path.exists());
        assert_eq!(fs::read(destination).unwrap(), b"selected");
        assert_eq!(
            fs::read(root.path().join("destination:stream")).unwrap(),
            b"alternate"
        );
    }
}
