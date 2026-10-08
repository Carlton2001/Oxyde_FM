//! Hold both the requested namespace and resolved directory parents on Windows.
use crate::platform::platform;
use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};

pub(crate) struct HeldParents {
    _handles: Vec<File>,
    directory: File,
    resolved: PathBuf,
}
impl HeldParents {
    pub fn new(file: &Path) -> io::Result<Self> {
        let absolute = std::path::absolute(file)?;
        let parent = absolute
            .parent()
            .ok_or_else(|| io::Error::other("Path has no parent"))?;
        let mut handles = Vec::new();
        // Existing junctions are usable. Hold their names and resolved parents.
        // This does not prevent attribute-only reparse mutation: these guards
        // provide parent rename protection, not complete path confinement.
        for ancestor in parent.ancestors().collect::<Vec<_>>().into_iter().rev() {
            handles.push(platform().hold_operation_directory(ancestor)?);
        }
        let resolved = std::fs::canonicalize(parent)?;
        for ancestor in resolved.ancestors().collect::<Vec<_>>().into_iter().rev() {
            handles.push(platform().hold_operation_directory(ancestor)?);
        }
        let directory = platform().hold_resolved_operation_directory(&resolved)?;
        if std::fs::canonicalize(parent)? != resolved {
            return Err(io::Error::other("File parent changed during the operation"));
        }
        Ok(Self {
            _handles: handles,
            directory,
            resolved,
        })
    }

    pub(crate) fn directory(&self) -> &File {
        &self.directory
    }

    pub(crate) fn resolved_path(&self, path: &Path) -> io::Result<PathBuf> {
        let name = path
            .file_name()
            .ok_or_else(|| io::Error::other("Operation path has no file name"))?;
        Ok(self.resolved.join(name))
    }

    pub(crate) fn open_file(&self, path: &Path) -> io::Result<File> {
        platform().open_operation_file(self.directory(), &self.resolved_path(path)?, true)
    }

    fn open_mutation(&self, path: &Path) -> io::Result<File> {
        // Moving/deleting does not need permission to read the file's contents.
        platform().open_operation_file(self.directory(), &self.resolved_path(path)?, false)
    }

    pub(crate) fn rename(
        &self,
        file: &File,
        source: &Path,
        destination: &Path,
        replace: bool,
    ) -> io::Result<()> {
        platform().rename_operation_file(
            file,
            source,
            self.directory(),
            &self.resolved_path(destination)?,
            replace,
        )
    }
}

/// Fast regular-file move. Parent binding and source opening precede the checkpoint.
pub(crate) fn move_file(
    source: &Path,
    destination: &Path,
    replace: bool,
    checkpoint: &mut dyn FnMut() -> io::Result<()>,
) -> io::Result<u64> {
    let source_parents = HeldParents::new(source)?;
    let destination_parents = HeldParents::new(destination)?;
    let file = source_parents.open_mutation(source)?;
    let metadata = file.metadata()?;
    if !metadata.is_file() {
        return Err(io::Error::other("Move source is not an ordinary file"));
    }
    if replace {
        let target = std::fs::symlink_metadata(destination_parents.resolved_path(destination)?)?;
        if !target.is_file() || platform().file_attributes(&target, "").2 {
            return Err(io::Error::other("Move destination is not an ordinary file"));
        }
    }
    checkpoint()?;
    destination_parents.rename(
        &file,
        &source_parents.resolved_path(source)?,
        destination,
        replace,
    )?;
    Ok(metadata.len())
}

pub(crate) fn remove(
    path: &Path,
    checkpoint: &mut dyn FnMut() -> io::Result<()>,
) -> io::Result<()> {
    let parents = HeldParents::new(path)?;
    let resolved = parents.resolved_path(path)?;
    if std::fs::symlink_metadata(&resolved)?.is_file() {
        let file = parents.open_mutation(path)?;
        checkpoint()?;
        platform().remove_open_file(&file, &resolved)
    } else {
        // Recursive traversal is still path-based; do not follow the original
        // junction namespace again after the checkpoint.
        checkpoint()?;
        if resolved.is_dir() {
            std::fs::remove_dir_all(resolved)
        } else {
            std::fs::remove_file(resolved)
        }
    }
}

#[cfg(all(test, target_os = "windows"))]
mod tests {
    use super::*;
    #[test]
    fn move_never_follows_a_reparse_point_installed_on_the_retained_parent() {
        use crate::platform::windows_reparse_tests::{junction, Retarget};
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source");
        let selected = root.path().join("selected");
        let outside = root.path().join("outside");
        std::fs::create_dir(&selected).unwrap();
        std::fs::create_dir(&outside).unwrap();
        std::fs::write(&source, b"selected").unwrap();
        std::fs::write(outside.join("witness"), b"outside witness").unwrap();
        let template = root.path().join("template");
        junction(&template, &outside);
        let retarget = Retarget::new(&selected, &template);
        let result = move_file(&source, &selected.join("output"), false, &mut || {
            retarget.apply();
            Ok(())
        });
        assert!(!outside.join("output").exists());
        assert_eq!(
            std::fs::read(outside.join("witness")).unwrap(),
            b"outside witness"
        );
        retarget.clear();
        match result {
            Ok(_) => assert_eq!(std::fs::read(selected.join("output")).unwrap(), b"selected"),
            Err(_) => assert_eq!(std::fs::read(source).unwrap(), b"selected"),
        }
        drop(retarget);
        std::fs::remove_dir(template).unwrap();
    }

    #[test]
    fn cancelled_move_preserves_source_and_destination() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source");
        let destination = root.path().join("destination");
        std::fs::write(&source, b"selected").unwrap();
        std::fs::write(&destination, b"old data").unwrap();
        let error = move_file(&source, &destination, true, &mut || {
            Err(io::Error::new(io::ErrorKind::Interrupted, "cancelled"))
        })
        .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::Interrupted);
        assert_eq!(std::fs::read(source).unwrap(), b"selected");
        assert_eq!(std::fs::read(destination).unwrap(), b"old data");
    }
    #[test]
    fn move_keeps_source_and_destination_bound_when_junctions_are_retargeted() {
        use crate::platform::windows_reparse_tests::{junction, Retarget};
        let root = tempfile::tempdir().unwrap();
        for name in [
            "source",
            "destination",
            "outside-source",
            "outside-destination",
        ] {
            std::fs::create_dir(root.path().join(name)).unwrap();
        }
        let source = root.path().join("source/file");
        let destination = root.path().join("destination/file");
        std::fs::write(&source, b"selected").unwrap();
        std::fs::write(root.path().join("outside-source/file"), b"source witness").unwrap();
        std::fs::write(
            root.path().join("outside-destination/file"),
            b"destination witness",
        )
        .unwrap();
        let identity = platform()
            .file_identity(&File::open(&source).unwrap())
            .unwrap();
        let source_link = root.path().join("source-link");
        let destination_link = root.path().join("destination-link");
        let source_template = root.path().join("source-template");
        let destination_template = root.path().join("destination-template");
        junction(&source_link, &root.path().join("source"));
        junction(&destination_link, &root.path().join("destination"));
        junction(&source_template, &root.path().join("outside-source"));
        junction(
            &destination_template,
            &root.path().join("outside-destination"),
        );
        let source_retarget = Retarget::new(&source_link, &source_template);
        let destination_retarget = Retarget::new(&destination_link, &destination_template);
        assert_eq!(
            move_file(
                &source_link.join("file"),
                &destination_link.join("file"),
                false,
                &mut || {
                    source_retarget.apply();
                    destination_retarget.apply();
                    Ok(())
                }
            )
            .unwrap(),
            8
        );
        assert!(!source.exists());
        assert_eq!(std::fs::read(&destination).unwrap(), b"selected");
        assert_eq!(
            platform()
                .file_identity(&File::open(destination).unwrap())
                .unwrap(),
            identity
        );
        assert_eq!(
            std::fs::read(root.path().join("outside-source/file")).unwrap(),
            b"source witness"
        );
        assert_eq!(
            std::fs::read(root.path().join("outside-destination/file")).unwrap(),
            b"destination witness"
        );
        drop((source_retarget, destination_retarget));
        for link in [
            source_link,
            destination_link,
            source_template,
            destination_template,
        ] {
            std::fs::remove_dir(link).unwrap();
        }
    }

    #[test]
    fn move_preserves_a_destination_that_appears_after_preparation() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source");
        let destination = root.path().join("destination");
        std::fs::write(&source, b"selected").unwrap();
        let error = move_file(&source, &destination, false, &mut || {
            std::fs::write(&destination, b"new witness")?;
            Ok(())
        })
        .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
        assert_eq!(std::fs::read(source).unwrap(), b"selected");
        assert_eq!(std::fs::read(destination).unwrap(), b"new witness");
    }

    #[test]
    fn move_replaces_only_when_explicitly_accepted_and_preserves_native_metadata() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source");
        let destination = root.path().join("destination");
        std::fs::write(&source, b"selected").unwrap();
        std::fs::write(&destination, b"old").unwrap();
        platform()
            .write_file_provenance(&source, b"[ZoneTransfer]\r\nZoneId=3\r\n")
            .unwrap();
        let original = File::open(&source).unwrap();
        let identity = platform().file_identity(&original).unwrap();
        let modified = original.metadata().unwrap().modified().unwrap();
        drop(original);
        move_file(&source, &destination, true, &mut || Ok(())).unwrap();
        assert!(!source.exists());
        let moved = File::open(&destination).unwrap();
        assert_eq!(platform().file_identity(&moved).unwrap(), identity);
        assert_eq!(moved.metadata().unwrap().modified().unwrap(), modified);
        assert_eq!(
            platform()
                .read_file_provenance(&destination)
                .unwrap()
                .unwrap(),
            b"[ZoneTransfer]\r\nZoneId=3\r\n"
        );
    }
    #[test]
    fn deletion_cannot_be_redirected_by_attribute_only_junction_retargeting() {
        use crate::platform::windows_reparse_tests::{junction, Retarget};
        let root = tempfile::tempdir().unwrap();
        let selected = root.path().join("selected");
        let outside = root.path().join("outside");
        std::fs::create_dir(&selected).unwrap();
        std::fs::create_dir(&outside).unwrap();
        std::fs::write(selected.join("file"), b"selected").unwrap();
        std::fs::write(outside.join("file"), b"outside witness").unwrap();
        let link = root.path().join("link");
        let template = root.path().join("template");
        junction(&link, &selected);
        junction(&template, &outside);
        let retarget = Retarget::new(&link, &template);
        remove(&link.join("file"), &mut || {
            retarget.apply();
            Ok(())
        })
        .unwrap();
        assert_eq!(
            std::fs::read(outside.join("file")).unwrap(),
            b"outside witness"
        );
        assert!(!selected.join("file").exists());
        drop(retarget);
        std::fs::remove_dir(link).unwrap();
        std::fs::remove_dir(template).unwrap();
    }
    #[test]
    fn deletion_keeps_parent_fixed_until_the_selected_file_is_removed() {
        let root = tempfile::tempdir().unwrap();
        let parent = root.path().join("parent");
        std::fs::create_dir(&parent).unwrap();
        let selected = parent.join("selected");
        std::fs::write(&selected, b"selected").unwrap();
        let witness = root.path().join("witness");
        std::fs::write(&witness, b"outside").unwrap();
        remove(&selected, &mut || {
            assert!(std::fs::rename(&parent, root.path().join("replaced")).is_err());
            Ok(())
        })
        .unwrap();
        assert!(!selected.exists());
        assert_eq!(std::fs::read(witness).unwrap(), b"outside");
        std::fs::rename(&parent, root.path().join("replaced")).unwrap();
    }

    #[test]
    fn existing_junctions_remain_usable_and_both_namespaces_are_held() {
        use std::os::windows::process::CommandExt;
        let root = tempfile::tempdir().unwrap();
        let target = root.path().join("target");
        std::fs::create_dir(&target).unwrap();
        let link = root.path().join("junction");
        let output = std::process::Command::new(platform().system_executable(crate::platform::SystemExecutable::PowerShell).unwrap())
            .args(["-NoProfile", "-NonInteractive", "-Command", "New-Item -ItemType Junction -Path $env:OXYDE_TEST_LINK -Value $env:OXYDE_TEST_TARGET -ErrorAction Stop | Out-Null"])
            .env("OXYDE_TEST_LINK", &link).env("OXYDE_TEST_TARGET", &target)
            .creation_flags(0x08000000).output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let held = HeldParents::new(&link.join("file")).unwrap();
        use std::os::windows::fs::OpenOptionsExt;
        let writer = std::fs::OpenOptions::new()
            .write(true)
            .share_mode(7)
            .custom_flags(
                windows::Win32::Storage::FileSystem::FILE_FLAG_BACKUP_SEMANTICS.0
                    | windows::Win32::Storage::FileSystem::FILE_FLAG_OPEN_REPARSE_POINT.0,
            )
            .open(&link);
        assert!(
            writer.is_err(),
            "held junction must reject a retargeting writer"
        );

        assert!(std::fs::rename(&link, root.path().join("new-link")).is_err());
        assert!(std::fs::rename(&target, root.path().join("new-target")).is_err());
        std::fs::write(link.join("file"), b"allowed").unwrap();
        drop(held);
        std::fs::remove_dir(link).unwrap();
    }
}
