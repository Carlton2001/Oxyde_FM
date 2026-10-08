//! Verified history moves; copy only when the filesystem reports a volume boundary.
use crate::platform::platform;
use std::fs::{self, File};
use std::io::{self, Read, Seek, SeekFrom};
use std::path::Path;

trait MoveIo {
    fn rename(&self, file: &File, source: &Path, destination: &Path) -> io::Result<()>;
    fn copy(&self, source: &Path, destination: &Path) -> io::Result<u64>;
    fn remove(&self, file: &File, path: &Path) -> io::Result<()>;
}
struct NativeIo;
impl MoveIo for NativeIo {
    fn rename(&self, file: &File, source: &Path, destination: &Path) -> io::Result<()> {
        platform().rename_open_file_exclusive(file, source, destination)
    }
    fn copy(&self, source: &Path, destination: &Path) -> io::Result<u64> {
        fs::copy(source, destination)
    }
    fn remove(&self, file: &File, path: &Path) -> io::Result<()> {
        platform().remove_open_file(file, path)
    }
}

struct StagingFile(tempfile::TempPath);
impl Drop for StagingFile {
    fn drop(&mut self) {
        // CopyFile also preserves readonly attributes. Only this generated
        // temporary path is made writable for cleanup, never the source.
        if let Ok(metadata) = fs::metadata(&self.0) {
            let mut permissions = metadata.permissions();
            if permissions.readonly() {
                permissions.set_readonly(false);
                let _ = fs::set_permissions(&self.0, permissions);
            }
        }
    }
}

fn hash(file: &mut File) -> io::Result<String> {
    file.seek(SeekFrom::Start(0))?;
    let mut hasher = blake3::Hasher::new();
    let mut buffer = [0u8; 128 * 1024];
    loop {
        let size = file.read(&mut buffer)?;
        if size == 0 {
            break;
        }
        hasher.update(&buffer[..size]);
    }
    Ok(hasher.finalize().to_hex().to_string())
}

pub(crate) fn move_file(
    source: &Path,
    destination: &Path,
    expected: Option<&str>,
) -> io::Result<()> {
    move_with(&NativeIo, source, destination, expected)
}

fn move_with(
    ops: &impl MoveIo,
    source: &Path,
    destination: &Path,
    expected: Option<&str>,
) -> io::Result<()> {
    if fs::symlink_metadata(source)?.file_type().is_symlink() {
        return Err(io::Error::other(
            "History moves cannot follow symbolic links",
        ));
    }
    let mut original = platform().open_file_for_move(source)?;
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
    match ops.rename(&original, source, destination) {
        Ok(()) => return Ok(()),
        Err(error) if error.kind() == io::ErrorKind::CrossesDevices => {}
        Err(error) => return Err(error),
    }

    let parent = destination.parent().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "Move destination has no parent",
        )
    })?;
    // CopyFile refuses an existing hidden destination. Hide the containing
    // staging directory instead; the copied file retains the source attributes.
    let staging_dir = tempfile::Builder::new()
        .prefix(".oxyde-history-")
        .tempdir_in(parent)?;
    platform()
        .set_hidden_attribute(staging_dir.path(), true)
        .map_err(io::Error::other)?;
    let staging = StagingFile(
        tempfile::Builder::new()
            .tempfile_in(staging_dir.path())?
            .into_temp_path(),
    );
    let copied = ops.copy(source, &staging.0)?;
    if copied != metadata.len() {
        return Err(io::Error::other("Incomplete history move copy"));
    }
    let copied_permissions = fs::metadata(&staging.0)?.permissions();
    if copied_permissions.readonly() {
        let mut writable = copied_permissions.clone();
        writable.set_readonly(false);
        fs::set_permissions(&staging.0, writable)?;
    }
    {
        let output = File::options().read(true).write(true).open(&staging.0)?;
        output.sync_all()?;
        output.set_permissions(copied_permissions)?;
    }
    let mut output = platform().open_file_for_move(&staging.0)?;
    if hash(&mut output)? != original_hash || hash(&mut original)? != original_hash {
        return Err(io::Error::other(
            "History move copy failed content verification",
        ));
    }
    ops.rename(&output, &staging.0, destination)?;
    if let Err(error) = ops.remove(&original, source) {
        // The opened destination still belongs to this operation. On Windows
        // its handle prevents an external writer or path replacement here.
        let cleanup = (|| {
            let mut permissions = output.metadata()?.permissions();
            if permissions.readonly() {
                permissions.set_readonly(false);
                fs::set_permissions(destination, permissions)?;
            }
            platform().remove_open_file(&output, destination)
        })();
        return match cleanup {
            Ok(()) => Err(error),
            Err(cleanup) => Err(io::Error::other(format!(
                "{error}; cannot remove published copy {}: {cleanup}",
                destination.display()
            ))),
        };
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
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
        fn rename(&self, file: &File, source: &Path, destination: &Path) -> io::Result<()> {
            self.attempts.set(self.attempts.get() + 1);
            if source == self.source {
                return Err(io::Error::new(
                    io::ErrorKind::CrossesDevices,
                    "injected volume boundary",
                ));
            }
            if matches!(self.fault, Fault::Conflict) {
                fs::write(&self.destination, b"external file")?;
            }
            NativeIo.rename(file, source, destination)
        }
        fn copy(&self, source: &Path, destination: &Path) -> io::Result<u64> {
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
            let count = NativeIo.copy(source, destination)?;
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
