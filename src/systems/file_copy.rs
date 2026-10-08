//! Copy into a private staging directory before publishing the completed file.
use crate::platform::platform;
use std::io;
use std::io::Read;
use std::path::{Path, PathBuf};

struct Staging {
    _directory: tempfile::TempDir,
    file: PathBuf,
}
impl Drop for Staging {
    fn drop(&mut self) {
        if let Ok(metadata) = std::fs::metadata(&self.file) {
            let mut permissions = metadata.permissions();
            if permissions.readonly() {
                permissions.set_readonly(false);
                let _ = std::fs::set_permissions(&self.file, permissions);
            }
        }
    }
}
pub(crate) fn copy(
    source: &Path,
    destination: &Path,
    replace: bool,
    progress: &mut dyn FnMut(u64) -> io::Result<()>,
) -> io::Result<String> {
    progress(0)?;
    if replace {
        check_replacement(destination)?;
    }
    let directory = tempfile::Builder::new().prefix(".oxyde-copy-").tempdir_in(
        destination
            .parent()
            .ok_or_else(|| io::Error::other("Copy destination has no parent"))?,
    )?;
    platform()
        .set_hidden_attribute(directory.path(), true)
        .map_err(io::Error::other)?;
    let staging = Staging {
        file: directory.path().join("output"),
        _directory: directory,
    };
    platform().copy_file_with_progress(source, &staging.file, progress)?;
    let output = platform().open_file_for_move(&staging.file)?;
    let hash = hash(&staging.file, progress)?;
    if replace {
        check_replacement(destination)?;
        platform().replace_open_file(&output, &staging.file, destination)?;
    } else {
        platform().rename_open_file_exclusive(&output, &staging.file, destination)?;
    }
    Ok(hash)
}

/// Remove only a journal-verified output, including copied readonly files.
pub(crate) fn remove_verified_output(path: &Path, expected: &str) -> io::Result<()> {
    let file = platform().open_file_for_move(path)?;
    let metadata = file.metadata()?;
    if !metadata.is_file()
        || platform().file_attributes(&metadata, "").2
        || super::history_move::hash_file(path)? != expected
    {
        return Err(io::Error::other(format!(
            "Copied output changed: {}",
            path.display()
        )));
    }
    let original = metadata.permissions();
    if original.readonly() {
        let mut writable = original.clone();
        writable.set_readonly(false);
        std::fs::set_permissions(path, writable)?;
    }
    let result = platform().remove_open_file(&file, path);
    if result.is_err() && original.readonly() {
        std::fs::set_permissions(path, original)?;
    }
    result
}

fn check_replacement(path: &Path) -> io::Result<()> {
    let metadata = std::fs::symlink_metadata(path)?;
    if !metadata.is_file()
        || metadata.file_type().is_symlink()
        || platform().file_attributes(&metadata, "").2
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Copy target is not an ordinary file",
        ));
    }
    if metadata.permissions().readonly() {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "Copy target is readonly",
        ));
    }
    Ok(())
}

fn hash(path: &Path, progress: &mut dyn FnMut(u64) -> io::Result<()>) -> io::Result<String> {
    let mut file = std::fs::File::open(path)?;
    let size = file.metadata()?.len();
    let mut hasher = blake3::Hasher::new();
    let mut buffer = vec![0_u8; 128 * 1024];
    loop {
        progress(size)?;
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }
    Ok(hasher.finalize().to_hex().to_string())
}

#[cfg(all(test, target_os = "windows"))]
mod tests {
    use super::*;
    use std::fs;
    use std::io::ErrorKind;

    struct Fixture {
        root: tempfile::TempDir,
        source: PathBuf,
        destination: PathBuf,
    }
    impl Fixture {
        fn new() -> Self {
            let root = tempfile::tempdir().unwrap();
            let source = root.path().join("source");
            let destination = root.path().join("destination");
            fs::write(&source, vec![0x42_u8; 3 * 1024 * 1024]).unwrap();
            Self {
                root,
                source,
                destination,
            }
        }
        fn no_staging(&self) {
            assert!(fs::read_dir(self.root.path()).unwrap().all(|entry| !entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with(".oxyde-copy-")));
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            for path in [&self.source, &self.destination] {
                if let Ok(metadata) = fs::metadata(path) {
                    let mut permissions = metadata.permissions();
                    permissions.set_readonly(false);
                    fs::set_permissions(path, permissions).unwrap();
                }
            }
        }
    }

    #[test]
    fn cancellation_during_native_copy_removes_only_private_partial_output() {
        let fixture = Fixture::new();
        let error = copy(&fixture.source, &fixture.destination, false, &mut |bytes| {
            if bytes > 0 {
                return Err(io::Error::new(
                    ErrorKind::Interrupted,
                    "injected mid-copy cancellation",
                ));
            }
            Ok(())
        })
        .unwrap_err();
        assert_eq!(error.to_string(), "injected mid-copy cancellation");
        assert!(!fixture.destination.exists());
        assert_eq!(
            fs::metadata(&fixture.source).unwrap().len(),
            3 * 1024 * 1024
        );
        fixture.no_staging();
    }

    #[test]
    fn cancellation_before_replacement_preserves_old_contents_without_publication() {
        let fixture = Fixture::new();
        fs::write(&fixture.destination, b"old contents").unwrap();
        let error = copy(&fixture.source, &fixture.destination, true, &mut |bytes| {
            if bytes > 0 {
                return Err(io::Error::new(
                    ErrorKind::Interrupted,
                    "injected overwrite stop",
                ));
            }
            Ok(())
        })
        .unwrap_err();
        assert_eq!(error.to_string(), "injected overwrite stop");
        assert_eq!(fs::read(&fixture.destination).unwrap(), b"old contents");
        assert!(fixture.source.is_file());
        fixture.no_staging();
    }

    #[test]
    fn callback_panic_stops_native_copy_without_crossing_ffi() {
        let fixture = Fixture::new();
        let error = copy(&fixture.source, &fixture.destination, false, &mut |bytes| {
            if bytes > 0 {
                panic!("injected callback panic");
            }
            Ok(())
        })
        .unwrap_err();
        assert_eq!(error.to_string(), "Copy progress callback panicked");
        assert!(!fixture.destination.exists());
        fixture.no_staging();
    }

    #[test]
    fn publication_conflict_keeps_foreign_file_and_cleans_readonly_copy() {
        let fixture = Fixture::new();
        let mut permissions = fs::metadata(&fixture.source).unwrap().permissions();
        permissions.set_readonly(true);
        fs::set_permissions(&fixture.source, permissions).unwrap();
        let error = copy(&fixture.source, &fixture.destination, false, &mut |bytes| {
            if bytes == 0 {
                fs::write(&fixture.destination, b"foreign content")?;
            }
            Ok(())
        })
        .unwrap_err();
        assert_eq!(error.kind(), ErrorKind::AlreadyExists);
        assert_eq!(fs::read(&fixture.destination).unwrap(), b"foreign content");
        assert!(fs::metadata(&fixture.source)
            .unwrap()
            .permissions()
            .readonly());
        fixture.no_staging();
    }

    #[test]
    fn hidden_overwrite_copies_streams_and_preserves_monotonic_main_progress() {
        let fixture = Fixture::new();
        fs::write(
            fixture.source.with_file_name("source:extra"),
            b"stream data",
        )
        .unwrap();
        fs::write(&fixture.destination, b"old contents").unwrap();
        platform()
            .set_hidden_attribute(&fixture.source, true)
            .unwrap();
        platform()
            .set_hidden_attribute(&fixture.destination, true)
            .unwrap();
        let mut last = 0;
        let hash = copy(&fixture.source, &fixture.destination, true, &mut |bytes| {
            assert!(bytes >= last);
            last = bytes;
            Ok(())
        })
        .unwrap();
        assert_eq!(last, 3 * 1024 * 1024);
        assert_eq!(
            hash,
            blake3::hash(&fs::read(&fixture.source).unwrap())
                .to_hex()
                .to_string()
        );
        assert_eq!(
            fs::read(fixture.destination.with_file_name("destination:extra")).unwrap(),
            b"stream data"
        );
        assert!(
            platform()
                .file_attributes(&fs::metadata(&fixture.destination).unwrap(), "")
                .0
        );
        fixture.no_staging();
    }

    #[test]
    fn empty_readonly_copy_is_published_with_attributes() {
        let fixture = Fixture::new();
        fs::write(&fixture.source, []).unwrap();
        let mut permissions = fs::metadata(&fixture.source).unwrap().permissions();
        permissions.set_readonly(true);
        fs::set_permissions(&fixture.source, permissions).unwrap();
        let hash = copy(
            &fixture.source,
            &fixture.destination,
            false,
            &mut |_| Ok(()),
        )
        .unwrap();
        assert_eq!(hash, blake3::hash(b"").to_hex().to_string());
        assert_eq!(fs::metadata(&fixture.destination).unwrap().len(), 0);
        assert!(fs::metadata(&fixture.destination)
            .unwrap()
            .permissions()
            .readonly());
        fixture.no_staging();
    }

    #[test]
    fn native_checkpoint_can_pause_and_source_writer_is_refused_until_resume() {
        let fixture = Fixture::new();
        let (paused_tx, paused_rx) = std::sync::mpsc::sync_channel(1);
        let (resume_tx, resume_rx) = std::sync::mpsc::sync_channel(1);
        std::thread::scope(|scope| {
            let fixture = &fixture;
            let worker = scope.spawn(move || {
                let mut paused = false;
                copy(&fixture.source, &fixture.destination, false, &mut |bytes| {
                    if bytes > 0 && !paused {
                        paused = true;
                        paused_tx.send(()).unwrap();
                        resume_rx
                            .recv_timeout(std::time::Duration::from_secs(5))
                            .map_err(io::Error::other)?;
                    }
                    Ok(())
                })
            });
            paused_rx
                .recv_timeout(std::time::Duration::from_secs(5))
                .unwrap();
            assert!(!fixture.destination.exists());
            assert!(fs::File::options()
                .write(true)
                .open(&fixture.source)
                .is_err());
            resume_tx.send(()).unwrap();
            worker.join().unwrap().unwrap();
        });
        assert_eq!(
            fs::read(&fixture.destination).unwrap(),
            fs::read(&fixture.source).unwrap()
        );
        fixture.no_staging();
    }
}
