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
    copy_with_source(source, destination, replace, None, progress).map(|output| output.hash)
}

pub(super) struct CopiedFile {
    pub file: std::fs::File,
    pub hash: String,
    pub(super) _parents: super::path_parents::HeldParents,
}

pub(super) fn copy_with_source(
    source: &Path,
    destination: &Path,
    replace: bool,
    held_source: Option<&std::fs::File>,
    progress: &mut dyn FnMut(u64) -> io::Result<()>,
) -> io::Result<CopiedFile> {
    let source_parents = super::path_parents::HeldParents::new(source)?;
    let source = source_parents.resolved_path(source)?;
    let destination_parents = super::path_parents::HeldParents::new(destination)?;
    let destination = destination_parents.resolved_path(destination)?;
    progress(0)?;
    if replace {
        check_replacement(&destination)?;
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
    platform().copy_file_with_progress(&source, &staging.file, held_source, progress)?;
    let mut output = platform().open_file_for_move(&staging.file)?;
    let hash = hash(&mut output, progress)?;
    if replace {
        check_replacement(&destination)?;
    }
    destination_parents.rename(&output, &staging.file, &destination, replace)?;
    Ok(CopiedFile {
        file: output,
        hash,
        _parents: destination_parents,
    })
}

/// Remove only a journal-verified output, including copied readonly files.
pub(crate) fn remove_verified_output(path: &Path, expected: &str) -> io::Result<()> {
    let _parents = super::path_parents::HeldParents::new(path)?;
    let mut file = platform().open_file_for_move(path)?;
    let metadata = file.metadata()?;
    if !metadata.is_file()
        || platform().file_attributes(&metadata, "").2
        || super::history_move::hash(&mut file)? != expected
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

fn hash(
    file: &mut std::fs::File,
    progress: &mut dyn FnMut(u64) -> io::Result<()>,
) -> io::Result<String> {
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

    #[test]
    fn copy_stays_in_the_selected_parent_after_junction_retargeting() {
        use crate::platform::windows_reparse_tests::{junction, Retarget};
        for replace in [false, true] {
            let root = tempfile::tempdir().unwrap();
            let source_directory = root.path().join("source-directory");
            fs::create_dir(&source_directory).unwrap();
            let source = source_directory.join("input");
            let selected = root.path().join("selected");
            let outside = root.path().join("outside");
            fs::create_dir(&selected).unwrap();
            fs::create_dir(&outside).unwrap();
            fs::write(&source, b"selected data").unwrap();
            fs::write(outside.join("witness"), b"outside witness").unwrap();
            fs::write(outside.join("input"), b"foreign source").unwrap();
            if replace {
                fs::write(selected.join("output"), b"old data").unwrap();
                fs::write(outside.join("output"), b"foreign output").unwrap();
            }
            let link = root.path().join("link");
            let template = root.path().join("template");
            let source_link = root.path().join("source-link");
            junction(&link, &selected);
            junction(&template, &outside);
            junction(&source_link, &source_directory);
            let retarget = Retarget::new(&link, &template);
            let source_retarget = Retarget::new(&source_link, &template);
            let mut changed = false;
            copy(
                &source_link.join("input"),
                &link.join("output"),
                replace,
                &mut |_| {
                    if !changed {
                        retarget.apply();
                        source_retarget.apply();
                        changed = true;
                    }
                    Ok(())
                },
            )
            .unwrap();
            assert_eq!(fs::read(selected.join("output")).unwrap(), b"selected data");
            assert_eq!(
                fs::read(outside.join("witness")).unwrap(),
                b"outside witness"
            );
            assert_eq!(fs::read(outside.join("input")).unwrap(), b"foreign source");
            assert_eq!(fs::read(&source).unwrap(), b"selected data");
            if replace {
                assert_eq!(fs::read(outside.join("output")).unwrap(), b"foreign output");
            } else {
                assert!(!outside.join("output").exists());
            }
            assert_eq!(
                fs::read_dir(&outside).unwrap().count(),
                if replace { 3 } else { 2 }
            );
            assert!(fs::read_dir(&selected).unwrap().all(|entry| !entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with(".oxyde-copy-")));
            drop((retarget, source_retarget));
            fs::remove_dir(link).unwrap();
            fs::remove_dir(template).unwrap();
            fs::remove_dir(source_link).unwrap();
        }
    }

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
    fn copy_keeps_source_and_publication_parents_fixed_during_progress() {
        let root = tempfile::tempdir().unwrap();
        let source_parent = root.path().join("source-parent");
        let target_parent = root.path().join("target-parent");
        fs::create_dir(&source_parent).unwrap();
        fs::create_dir(&target_parent).unwrap();
        let source = source_parent.join("file");
        let destination = target_parent.join("file");
        fs::write(&source, vec![0x42; 256 * 1024]).unwrap();
        let mut checked = false;
        copy(&source, &destination, false, &mut |bytes| {
            if bytes > 0 && !checked {
                assert!(fs::rename(&source_parent, root.path().join("replaced-source")).is_err());
                assert!(fs::rename(&target_parent, root.path().join("replaced-target")).is_err());
                checked = true;
            }
            Ok(())
        })
        .unwrap();
        assert!(checked);
        assert_eq!(fs::read(&source).unwrap(), fs::read(&destination).unwrap());
        fs::rename(&target_parent, root.path().join("released-target")).unwrap();
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
