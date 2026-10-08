//! Keep initial Move sources and published copies held until source deletion.
use super::file_copy::CopiedFile;
use crate::platform::platform;
use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};

pub(super) struct CopiedSource {
    file: File,
    path: PathBuf,
    _parents: super::path_parents::HeldParents,
}
impl CopiedSource {
    pub fn open(path: &Path) -> io::Result<Self> {
        let parents = super::path_parents::HeldParents::new(path)?;
        let file = parents.open_file(path)?;
        let metadata = file.metadata()?;
        if !metadata.is_file() || platform().file_attributes(&metadata, "").2 {
            return Err(io::Error::other("Move source is not an ordinary file"));
        }
        Ok(Self {
            file,
            path: path.into(),
            _parents: parents,
        })
    }
    pub fn file(&self) -> &File {
        &self.file
    }
    pub fn finish(
        mut self,
        output: CopiedFile,
        checkpoint: &mut dyn FnMut() -> io::Result<()>,
    ) -> io::Result<()> {
        // The copied file remains held throughout verification/deletion; it
        // cannot be replaced or written through an incompatible Windows handle.
        if super::history_move::hash_with(&mut self.file, checkpoint)? != output.hash {
            return Err(io::Error::other(format!(
                "Copied move source changed: {}",
                self.path.display()
            )));
        }
        checkpoint()?;
        platform().remove_open_file(&self.file, &self.path)?;
        drop(self);
        drop(output.file);
        Ok(())
    }
}

#[cfg(all(test, target_os = "windows"))]
mod tests {
    use super::*;
    use std::fs;
    use std::io::ErrorKind;

    #[test]
    fn initial_move_holds_source_and_destination_until_verified_deletion() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source");
        let destination = root.path().join("destination");
        fs::write(&source, b"source data").unwrap();
        fs::write(root.path().join("source:extra"), b"stream data").unwrap();
        let held = CopiedSource::open(&source).unwrap();
        let output = super::super::file_copy::copy_with_source(
            &source,
            &destination,
            false,
            Some(held.file()),
            &mut |_| Ok(()),
        )
        .unwrap();
        assert!(fs::File::options().write(true).open(&source).is_err());
        assert!(fs::remove_file(&source).is_err());
        assert!(fs::rename(&source, root.path().join("foreign")).is_err());
        assert!(fs::File::options().write(true).open(&destination).is_err());
        assert!(fs::remove_file(&destination).is_err());
        held.finish(output, &mut || Ok(())).unwrap();
        assert!(!source.exists());
        assert_eq!(fs::read(&destination).unwrap(), b"source data");
        assert_eq!(
            fs::read(root.path().join("destination:extra")).unwrap(),
            b"stream data"
        );
        assert!(fs::File::options().write(true).open(&destination).is_ok());
    }

    #[test]
    fn changed_source_hash_is_refused_without_deleting_either_file() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source");
        let destination = root.path().join("destination");
        fs::write(&source, b"copied contents").unwrap();
        super::super::file_copy::copy(&source, &destination, false, &mut |_| Ok(())).unwrap();
        let hash = blake3::hash(b"copied contents").to_hex().to_string();
        fs::write(&source, b"foreign modification").unwrap();
        let held = CopiedSource::open(&source).unwrap();
        let output = CopiedFile {
            file: platform().open_file_for_move(&destination).unwrap(),
            hash,
            _parents: super::super::path_parents::HeldParents::new(&destination).unwrap(),
        };
        let error = held.finish(output, &mut || Ok(())).unwrap_err();
        assert!(error.to_string().contains("source changed"));
        assert_eq!(fs::read(source).unwrap(), b"foreign modification");
        assert_eq!(fs::read(destination).unwrap(), b"copied contents");
    }

    #[test]
    fn cancellation_during_source_verification_keeps_both_files() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source");
        let destination = root.path().join("destination");
        fs::write(&source, vec![0x42; 256 * 1024]).unwrap();
        let held = CopiedSource::open(&source).unwrap();
        let output = super::super::file_copy::copy_with_source(
            &source,
            &destination,
            false,
            Some(held.file()),
            &mut |_| Ok(()),
        )
        .unwrap();
        let mut checks = 0;
        let error = held
            .finish(output, &mut || {
                checks += 1;
                if checks == 2 {
                    return Err(io::Error::new(
                        ErrorKind::Interrupted,
                        "injected verification cancellation",
                    ));
                }
                Ok(())
            })
            .unwrap_err();
        assert_eq!(error.to_string(), "injected verification cancellation");
        assert_eq!(fs::read(&source).unwrap(), fs::read(&destination).unwrap());
        assert!(fs::File::options().write(true).open(source).is_ok());
        assert!(fs::File::options().write(true).open(destination).is_ok());
    }

    #[test]
    fn refused_source_deletion_keeps_both_copies_and_releases_handles() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source");
        let destination = root.path().join("destination");
        fs::write(&source, b"readonly contents").unwrap();
        let mut permissions = fs::metadata(&source).unwrap().permissions();
        permissions.set_readonly(true);
        fs::set_permissions(&source, permissions).unwrap();
        let held = CopiedSource::open(&source).unwrap();
        let output = super::super::file_copy::copy_with_source(
            &source,
            &destination,
            false,
            Some(held.file()),
            &mut |_| Ok(()),
        )
        .unwrap();
        let error = held.finish(output, &mut || Ok(())).unwrap_err();
        for path in [&source, &destination] {
            let mut permissions = fs::metadata(path).unwrap().permissions();
            assert!(permissions.readonly());
            permissions.set_readonly(false);
            fs::set_permissions(path, permissions).unwrap();
            assert!(fs::File::options().write(true).open(path).is_ok());
        }
        assert_eq!(error.kind(), ErrorKind::PermissionDenied);
        assert_eq!(fs::read(source).unwrap(), b"readonly contents");
        assert_eq!(fs::read(destination).unwrap(), b"readonly contents");
    }
}
