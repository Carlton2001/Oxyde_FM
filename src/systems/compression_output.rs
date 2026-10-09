//! Retain private archive output through writing, publication and failure cleanup.
use super::{file_copy::Staging, path_parents::HeldParents};
use crate::platform::platform;
use std::{fs::File, io, path::Path};
pub(super) struct Output {
    pub file: File,
    staging: Staging,
    published: bool,
}
impl Output {
    pub fn new(parents: &HeldParents) -> io::Result<Self> {
        let staging = Staging::with_prefix(parents, ".oxyde-compress-")?;
        // Never delete an unowned output on a create failure or after closing
        // its handle. Failure cleanup below uses only the retained object.
        let file = platform().create_operation_output(&staging.directory, &staging.file)?;
        Ok(Self {
            file,
            staging,
            published: false,
        })
    }
    pub fn publish(&mut self, parents: &HeldParents, destination: &Path) -> io::Result<()> {
        parents.rename(&self.file, &self.staging.file, destination, false)?;
        self.published = true;
        Ok(())
    }
}
impl Drop for Output {
    fn drop(&mut self) {
        if !self.published {
            if let Err(error) = platform().remove_copied_file(&self.file, &self.staging.file) {
                log::warn!(
                    "Cannot remove retained archive output {}: {error}",
                    self.staging.file.display()
                );
            }
        }
    }
}

#[cfg(all(test, target_os = "windows"))]
mod tests {
    use super::*;
    use std::{fs, io::Write};
    #[test]
    fn archive_writer_cleanup_deletes_only_its_retained_output_and_preserves_unknown_children() {
        for foreign_child in [false, true] {
            let root = tempfile::tempdir().unwrap();
            let parents = HeldParents::new(&root.path().join("archive.zip")).unwrap();
            let mut output = Output::new(&parents).unwrap();
            output.file.write_all(b"partial archive").unwrap();
            assert!(
                fs::OpenOptions::new()
                    .write(true)
                    .open(&output.staging.file)
                    .is_err()
            );
            assert!(fs::remove_file(&output.staging.file).is_err());
            let directory = output.staging.file.parent().unwrap().to_path_buf();
            if foreign_child {
                fs::write(directory.join("witness"), b"unowned child").unwrap();
            }
            drop(output);
            assert!(!directory.join("output").exists());
            assert!(!directory.join(".pin").exists());
            if foreign_child {
                assert_eq!(
                    fs::read(directory.join("witness")).unwrap(),
                    b"unowned child"
                );
            } else {
                assert!(!directory.exists());
            }
        }
    }
    #[test]
    fn failed_archive_output_creation_preserves_preexisting_staging_file() {
        let root = tempfile::tempdir().unwrap();
        let parents = HeldParents::new(&root.path().join("archive.zip")).unwrap();
        let staging = Staging::with_prefix(&parents, ".oxyde-compress-").unwrap();
        let path = staging.file.clone();
        fs::write(&path, b"foreign output witness").unwrap();
        let error = platform()
            .create_operation_output(&staging.directory, &path)
            .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
        drop(staging);
        assert_eq!(fs::read(path).unwrap(), b"foreign output witness");
    }
    #[test]
    fn archive_publication_is_exclusive_and_keeps_owned_output_available_after_conflict() {
        let root = tempfile::tempdir().unwrap();
        let destination = root.path().join("archive.zip");
        let parents = HeldParents::new(&destination).unwrap();
        let mut output = Output::new(&parents).unwrap();
        output.file.write_all(b"finished archive").unwrap();
        fs::write(&destination, b"existing archive witness").unwrap();
        assert_eq!(
            output.publish(&parents, &destination).unwrap_err().kind(),
            io::ErrorKind::AlreadyExists
        );
        assert_eq!(fs::read(&destination).unwrap(), b"existing archive witness");
        let alternate = root.path().join("archive (1).zip");
        output.publish(&parents, &alternate).unwrap();
        drop(output);
        assert_eq!(fs::read(alternate).unwrap(), b"finished archive");
        assert_eq!(fs::read_dir(root.path()).unwrap().count(), 2);
    }
}
