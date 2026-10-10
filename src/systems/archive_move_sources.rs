//! Keep copied archive-transfer sources alive through publication and native deletion.
use super::{
    copied_source::CopiedSource, destination_tree::DestinationTree, file_copy::CopiedFile,
    source_directories::SourceDirectories,
};
use std::{
    io,
    path::{Path, PathBuf},
    sync::Arc,
};

struct Entry {
    path: PathBuf,
    destination: PathBuf,
    source: CopiedSource,
    output: CopiedFile,
}

#[derive(Default)]
pub(super) struct ArchiveMoveSources {
    entries: Vec<Entry>,
    verified: bool,
    directories: SourceDirectories,
    destination: Option<Arc<DestinationTree>>,
}

impl ArchiveMoveSources {
    pub fn retain_directories(
        &mut self,
        directories: SourceDirectories,
        destination: Arc<DestinationTree>,
    ) {
        self.directories = directories;
        self.destination = Some(destination);
    }

    pub fn remove_directories_matching(
        &mut self,
        matches: &dyn Fn(&Path) -> bool,
        checkpoint: &mut dyn FnMut() -> io::Result<()>,
        removed: &mut dyn FnMut(&Path, &Path),
    ) -> io::Result<()> {
        self.remove_directories_matching_observed(
            matches,
            checkpoint,
            &mut |source, destination, _| {
                removed(source, destination);
                Ok(())
            },
        )
    }
    pub fn remove_directories_matching_observed(
        &mut self,
        matches: &dyn Fn(&Path) -> bool,
        checkpoint: &mut dyn FnMut() -> io::Result<()>,
        removed: &mut dyn FnMut(&Path, &Path, crate::platform::FileIdentity) -> io::Result<()>,
    ) -> io::Result<()> {
        if !self.verified {
            return Err(io::Error::other(
                "Archive move source has not been verified",
            ));
        }
        let Some(destination) = &self.destination else {
            return Ok(());
        };
        self.directories.remove_matching_observed(
            matches,
            checkpoint,
            &mut |path, checkpoint| destination.check_directory(path, checkpoint),
            removed,
        )
    }
    pub fn record(
        &mut self,
        path: PathBuf,
        destination: PathBuf,
        source: CopiedSource,
        output: CopiedFile,
    ) {
        self.verified = false;
        self.entries.push(Entry {
            path,
            destination,
            source,
            output,
        });
    }

    pub fn retained_outputs(
        &self,
    ) -> io::Result<std::collections::HashMap<PathBuf, std::fs::File>> {
        self.entries
            .iter()
            .map(|entry| {
                Ok((
                    entry.output._parents.resolved_path(&entry.destination)?,
                    entry.output.file.try_clone()?,
                ))
            })
            .collect()
    }
    pub fn verify_all(&mut self, checkpoint: &mut dyn FnMut() -> io::Result<()>) -> io::Result<()> {
        self.verified = false;
        if let Some(destination) = &self.destination {
            destination.check_all(checkpoint)?;
        }
        // Verify the entire batch before any source deletion. Windows denies data
        // writes/replacement throughout verification, publication and removal.
        for entry in &mut self.entries {
            entry.source.verify(&entry.output.hash, checkpoint)?;
        }
        self.verified = true;
        Ok(())
    }

    pub fn remove_matching(
        &mut self,
        matches: &dyn Fn(&Path) -> bool,
        checkpoint: &mut dyn FnMut() -> io::Result<()>,
    ) -> io::Result<()> {
        self.remove_matching_observed(matches, checkpoint, &mut |_, _| Ok(()))
    }
    pub fn remove_matching_observed(
        &mut self,
        matches: &dyn Fn(&Path) -> bool,
        checkpoint: &mut dyn FnMut() -> io::Result<()>,
        removed: &mut dyn FnMut(&Path, crate::platform::FileIdentity) -> io::Result<()>,
    ) -> io::Result<()> {
        if !self.verified {
            return Err(io::Error::other(
                "Archive move source has not been verified",
            ));
        }
        let entries = std::mem::take(&mut self.entries);
        for entry in entries {
            if !matches(&entry.path) {
                self.entries.push(entry);
                continue;
            }
            let identity = crate::platform::platform().file_identity(entry.source.file())?;
            entry.source.remove_verified(checkpoint).map_err(|error| {
                io::Error::new(
                    error.kind(),
                    format!(
                        "Copied destination, but cannot remove source {:?}: {error}",
                        entry.path
                    ),
                )
            })?;
            removed(&entry.path, identity)?;
            // Keep the output alive until native source removal has completed.
            drop(entry.output);
        }
        Ok(())
    }
}

#[cfg(all(test, target_os = "windows"))]
mod tests {
    use super::*;
    use std::fs;

    fn record(moves: &mut ArchiveMoveSources, source: &Path, destination: &Path) {
        let held = CopiedSource::open(source).unwrap();
        let output = super::super::file_copy::copy_with_source(
            source,
            destination,
            false,
            Some(held.file()),
            &mut |_| Ok(()),
        )
        .unwrap();
        moves.record(
            source.to_path_buf(),
            destination.to_path_buf(),
            held,
            output,
        );
    }

    #[test]
    fn archive_move_sources_keep_both_files_until_publication_and_release_on_failure() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source");
        let output = root.path().join("output");
        fs::write(&source, b"chosen data").unwrap();
        let mut moves = ArchiveMoveSources::default();
        record(&mut moves, &source, &output);
        moves.verify_all(&mut || Ok(())).unwrap();
        for path in [&source, &output] {
            assert!(fs::OpenOptions::new().write(true).open(path).is_err());
            assert!(fs::remove_file(path).is_err());
        }
        // Simulate returning before source deletion when publication fails.
        drop(moves);
        for path in [&source, &output] {
            assert_eq!(fs::read(path).unwrap(), b"chosen data");
            assert!(fs::OpenOptions::new().write(true).open(path).is_ok());
        }
    }

    #[test]
    fn archive_move_sources_remove_the_original_after_parent_junction_retargeting() {
        use crate::platform::windows_reparse_tests::{junction, Retarget};
        let root = tempfile::tempdir().unwrap();
        let chosen = root.path().join("chosen");
        let outside = root.path().join("outside");
        let link = root.path().join("link");
        let template = root.path().join("template");
        fs::create_dir(&chosen).unwrap();
        fs::create_dir(&outside).unwrap();
        fs::write(chosen.join("file"), b"chosen data").unwrap();
        fs::write(outside.join("file"), b"foreign witness").unwrap();
        junction(&link, &chosen);
        junction(&template, &outside);
        let retarget = Retarget::new(&link, &template);
        let output = root.path().join("output");
        let mut moves = ArchiveMoveSources::default();
        record(&mut moves, &link.join("file"), &output);
        moves.verify_all(&mut || Ok(())).unwrap();
        moves
            .remove_matching(&|_| true, &mut || {
                retarget.apply();
                Ok(())
            })
            .unwrap();
        assert_eq!(fs::read(outside.join("file")).unwrap(), b"foreign witness");
        assert!(!chosen.join("file").exists());
        assert_eq!(fs::read(&output).unwrap(), b"chosen data");
        drop(moves);
        drop(retarget);
        fs::remove_dir(&link).unwrap();
        fs::remove_dir(&template).unwrap();
    }

    #[test]
    fn archive_move_sources_require_verification_of_the_entire_batch() {
        let root = tempfile::tempdir().unwrap();
        let first = root.path().join("first");
        let second = root.path().join("second");
        fs::write(&first, b"first").unwrap();
        fs::write(&second, b"second").unwrap();
        let mut moves = ArchiveMoveSources::default();
        record(&mut moves, &first, &root.path().join("first-output"));
        record(&mut moves, &second, &root.path().join("second-output"));
        moves.entries[1].output.hash = blake3::hash(b"different").to_hex().to_string();
        assert!(moves.remove_matching(&|_| true, &mut || Ok(())).is_err());
        assert!(moves.verify_all(&mut || Ok(())).is_err());
        assert!(moves
            .remove_matching(&|path| path == first, &mut || Ok(()))
            .is_err());
        drop(moves);
        assert_eq!(fs::read(first).unwrap(), b"first");
        assert_eq!(fs::read(second).unwrap(), b"second");
    }

    #[test]
    fn archive_move_sources_cancel_before_deletion_and_release_handles() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source");
        let output = root.path().join("output");
        fs::write(&source, b"data").unwrap();
        let mut moves = ArchiveMoveSources::default();
        record(&mut moves, &source, &output);
        moves.verify_all(&mut || Ok(())).unwrap();
        let error = moves
            .remove_matching(&|_| true, &mut || {
                Err(io::Error::new(io::ErrorKind::Interrupted, "cancelled"))
            })
            .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::Interrupted);
        drop(moves);
        assert_eq!(fs::read(&source).unwrap(), b"data");
        assert_eq!(fs::read(&output).unwrap(), b"data");
        fs::rename(&source, root.path().join("released")).unwrap();
    }

    #[test]
    fn archive_move_sources_separate_staging_removal_from_real_source_deletion() {
        let root = tempfile::tempdir().unwrap();
        let staged = root.path().join("staged");
        let real = root.path().join("real");
        fs::write(&staged, b"virtual data").unwrap();
        fs::write(&real, b"real data").unwrap();
        let mut moves = ArchiveMoveSources::default();
        record(&mut moves, &staged, &root.path().join("staged-output"));
        record(&mut moves, &real, &root.path().join("real-output"));
        moves.verify_all(&mut || Ok(())).unwrap();
        moves
            .remove_matching(&|path| path == staged, &mut || Ok(()))
            .unwrap();
        assert!(!staged.exists());
        assert!(real.is_file());
        assert!(fs::remove_file(&real).is_err());
        moves
            .remove_matching(&|path| path == real, &mut || Ok(()))
            .unwrap();
        assert!(!real.exists());
        assert_eq!(
            fs::read(root.path().join("real-output")).unwrap(),
            b"real data"
        );
    }
}
