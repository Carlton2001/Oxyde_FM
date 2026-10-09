//! Verified history moves; copy only when the filesystem reports a volume boundary.
use crate::platform::platform;
use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::Path;

trait MoveIo {
    fn rename(
        &self,
        file: &File,
        source: &Path,
        destination: &Path,
        parents: &super::path_parents::HeldParents,
    ) -> io::Result<()>;
    fn copy(
        &self,
        file: &File,
        source: &Path,
        destination: &Path,
        parent: &File,
    ) -> io::Result<u64>;
    fn remove(&self, file: &File, path: &Path) -> io::Result<()>;
}
struct NativeIo;
impl MoveIo for NativeIo {
    fn rename(
        &self,
        file: &File,
        source: &Path,
        destination: &Path,
        parents: &super::path_parents::HeldParents,
    ) -> io::Result<()> {
        parents.rename(file, source, destination, false)
    }
    fn copy(
        &self,
        file: &File,
        source: &Path,
        destination: &Path,
        parent: &File,
    ) -> io::Result<u64> {
        platform()
            .copy_file_with_progress(source, destination, Some(file), parent, &mut |_| Ok(()))?;
        Ok(file.metadata()?.len())
    }
    fn remove(&self, file: &File, path: &Path) -> io::Result<()> {
        platform().remove_open_file(file, path)
    }
}

pub(super) fn hash(file: &mut File) -> io::Result<String> {
    hash_with(file, &mut || Ok(()))
}
pub(super) fn hash_with(
    file: &mut File,
    checkpoint: &mut dyn FnMut() -> io::Result<()>,
) -> io::Result<String> {
    file.seek(SeekFrom::Start(0))?;
    let mut hasher = blake3::Hasher::new();
    let mut buffer = [0u8; 128 * 1024];
    loop {
        checkpoint()?;
        let size = file.read(&mut buffer)?;
        if size == 0 {
            break;
        }
        hasher.update(&buffer[..size]);
    }
    Ok(hasher.finalize().to_hex().to_string())
}

pub(super) fn hash_file(path: &Path) -> io::Result<String> {
    hash(&mut File::open(path)?)
}

#[cfg(test)]
pub(crate) fn move_file(
    source: &Path,
    destination: &Path,
    expected: Option<&str>,
) -> io::Result<()> {
    move_with(&NativeIo, source, destination, expected)
}

#[cfg(test)]
fn move_with(
    ops: &impl MoveIo,
    source: &Path,
    destination: &Path,
    expected: Option<&str>,
) -> io::Result<()> {
    let source_parents = super::path_parents::HeldParents::new(source)?;
    let destination_parents = super::path_parents::HeldParents::new(destination)?;
    move_bound(
        ops,
        source,
        destination,
        expected,
        None,
        &source_parents,
        &destination_parents,
    )
    .map(|_| ())
}

pub(super) fn move_file_with_bound_parents(
    source: &Path,
    destination: &Path,
    expected: Option<&str>,
    source_parents: &super::path_parents::HeldParents,
    destination_parents: &super::path_parents::HeldParents,
) -> io::Result<String> {
    move_bound(
        &NativeIo,
        source,
        destination,
        expected,
        None,
        source_parents,
        destination_parents,
    )
}

pub(super) fn move_file_with_parents(
    source: &Path,
    destination: &Path,
    expected: &str,
    source_parents: &super::path_parents::HeldParents,
    destination_parents: &super::path_parents::HeldParents,
) -> io::Result<()> {
    move_bound(
        &NativeIo,
        source,
        destination,
        Some(expected),
        None,
        source_parents,
        destination_parents,
    )
    .map(|_| ())
}

pub(super) fn move_file_with_identity(
    source: &Path,
    destination: &Path,
    expected: &str,
    identity: crate::models::transaction::HistoryFileIdentity,
    source_parents: &super::path_parents::HeldParents,
    destination_parents: &super::path_parents::HeldParents,
) -> io::Result<()> {
    move_bound(
        &NativeIo,
        source,
        destination,
        Some(expected),
        Some(identity),
        source_parents,
        destination_parents,
    )
    .map(|_| ())
}

fn move_bound(
    ops: &impl MoveIo,
    source: &Path,
    destination: &Path,
    expected: Option<&str>,
    identity: Option<crate::models::transaction::HistoryFileIdentity>,
    source_parents: &super::path_parents::HeldParents,
    destination_parents: &super::path_parents::HeldParents,
) -> io::Result<String> {
    let source = source_parents.resolved_path(source)?;
    let destination = destination_parents.resolved_path(destination)?;
    let mut original = source_parents.open_file(&source)?;
    if let Some(identity) = identity {
        if !identity.matches(&original)? {
            return Err(super::file_copy::identity_changed(&source));
        }
    }
    let metadata = original.metadata()?;
    let name = source
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    let (_, _, reparse) = platform().file_attributes(&metadata, name);
    if !metadata.is_file() || reparse {
        return Err(io::Error::other(
            "History moves require an ordinary file, not a reparse point",
        ));
    }
    let original_hash = hash(&mut original)?;
    if expected.is_some_and(|expected| expected != original_hash) {
        return Err(io::Error::other(format!(
            "Cannot move a file changed since the operation: {}",
            source.display()
        )));
    }
    match ops.rename(&original, &source, &destination, destination_parents) {
        Ok(()) => return Ok(original_hash),
        Err(error) if error.kind() == io::ErrorKind::CrossesDevices && identity.is_none() => {}
        Err(error) => return Err(error),
    }

    let staging = super::file_copy::Staging::with_prefix(destination_parents, ".oxyde-history-")?;
    let copied = ops.copy(&original, &source, &staging.file, &staging.directory)?;
    if copied != metadata.len() {
        return Err(io::Error::other("Incomplete history move copy"));
    }
    let mut output = platform().open_copied_file(&staging.directory, &staging.file)?;
    if hash(&mut output)? != original_hash || hash(&mut original)? != original_hash {
        return Err(io::Error::other(
            "History move copy failed content verification",
        ));
    }
    ops.rename(&output, &staging.file, &destination, destination_parents)?;
    if let Err(error) = ops.remove(&original, &source) {
        // The opened destination still belongs to this operation. On Windows
        // its handle prevents an external writer or path replacement here.
        let cleanup = platform().remove_copied_file(&output, &destination);
        return match cleanup {
            Ok(()) => Err(error),
            Err(cleanup) => Err(io::Error::other(format!(
                "{error}; cannot remove published copy {}: {cleanup}",
                destination.display()
            ))),
        };
    }
    Ok(original_hash)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::fs;
    use std::path::PathBuf;

    #[derive(Clone, Copy)]
    enum Fault {
        None,
        Copy,
        Corrupt,
        Conflict,
        Remove,
    }
    struct ForcedCopy {
        source: PathBuf,
        destination: PathBuf,
        attempts: Cell<usize>,
        fault: Fault,
    }
    impl MoveIo for ForcedCopy {
        fn rename(
            &self,
            file: &File,
            source: &Path,
            destination: &Path,
            parents: &super::super::path_parents::HeldParents,
        ) -> io::Result<()> {
            self.attempts.set(self.attempts.get() + 1);
            if self.attempts.get() == 1 {
                return Err(io::Error::new(
                    io::ErrorKind::CrossesDevices,
                    "injected volume boundary",
                ));
            }
            if matches!(self.fault, Fault::Conflict) {
                fs::write(&self.destination, b"external file")?;
            }
            NativeIo.rename(file, source, destination, parents)
        }
        fn copy(
            &self,
            file: &File,
            source: &Path,
            destination: &Path,
            parent: &File,
        ) -> io::Result<u64> {
            #[cfg(target_os = "windows")]
            assert!(
                platform()
                    .file_attributes(
                        &fs::metadata(destination.parent().unwrap()).unwrap(),
                        "staging",
                    )
                    .0
            );
            if matches!(self.fault, Fault::Copy) {
                fs::write(destination, b"partial copy")?;
                return Err(io::Error::other("injected write failure"));
            }
            let count = NativeIo.copy(file, source, destination, parent)?;
            if matches!(self.fault, Fault::Corrupt) {
                let content = vec![b'x'; count as usize];
                fs::write(destination, content)?;
            }
            Ok(count)
        }
        fn remove(&self, file: &File, path: &Path) -> io::Result<()> {
            if matches!(self.fault, Fault::Remove) {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "injected delete failure",
                ));
            }
            NativeIo.remove(file, path)
        }
    }
    fn fixture(root: &Path, fault: Fault) -> ForcedCopy {
        let source = root.join("source.txt");
        fs::write(&source, b"original bytes").unwrap();
        ForcedCopy {
            source,
            destination: root.join("destination.txt"),
            attempts: Cell::new(0),
            fault,
        }
    }
    fn assert_no_staging(root: &Path) {
        assert!(fs::read_dir(root).unwrap().all(|entry| !entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".oxyde-history-")));
    }

    #[test]
    fn identity_bound_copy_undo_refuses_a_cross_volume_fallback() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source");
        let destination = root.path().join("staged");
        fs::write(&source, b"selected data").unwrap();
        let identity = crate::models::transaction::HistoryFileIdentity::from_file(
            &File::open(&source).unwrap(),
        )
        .unwrap();
        let source_parents = super::super::path_parents::HeldParents::new(&source).unwrap();
        let destination_parents =
            super::super::path_parents::HeldParents::new(&destination).unwrap();
        let ops = ForcedCopy {
            source: source.clone(),
            destination: destination.clone(),
            attempts: Cell::new(0),
            fault: Fault::None,
        };
        let expected = blake3::hash(b"selected data").to_hex().to_string();
        let error = move_bound(
            &ops,
            &source,
            &destination,
            Some(&expected),
            Some(identity),
            &source_parents,
            &destination_parents,
        )
        .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::CrossesDevices);
        assert_eq!(ops.attempts.get(), 1);
        assert_eq!(fs::read(&source).unwrap(), b"selected data");
        assert!(identity.matches(&File::open(&source).unwrap()).unwrap());
        assert!(!destination.exists());
        assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn history_move_keeps_the_initial_destination_after_junction_retargeting() {
        use crate::platform::windows_reparse_tests::{junction, Retarget};
        struct RetargetingIo {
            retarget: Retarget,
        }
        impl MoveIo for RetargetingIo {
            fn rename(
                &self,
                file: &File,
                source: &Path,
                destination: &Path,
                parents: &super::super::path_parents::HeldParents,
            ) -> io::Result<()> {
                self.retarget.apply();
                NativeIo.rename(file, source, destination, parents)
            }
            fn copy(
                &self,
                file: &File,
                source: &Path,
                destination: &Path,
                parent: &File,
            ) -> io::Result<u64> {
                NativeIo.copy(file, source, destination, parent)
            }
            fn remove(&self, file: &File, path: &Path) -> io::Result<()> {
                NativeIo.remove(file, path)
            }
        }
        let root = tempfile::tempdir().unwrap();
        let selected = root.path().join("selected");
        let outside = root.path().join("outside");
        let source = root.path().join("source");
        fs::create_dir(&selected).unwrap();
        fs::create_dir(&outside).unwrap();
        fs::write(&source, b"selected data").unwrap();
        fs::write(outside.join("witness"), b"outside witness").unwrap();
        let link = root.path().join("link");
        let template = root.path().join("template");
        junction(&link, &selected);
        junction(&template, &outside);
        let ops = RetargetingIo {
            retarget: Retarget::new(&link, &template),
        };
        move_with(&ops, &source, &link.join("output"), None).unwrap();
        let stayed = selected.join("output").is_file();
        assert_eq!(
            fs::read(outside.join("witness")).unwrap(),
            b"outside witness"
        );
        drop(ops);
        fs::remove_dir(link).unwrap();
        fs::remove_dir(template).unwrap();
        assert!(
            stayed,
            "history move followed the retargeted destination junction"
        );
        assert!(!outside.join("output").exists());
        assert!(!source.exists());
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn history_copy_keeps_initial_source_and_destination_after_retargeting() {
        use crate::platform::windows_reparse_tests::{junction, Retarget};
        struct RetargetingCopy {
            source: Retarget,
            destination: Retarget,
            calls: Cell<usize>,
        }
        impl MoveIo for RetargetingCopy {
            fn rename(
                &self,
                file: &File,
                source: &Path,
                destination: &Path,
                parents: &super::super::path_parents::HeldParents,
            ) -> io::Result<()> {
                self.calls.set(self.calls.get() + 1);
                if self.calls.get() == 1 {
                    self.source.apply();
                    self.destination.apply();
                    return Err(io::Error::new(
                        io::ErrorKind::CrossesDevices,
                        "test volume boundary",
                    ));
                }
                NativeIo.rename(file, source, destination, parents)
            }
            fn copy(
                &self,
                file: &File,
                source: &Path,
                destination: &Path,
                parent: &File,
            ) -> io::Result<u64> {
                NativeIo.copy(file, source, destination, parent)
            }
            fn remove(&self, file: &File, path: &Path) -> io::Result<()> {
                NativeIo.remove(file, path)
            }
        }
        let root = tempfile::tempdir().unwrap();
        let source_parent = root.path().join("source-parent");
        let selected = root.path().join("selected");
        let outside = root.path().join("outside");
        for directory in [&source_parent, &selected, &outside] {
            fs::create_dir(directory).unwrap();
        }
        fs::write(source_parent.join("input"), b"selected data").unwrap();
        fs::write(outside.join("input"), b"foreign source").unwrap();
        fs::write(outside.join("output"), b"foreign destination").unwrap();
        let source_link = root.path().join("source-link");
        let destination_link = root.path().join("destination-link");
        let template = root.path().join("template");
        junction(&source_link, &source_parent);
        junction(&destination_link, &selected);
        junction(&template, &outside);
        let ops = RetargetingCopy {
            source: Retarget::new(&source_link, &template),
            destination: Retarget::new(&destination_link, &template),
            calls: Cell::new(0),
        };
        move_with(
            &ops,
            &source_link.join("input"),
            &destination_link.join("output"),
            None,
        )
        .unwrap();
        assert_eq!(fs::read(selected.join("output")).unwrap(), b"selected data");
        assert!(!source_parent.join("input").exists());
        assert_eq!(fs::read(outside.join("input")).unwrap(), b"foreign source");
        assert_eq!(
            fs::read(outside.join("output")).unwrap(),
            b"foreign destination"
        );
        assert_eq!(fs::read_dir(&outside).unwrap().count(), 2);
        assert_no_staging(&selected);
        drop(ops);
        for link in [source_link, destination_link, template] {
            fs::remove_dir(link).unwrap();
        }
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn history_copy_refuses_a_new_destination_reparse_before_staging() {
        use crate::platform::windows_reparse_tests::{junction, Retarget};
        struct RedirectBeforeCopy {
            retarget: Retarget,
            copied: Cell<bool>,
        }
        impl MoveIo for RedirectBeforeCopy {
            fn rename(
                &self,
                _file: &File,
                _source: &Path,
                _destination: &Path,
                _parents: &super::super::path_parents::HeldParents,
            ) -> io::Result<()> {
                self.retarget.apply();
                Err(io::Error::new(
                    io::ErrorKind::CrossesDevices,
                    "test volume boundary",
                ))
            }
            fn copy(
                &self,
                _file: &File,
                _source: &Path,
                _destination: &Path,
                _parent: &File,
            ) -> io::Result<u64> {
                self.copied.set(true);
                Err(io::Error::other("test stop before data"))
            }
            fn remove(&self, file: &File, path: &Path) -> io::Result<()> {
                NativeIo.remove(file, path)
            }
        }
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source");
        let selected = root.path().join("selected");
        let outside = root.path().join("outside");
        fs::write(&source, b"selected data").unwrap();
        fs::create_dir(&selected).unwrap();
        fs::create_dir(&outside).unwrap();
        fs::write(outside.join("witness"), b"unchanged").unwrap();
        let template = root.path().join("template");
        junction(&template, &outside);
        let ops = RedirectBeforeCopy {
            retarget: Retarget::new(&selected, &template),
            copied: Cell::new(false),
        };
        let result = move_with(&ops, &source, &selected.join("output"), None);
        ops.retarget.clear();
        assert!(result.is_err());
        assert!(!ops.copied.get());
        assert_eq!(fs::read(&source).unwrap(), b"selected data");
        assert_eq!(fs::read(outside.join("witness")).unwrap(), b"unchanged");
        assert_eq!(fs::read_dir(&outside).unwrap().count(), 1);
        assert_eq!(fs::read_dir(&selected).unwrap().count(), 0);
        drop(ops);
        fs::remove_dir(template).unwrap();
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn history_readonly_cleanup_uses_the_published_identity_after_retargeting() {
        use crate::platform::windows_reparse_tests::{junction, Retarget};
        struct RedirectOnRemove {
            retarget: Retarget,
            calls: Cell<usize>,
        }
        impl MoveIo for RedirectOnRemove {
            fn rename(
                &self,
                file: &File,
                source: &Path,
                destination: &Path,
                parents: &super::super::path_parents::HeldParents,
            ) -> io::Result<()> {
                self.calls.set(self.calls.get() + 1);
                if self.calls.get() == 1 {
                    return Err(io::Error::new(
                        io::ErrorKind::CrossesDevices,
                        "test volume boundary",
                    ));
                }
                NativeIo.rename(file, source, destination, parents)
            }
            fn copy(
                &self,
                file: &File,
                source: &Path,
                destination: &Path,
                parent: &File,
            ) -> io::Result<u64> {
                NativeIo.copy(file, source, destination, parent)
            }
            fn remove(&self, _file: &File, _path: &Path) -> io::Result<()> {
                self.retarget.apply();
                Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "test source deletion refused",
                ))
            }
        }
        let root = tempfile::tempdir().unwrap();
        let selected = root.path().join("selected");
        let outside = root.path().join("outside");
        fs::create_dir(&selected).unwrap();
        fs::create_dir(&outside).unwrap();
        let source = root.path().join("source");
        let foreign = outside.join("output");
        fs::write(&source, b"selected data").unwrap();
        fs::write(&foreign, b"foreign data").unwrap();
        for path in [&source, &foreign] {
            let mut permissions = fs::metadata(path).unwrap().permissions();
            permissions.set_readonly(true);
            fs::set_permissions(path, permissions).unwrap();
        }
        let link = root.path().join("link");
        let template = root.path().join("template");
        junction(&link, &selected);
        junction(&template, &outside);
        let ops = RedirectOnRemove {
            retarget: Retarget::new(&link, &template),
            calls: Cell::new(0),
        };
        let error = move_with(&ops, &source, &link.join("output"), None).unwrap_err();
        let preserved_attributes = fs::metadata(&source).unwrap().permissions().readonly()
            && fs::metadata(&foreign).unwrap().permissions().readonly();
        for path in [&source, &foreign] {
            let mut permissions = fs::metadata(path).unwrap().permissions();
            permissions.set_readonly(false);
            fs::set_permissions(path, permissions).unwrap();
        }
        assert_eq!(error.to_string(), "test source deletion refused");
        assert!(preserved_attributes);
        assert_eq!(fs::read(&source).unwrap(), b"selected data");
        assert_eq!(fs::read(&foreign).unwrap(), b"foreign data");
        assert!(!selected.join("output").exists());
        assert_no_staging(&selected);
        drop(ops);
        fs::remove_dir(link).unwrap();
        fs::remove_dir(template).unwrap();
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn forced_history_copy_preserves_streams_hidden_attribute_and_modified_time() {
        let root = tempfile::tempdir().unwrap();
        let ops = fixture(root.path(), Fault::None);
        fs::write(
            ops.source.with_file_name("source.txt:extra"),
            b"alternate stream",
        )
        .unwrap();
        fs::write(
            ops.source.with_file_name("source.txt:Zone.Identifier"),
            b"[ZoneTransfer]\r\nZoneId=3\r\n",
        )
        .unwrap();
        platform().set_hidden_attribute(&ops.source, true).unwrap();
        let modified = std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_700_000_000);
        File::options()
            .write(true)
            .open(&ops.source)
            .unwrap()
            .set_times(std::fs::FileTimes::new().set_modified(modified))
            .unwrap();
        move_with(&ops, &ops.source, &ops.destination, None).unwrap();
        assert!(!ops.source.exists());
        assert_eq!(
            fs::read(ops.destination.with_file_name("destination.txt:extra")).unwrap(),
            b"alternate stream"
        );
        assert_eq!(
            fs::read(
                ops.destination
                    .with_file_name("destination.txt:Zone.Identifier")
            )
            .unwrap(),
            b"[ZoneTransfer]\r\nZoneId=3\r\n"
        );
        let metadata = fs::metadata(&ops.destination).unwrap();
        assert_eq!(metadata.modified().unwrap(), modified);
        assert!(platform().file_attributes(&metadata, "").0);
        assert_no_staging(root.path());
    }

    #[test]
    fn copy_fallback_publishes_verified_content_then_removes_source() {
        let root = tempfile::tempdir().unwrap();
        let ops = fixture(root.path(), Fault::None);
        let expected = blake3::hash(b"original bytes").to_hex().to_string();
        move_with(&ops, &ops.source, &ops.destination, Some(&expected)).unwrap();
        assert!(!ops.source.exists());
        assert_eq!(fs::read(&ops.destination).unwrap(), b"original bytes");
        assert_no_staging(root.path());
    }
    #[test]
    fn failed_or_corrupted_copy_keeps_source_and_removes_staging() {
        for fault in [Fault::Copy, Fault::Corrupt] {
            let root = tempfile::tempdir().unwrap();
            let ops = fixture(root.path(), fault);
            let error = move_with(&ops, &ops.source, &ops.destination, None).unwrap_err();
            assert!(error.to_string().contains(match fault {
                Fault::Copy => "injected write failure",
                Fault::Corrupt => "content verification",
                _ => unreachable!(),
            }));
            assert_eq!(fs::read(&ops.source).unwrap(), b"original bytes");
            assert!(!ops.destination.exists());
            assert_no_staging(root.path());
        }
    }
    #[test]
    fn publication_conflict_preserves_external_file_and_source() {
        let root = tempfile::tempdir().unwrap();
        let ops = fixture(root.path(), Fault::Conflict);
        let error = move_with(&ops, &ops.source, &ops.destination, None).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
        assert_eq!(fs::read(&ops.source).unwrap(), b"original bytes");
        assert_eq!(fs::read(&ops.destination).unwrap(), b"external file");
        assert_no_staging(root.path());
    }
    #[test]
    fn refused_source_deletion_removes_own_published_copy() {
        let root = tempfile::tempdir().unwrap();
        let ops = fixture(root.path(), Fault::Remove);
        let error = move_with(&ops, &ops.source, &ops.destination, None).unwrap_err();
        assert_eq!(error.to_string(), "injected delete failure");
        assert_eq!(fs::read(&ops.source).unwrap(), b"original bytes");
        assert!(!ops.destination.exists());
        assert_no_staging(root.path());
    }
    #[test]
    fn changed_source_is_rejected_before_rename_or_copy() {
        let root = tempfile::tempdir().unwrap();
        let ops = fixture(root.path(), Fault::None);
        assert!(move_with(&ops, &ops.source, &ops.destination, Some("old hash")).is_err());
        assert_eq!(ops.attempts.get(), 0);
        assert_eq!(fs::read(&ops.source).unwrap(), b"original bytes");
        assert!(!ops.destination.exists());
    }

    #[test]
    fn readonly_copy_cleanup_preserves_original_attributes() {
        let root = tempfile::tempdir().unwrap();
        let ops = fixture(root.path(), Fault::Remove);
        let mut permissions = fs::metadata(&ops.source).unwrap().permissions();
        permissions.set_readonly(true);
        fs::set_permissions(&ops.source, permissions).unwrap();
        let result = move_with(&ops, &ops.source, &ops.destination, None);
        assert_eq!(result.unwrap_err().to_string(), "injected delete failure");
        assert_eq!(fs::read(&ops.source).unwrap(), b"original bytes");
        assert!(fs::metadata(&ops.source).unwrap().permissions().readonly());
        assert!(!ops.destination.exists());
        assert_no_staging(root.path());
        let mut permissions = fs::metadata(&ops.source).unwrap().permissions();
        permissions.set_readonly(false);
        fs::set_permissions(&ops.source, permissions).unwrap();
    }
    #[test]
    fn native_same_volume_move_preserves_identity_and_refuses_overwrite() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("Données 日本語 🦀.txt");
        let destination = root.path().join("moved.txt");
        let alias = root.path().join("alias.txt");
        fs::write(&source, b"original").unwrap();
        fs::hard_link(&source, &alias).unwrap();
        fs::write(&destination, b"external").unwrap();
        assert!(move_file(&source, &destination, None).is_err());
        assert_eq!(fs::read(&destination).unwrap(), b"external");
        fs::remove_file(&destination).unwrap();
        move_file(&source, &destination, None).unwrap();
        fs::write(&alias, b"same identity").unwrap();
        assert_eq!(fs::read(&destination).unwrap(), b"same identity");
        assert!(!source.exists());
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn native_move_handle_blocks_writers_and_path_replacement() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source.txt");
        fs::write(&source, b"original").unwrap();
        let file = platform().open_file_for_move(&source).unwrap();
        assert!(File::options().write(true).open(&source).is_err());
        assert!(fs::remove_file(&source).is_err());
        assert!(fs::rename(&source, root.path().join("replacement.txt")).is_err());
        drop(file);
        assert!(File::options().write(true).open(&source).is_ok());
    }

    #[cfg(target_os = "windows")]
    #[test]
    #[ignore = "requires OXYDE_TEST_SECOND_VOLUME; copies only generated fixtures between two volumes"]
    fn native_cross_volume_transfer_preserves_streams_and_modified_time() {
        let second_root = PathBuf::from(std::env::var_os("OXYDE_TEST_SECOND_VOLUME").unwrap());
        assert!(second_root.is_absolute() && second_root.is_dir());
        let first = tempfile::tempdir().unwrap();
        let second = tempfile::Builder::new()
            .prefix("oxyde-a19-")
            .tempdir_in(second_root)
            .unwrap();
        let source = first.path().join("Données 日本語 🦀.txt");
        let destination = second.path().join(source.file_name().unwrap());
        fs::write(&source, b"main stream").unwrap();
        let stream_path = |path: &Path| {
            let mut name = path.as_os_str().to_os_string();
            name.push(":oxyde_fixture");
            PathBuf::from(name)
        };
        fs::write(stream_path(&source), b"alternate stream").unwrap();
        let modified = std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_700_000_000);
        File::options()
            .write(true)
            .open(&source)
            .unwrap()
            .set_times(fs::FileTimes::new().set_modified(modified))
            .unwrap();
        assert_eq!(
            platform()
                .rename_open_file_exclusive(
                    &platform().open_file_for_move(&source).unwrap(),
                    &source,
                    &destination,
                )
                .unwrap_err()
                .kind(),
            io::ErrorKind::CrossesDevices
        );
        for (from, to) in [(&source, &destination), (&destination, &source)] {
            move_file(from, to, None).unwrap();
            assert!(!from.exists());
            assert_eq!(fs::read(to).unwrap(), b"main stream");
            assert_eq!(fs::read(stream_path(to)).unwrap(), b"alternate stream");
            assert_eq!(fs::metadata(to).unwrap().modified().unwrap(), modified);
            let metadata = fs::metadata(to).unwrap();
            assert!(!platform().file_attributes(&metadata, "fixture").0);
        }
        assert_no_staging(first.path());
        assert_no_staging(second.path());
    }
}
