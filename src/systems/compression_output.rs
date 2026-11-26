//! Create an archive under its final name and retain it through writing; an archive
//! not kept is emptied and removed by its creation handle.
use super::{copy_output, path_parents::HeldParents};
use crate::platform::platform;
use std::{fs::File, io, path::Path};
pub(super) struct Output {
    pub file: File,
    // Dropped after the writer above: its cleanup reopens the file by name.
    written: copy_output::Output,
}
impl Output {
    /// Creates `target`, a resolved path in `parents`, exclusively: an existing name,
    /// a link included, is refused with `AlreadyExists`.
    pub fn create(parents: &HeldParents, target: &Path) -> io::Result<Self> {
        let written = copy_output::Output::new(parents, target, false)?;
        // Never delete an unowned output on a create failure: cleanup uses only
        // the object this call created.
        let file = platform().create_operation_output(parents.directory(), written.path())?;
        written.capture(&file)?;
        Ok(Self { file, written })
    }
    pub fn keep(&self, parents: &HeldParents) -> io::Result<()> {
        self.written.keep(parents, &self.file, false)
    }
}

#[cfg(all(test, target_os = "windows"))]
mod tests {
    use super::*;
    use std::{fs, io::Write};
    #[test]
    fn an_archive_not_kept_is_removed_and_a_foreign_file_stays() {
        let root = tempfile::tempdir().unwrap();
        let target = root.path().join("archive.zip");
        let parents = HeldParents::new(&target).unwrap();
        let mut output = Output::create(&parents, &target).unwrap();
        output.file.write_all(b"partial archive").unwrap();
        assert!(fs::OpenOptions::new().write(true).open(&target).is_err());
        assert!(fs::remove_file(&target).is_err());
        fs::write(root.path().join("witness"), b"unowned").unwrap();
        drop(output);
        assert!(!target.exists());
        assert_eq!(fs::read(root.path().join("witness")).unwrap(), b"unowned");
        assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
    }
    #[test]
    fn creation_is_exclusive_and_keeps_an_existing_archive() {
        let root = tempfile::tempdir().unwrap();
        let target = root.path().join("archive.zip");
        fs::write(&target, b"existing archive witness").unwrap();
        let parents = HeldParents::new(&target).unwrap();
        let error = Output::create(&parents, &target).err().unwrap();
        assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
        assert_eq!(fs::read(&target).unwrap(), b"existing archive witness");
        let alternate = root.path().join("archive (1).zip");
        let mut output = Output::create(&parents, &alternate).unwrap();
        output.file.write_all(b"finished archive").unwrap();
        output.keep(&parents).unwrap();
        drop(output);
        assert_eq!(fs::read(alternate).unwrap(), b"finished archive");
        assert_eq!(fs::read_dir(root.path()).unwrap().count(), 2);
    }
}
