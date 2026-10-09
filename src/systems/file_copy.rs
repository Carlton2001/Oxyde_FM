//! Copy into a private staging directory before publishing the completed file.
use crate::platform::platform;
use std::io;
use std::io::Read;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, PartialEq, Eq)]
struct StagingIdentity {
    object: crate::platform::FileIdentity,
    created: Option<std::time::SystemTime>,
}
impl StagingIdentity {
    fn from_file(file: &std::fs::File) -> io::Result<Self> {
        let created = file.metadata()?.created();
        #[cfg(target_os = "windows")]
        let created = Some(created?);
        #[cfg(not(target_os = "windows"))]
        let created = created.ok(); // Existing Unix copies do not require filesystem birth-time support.
        Ok(Self {
            object: platform().file_identity(file)?,
            created,
        })
    }
    fn matches(self, file: &std::fs::File) -> io::Result<bool> {
        Ok(self == Self::from_file(file)?)
    }
}

pub(super) struct Staging {
    pub directory: std::fs::File,
    pin: Option<std::fs::File>,
    owned_output: std::cell::RefCell<Option<std::fs::File>>,
    owned_identity: std::cell::Cell<Option<StagingIdentity>>,
    path: PathBuf,
    pub file: PathBuf,
}
impl Staging {
    fn new(parents: &super::path_parents::HeldParents) -> io::Result<Self> {
        Self::with_prefix(parents, ".oxyde-copy-")
    }
    pub fn with_prefix(
        parents: &super::path_parents::HeldParents,
        prefix: &str,
    ) -> io::Result<Self> {
        let leaf = format!("{prefix}{}", uuid::Uuid::new_v4());
        let path = parents.resolved_path(Path::new(&leaf))?;
        let (directory, pin) = platform().create_copy_staging(parents.directory(), &path)?;
        Ok(Self {
            directory,
            pin: Some(pin),
            owned_output: std::cell::RefCell::new(None),
            owned_identity: std::cell::Cell::new(None),
            file: path.join("output"),
            path,
        })
    }
    pub(super) fn capture_output(&self, file: &std::fs::File) -> io::Result<()> {
        let mut owned = self.owned_output.borrow_mut();
        if owned.is_none() {
            *owned = Some(file.try_clone()?);
        }
        Ok(())
    }
    fn release_output(&self) -> io::Result<Option<StagingIdentity>> {
        if let Some(file) = self.owned_output.borrow_mut().take() {
            // Native copies apply source dates after their last callback. Capture
            // the final proof while the actual output still has a retained handle.
            let identity = StagingIdentity::from_file(&file);
            drop(file); // The writer's access would conflict with the DELETE snapshot.
            self.owned_identity.set(identity.as_ref().ok().copied());
            identity?;
        }
        Ok(self.owned_identity.get())
    }
    pub(super) fn open_output(&self) -> io::Result<std::fs::File> {
        let expected = self
            .release_output()?
            .ok_or_else(|| io::Error::other("Copy output ownership unavailable"))?;
        let file = platform().open_copied_file(&self.directory, &self.file)?;
        if !expected.matches(&file)? {
            return Err(identity_changed(&self.file));
        }
        Ok(file)
    }
    fn clean_owned_output(&self) -> io::Result<()> {
        if let Some(expected) = self.release_output()? {
            let file = match platform().open_copied_file(&self.directory, &self.file) {
                Ok(file) => file,
                Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
                Err(error) => return Err(error),
            };
            if !expected.matches(&file)? {
                return Err(identity_changed(&self.file));
            }
            platform().remove_copied_file(&file, &self.file)?;
        }
        Ok(())
    }
}
impl Drop for Staging {
    fn drop(&mut self) {
        if let Err(error) = self.clean_owned_output() {
            log::warn!("Copy output preserved at {}: {error}", self.file.display());
        }
        if let Some(pin) = self.pin.take() {
            if let Err(error) = platform().remove_copy_staging(&self.directory, pin, &self.path) {
                log::warn!("Cannot clean copy staging {}: {error}", self.path.display());
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

pub(crate) fn copy_verified(
    source: &Path,
    destination: &Path,
    expected: &str,
    progress: &mut dyn FnMut(u64) -> io::Result<()>,
) -> io::Result<(
    String,
    Option<crate::models::transaction::HistoryFileIdentity>,
)> {
    copy_verified_with_checkpoint(source, destination, expected, &mut || Ok(()), progress)
}

fn copy_verified_with_checkpoint(
    source: &Path,
    destination: &Path,
    expected: &str,
    checkpoint: &mut dyn FnMut() -> io::Result<()>,
    progress: &mut dyn FnMut(u64) -> io::Result<()>,
) -> io::Result<(
    String,
    Option<crate::models::transaction::HistoryFileIdentity>,
)> {
    let source_parents = super::path_parents::HeldParents::new(source)?;
    let resolved = source_parents.resolved_path(source)?;
    // Read access only: Copy must not require DELETE permission on its source.
    // Windows keeps this ordinary file unchanged through hashing and publication.
    let mut original = platform().open_archive_source(&resolved, false)?;
    if super::history_move::hash_with(&mut original, &mut || progress(0))? != expected {
        return Err(history_source_changed(source));
    }
    checkpoint()?;
    let destination_parents = super::path_parents::HeldParents::new(destination)?;
    copy_with_parents_checked(
        &resolved,
        destination,
        false,
        Some(&original),
        Some(expected),
        destination_parents,
        progress,
    )
    .map(|output| {
        let identity = crate::models::transaction::HistoryFileIdentity::from_file(&output.file)
            .map_err(|error| {
                log::warn!(
                    "Redo copy identity unavailable for {}: {error}",
                    destination.display()
                )
            })
            .ok();
        (output.hash, identity)
    })
}

pub(super) fn identity_changed(path: &Path) -> io::Error {
    io::Error::other(crate::models::error::HistoryIdentityFailure {
        issue: crate::models::error::HistoryIdentityIssue::Changed,
        path: path.to_string_lossy().into_owned(),
    })
}

fn history_source_changed(source: &Path) -> io::Error {
    io::Error::other(crate::models::error::HistoryCopyFailure {
        issue: crate::models::error::HistoryCopyIssue::SourceChanged,
        path: source.to_string_lossy().into_owned(),
    })
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
    let destination_parents = super::path_parents::HeldParents::new(destination)?;
    copy_with_parents(
        source,
        destination,
        replace,
        held_source,
        destination_parents,
        progress,
    )
}

pub(super) fn copy_with_parents(
    source: &Path,
    destination: &Path,
    replace: bool,
    held_source: Option<&std::fs::File>,
    destination_parents: super::path_parents::HeldParents,
    progress: &mut dyn FnMut(u64) -> io::Result<()>,
) -> io::Result<CopiedFile> {
    copy_with_parents_checked(
        source,
        destination,
        replace,
        held_source,
        None,
        destination_parents,
        progress,
    )
}

fn copy_with_parents_checked(
    source: &Path,
    destination: &Path,
    replace: bool,
    held_source: Option<&std::fs::File>,
    expected: Option<&str>,
    destination_parents: super::path_parents::HeldParents,
    progress: &mut dyn FnMut(u64) -> io::Result<()>,
) -> io::Result<CopiedFile> {
    let source_parents = super::path_parents::HeldParents::new(source)?;
    let source = source_parents.resolved_path(source)?;
    let destination = destination_parents.resolved_path(destination)?;
    progress(0)?;
    if replace {
        check_replacement(&destination)?;
    }
    let staging = Staging::new(&destination_parents)?;
    platform().copy_file_with_progress(
        &source,
        &staging.file,
        held_source,
        &staging.directory,
        &mut |file| staging.capture_output(file),
        progress,
    )?;
    let mut output = staging.open_output()?;
    let hash = hash(&mut output, progress)?;
    if expected.is_some_and(|expected| expected != hash) {
        // The output is still private: failure never publishes unexpected bytes.
        return Err(history_source_changed(&source));
    }
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

#[cfg(test)]
pub(crate) fn remove_verified_output(path: &Path, expected: &str) -> io::Result<()> {
    let parents = super::path_parents::HeldParents::new(path)?;
    remove_verified_output_checked(path, expected, &parents, None)
}

pub(crate) fn remove_verified_output_identity(
    path: &Path,
    expected: &str,
    identity: Option<crate::models::transaction::HistoryFileIdentity>,
) -> io::Result<()> {
    // Redo rollback must retain an output if its identity could not be captured.
    let identity = identity.ok_or_else(|| {
        io::Error::other(crate::models::error::HistoryIdentityFailure {
            issue: crate::models::error::HistoryIdentityIssue::Unverified,
            path: path.to_string_lossy().into_owned(),
        })
    })?;
    let parents = super::path_parents::HeldParents::new(path)?;
    remove_verified_output_checked(path, expected, &parents, Some(identity))
}

pub(super) fn remove_verified_output_checked(
    path: &Path,
    expected: &str,
    parents: &super::path_parents::HeldParents,
    identity: Option<crate::models::transaction::HistoryFileIdentity>,
) -> io::Result<()> {
    let resolved = parents.resolved_path(path)?;
    let mut file = platform().open_copied_file(parents.directory(), &resolved)?;
    if let Some(identity) = identity {
        if !identity.matches(&file)? {
            return Err(identity_changed(path));
        }
    }
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
    platform().remove_copied_file(&file, &resolved)
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
    fn staging_preserves_unowned_outputs_after_a_native_creation_collision() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source");
        fs::write(&source, b"selected contents").unwrap();
        let parents =
            super::super::path_parents::HeldParents::new(&root.path().join("destination")).unwrap();
        let staging = Staging::new(&parents).unwrap();
        let path = staging.path.clone();
        fs::write(&staging.file, b"independent contents").unwrap();
        let mut captured = false;
        let error = platform()
            .copy_file_with_progress(
                &source,
                &staging.file,
                None,
                &staging.directory,
                &mut |file| {
                    captured = true;
                    staging.capture_output(file)
                },
                &mut |_| Ok(()),
            )
            .unwrap_err();
        assert_eq!(error.kind(), ErrorKind::AlreadyExists);
        assert!(!captured);
        drop(staging);
        assert_eq!(
            fs::read(path.join("output")).unwrap(),
            b"independent contents"
        );
        assert!(!path.join(".pin").exists());
    }
    #[test]
    fn staging_refuses_same_content_replacements_for_publication_and_cleanup() {
        let root = tempfile::tempdir().unwrap();
        let parents =
            super::super::path_parents::HeldParents::new(&root.path().join("destination")).unwrap();
        let staging = Staging::new(&parents).unwrap();
        let path = staging.path.clone();
        let file = platform()
            .create_operation_output(&staging.directory, &staging.file)
            .unwrap();
        use std::io::Write;
        (&file).write_all(b"matching contents").unwrap();
        staging.capture_output(&file).unwrap();
        drop(file);
        staging.release_output().unwrap();
        let displaced = root.path().join("original-output");
        fs::rename(&staging.file, &displaced).unwrap();
        fs::write(&staging.file, b"matching contents").unwrap();
        assert!(staging.open_output().is_err());
        drop(staging);
        assert_eq!(fs::read(path.join("output")).unwrap(), b"matching contents");
        assert_eq!(fs::read(displaced).unwrap(), b"matching contents");
        assert!(!path.join(".pin").exists());
    }
    #[test]
    fn staging_cleans_only_owned_output_and_preserves_unknown_children() {
        let root = tempfile::tempdir().unwrap();
        let parents =
            super::super::path_parents::HeldParents::new(&root.path().join("destination")).unwrap();
        let staging = Staging::new(&parents).unwrap();
        let path = staging.path.clone();
        let file = platform()
            .create_operation_output(&staging.directory, &staging.file)
            .unwrap();
        staging.capture_output(&file).unwrap();
        drop(file);
        fs::write(path.join("independent"), b"independent contents").unwrap();
        drop(staging);
        assert!(!path.join("output").exists());
        assert!(!path.join(".pin").exists());
        assert_eq!(
            fs::read(path.join("independent")).unwrap(),
            b"independent contents"
        );
    }
    #[test]
    fn staging_keeps_unverified_ownership_without_adopting_a_file() {
        let root = tempfile::tempdir().unwrap();
        let parents =
            super::super::path_parents::HeldParents::new(&root.path().join("destination")).unwrap();
        let staging = Staging::new(&parents).unwrap();
        let path = staging.path.clone();
        fs::write(&staging.file, b"independent contents").unwrap();
        assert!(staging.open_output().is_err());
        drop(staging);
        assert_eq!(
            fs::read(path.join("output")).unwrap(),
            b"independent contents"
        );
    }
    #[test]
    fn identity_checked_cleanup_preserves_a_matching_replacement() {
        let root = tempfile::tempdir().unwrap();
        let output = root.path().join("output");
        let saved = root.path().join("selected-output");
        fs::write(&output, b"matching contents").unwrap();
        let identity = crate::models::transaction::HistoryFileIdentity::from_file(
            &fs::File::open(&output).unwrap(),
        )
        .unwrap();
        fs::rename(&output, &saved).unwrap();
        fs::write(&output, b"matching contents").unwrap();
        let expected = blake3::hash(b"matching contents").to_hex().to_string();
        let error =
            remove_verified_output_identity(&output, &expected, Some(identity)).unwrap_err();
        assert!(matches!(
            crate::models::CommandError::from(error),
            crate::models::CommandError::HistoryIdentity(_)
        ));
        assert_eq!(fs::read(output).unwrap(), b"matching contents");
        assert_eq!(fs::read(saved).unwrap(), b"matching contents");
    }

    #[test]
    fn identity_cleanup_without_a_snapshot_preserves_the_output() {
        let root = tempfile::tempdir().unwrap();
        let output = root.path().join("output");
        fs::write(&output, b"retained contents").unwrap();
        let expected = blake3::hash(b"retained contents").to_hex().to_string();
        let error = remove_verified_output_identity(&output, &expected, None).unwrap_err();
        assert!(
            matches!(crate::models::CommandError::from(error), crate::models::CommandError::HistoryIdentity(failure) if failure.issue == crate::models::error::HistoryIdentityIssue::Unverified)
        );
        assert_eq!(fs::read(output).unwrap(), b"retained contents");
    }

    #[test]
    fn redo_copy_rejects_changed_sources_and_cancels_verification_before_staging() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source");
        let destination = root.path().join("destination");
        fs::write(&source, vec![0x42; 256 * 1024]).unwrap();
        let error = copy_verified(&source, &destination, "old hash", &mut |_| Ok(())).unwrap_err();
        assert!(
            matches!(crate::models::CommandError::from(error), crate::models::CommandError::HistoryCopy(failure) if failure.issue == crate::models::error::HistoryCopyIssue::SourceChanged)
        );
        assert!(!destination.exists());
        let expected = blake3::hash(&vec![0x42; 256 * 1024]).to_hex().to_string();
        let mut calls = 0;
        let error = copy_verified(&source, &destination, &expected, &mut |_| {
            calls += 1;
            if calls == 2 {
                Err(io::Error::new(
                    ErrorKind::Interrupted,
                    "cancel verification",
                ))
            } else {
                Ok(())
            }
        })
        .unwrap_err();
        assert_eq!(error.kind(), ErrorKind::Interrupted);
        assert!(!destination.exists());
        assert_eq!(fs::read(&source).unwrap(), vec![0x42; 256 * 1024]);
        assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
    }

    #[test]
    fn redo_copy_never_publishes_a_staged_hash_mismatch() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source");
        let destination = root.path().join("destination");
        fs::write(&source, b"unexpected staged bytes").unwrap();
        let parents = super::super::path_parents::HeldParents::new(&destination).unwrap();
        let result = copy_with_parents_checked(
            &source,
            &destination,
            false,
            None,
            Some("expected journal hash"),
            parents,
            &mut |_| Ok(()),
        );
        let error = match result {
            Ok(_) => panic!("unverified output was published"),
            Err(error) => error,
        };
        assert!(matches!(
            crate::models::CommandError::from(error),
            crate::models::CommandError::HistoryCopy(_)
        ));
        assert!(!destination.exists());
        assert_eq!(fs::read(&source).unwrap(), b"unexpected staged bytes");
        assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
    }

    #[test]
    fn redo_copy_stays_in_its_resolved_source_after_junction_retargeting() {
        use crate::platform::windows_reparse_tests::{Retarget, junction};
        let root = tempfile::tempdir().unwrap();
        let selected = root.path().join("selected");
        let outside = root.path().join("outside");
        fs::create_dir(&selected).unwrap();
        fs::create_dir(&outside).unwrap();
        fs::write(selected.join("source"), b"verified source").unwrap();
        fs::write(outside.join("source"), b"outside witness").unwrap();
        let link = root.path().join("link");
        let template = root.path().join("template");
        junction(&link, &selected);
        junction(&template, &outside);
        let retarget = Retarget::new(&link, &template);
        let destination = root.path().join("destination");
        let expected = blake3::hash(b"verified source").to_hex().to_string();
        copy_verified_with_checkpoint(
            &link.join("source"),
            &destination,
            &expected,
            &mut || {
                retarget.apply();
                Ok(())
            },
            &mut |_| Ok(()),
        )
        .unwrap();
        assert_eq!(fs::read(destination).unwrap(), b"verified source");
        assert_eq!(
            fs::read(selected.join("source")).unwrap(),
            b"verified source"
        );
        assert_eq!(
            fs::read(outside.join("source")).unwrap(),
            b"outside witness"
        );
        drop(retarget);
        fs::remove_dir(link).unwrap();
        fs::remove_dir(template).unwrap();
    }

    #[test]
    fn redo_copy_keeps_the_verified_source_when_replacement_is_attempted() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source");
        let destination = root.path().join("destination");
        let displaced = root.path().join("displaced");
        fs::write(&source, b"verified contents").unwrap();
        let expected = blake3::hash(b"verified contents").to_hex().to_string();
        let mut blocked = false;
        let result = copy_verified_with_checkpoint(
            &source,
            &destination,
            &expected,
            &mut || {
                if fs::rename(&source, &displaced).is_ok() {
                    fs::write(&source, b"independent replacement")?;
                } else {
                    blocked = true;
                    assert!(fs::write(&source, b"concurrent write").is_err());
                }
                Ok(())
            },
            &mut |_| Ok(()),
        );
        assert!(result.is_ok(), "verified copy failed: {result:?}");
        assert_eq!(
            fs::read(&destination).unwrap(),
            b"verified contents",
            "replacement was published after successful hash verification"
        );
        assert!(blocked);
        assert_eq!(fs::read(&source).unwrap(), b"verified contents");
        assert!(!displaced.exists());
    }

    #[test]
    fn verified_copy_cleanup_preserves_readonly_foreign_hardlink_attributes() {
        let root = tempfile::tempdir().unwrap();
        let witness = root.path().join("foreign-witness");
        let output = root.path().join("copied-output");
        fs::write(&witness, b"same verified data").unwrap();
        fs::hard_link(&witness, &output).unwrap();
        let mut permissions = fs::metadata(&witness).unwrap().permissions();
        permissions.set_readonly(true);
        fs::set_permissions(&witness, permissions.clone()).unwrap();
        let expected = blake3::hash(b"same verified data").to_hex().to_string();
        let result = remove_verified_output(&output, &expected);
        let preserved = fs::metadata(&witness).unwrap().permissions().readonly();
        permissions.set_readonly(false);
        fs::set_permissions(&witness, permissions).unwrap();
        assert!(
            preserved,
            "verified copy cleanup changed the foreign hardlink attributes"
        );
        assert!(result.is_err());
        assert_eq!(fs::read(&witness).unwrap(), b"same verified data");
        assert_eq!(fs::read(&output).unwrap(), b"same verified data");
    }

    #[test]
    fn staging_is_never_created_in_a_new_reparse_target() {
        use crate::platform::windows_reparse_tests::{Retarget, junction};
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source");
        let selected = root.path().join("selected");
        let outside = root.path().join("outside");
        fs::write(&source, vec![0x42; 1024 * 1024]).unwrap();
        fs::create_dir(&selected).unwrap();
        fs::create_dir(&outside).unwrap();
        fs::write(outside.join("witness"), b"outside witness").unwrap();
        let template = root.path().join("template");
        junction(&template, &outside);
        let retarget = Retarget::new(&selected, &template);
        let mut changed = false;
        let mut foreign_staging = false;
        let result = copy(&source, &selected.join("output"), false, &mut |bytes| {
            if !changed {
                retarget.apply();
                changed = true;
            }
            foreign_staging |= fs::read_dir(&outside)?.any(|entry| {
                entry
                    .unwrap()
                    .file_name()
                    .to_string_lossy()
                    .starts_with(".oxyde-copy-")
            });
            if bytes > 0 {
                return Err(io::Error::new(ErrorKind::Interrupted, "test stop"));
            }
            Ok(())
        });
        retarget.clear();
        assert!(result.is_err());
        assert!(
            !foreign_staging,
            "copy preparation wrote outside the retained destination"
        );
        assert_eq!(
            fs::read(outside.join("witness")).unwrap(),
            b"outside witness"
        );
        assert_eq!(fs::read_dir(&selected).unwrap().count(), 0);
        drop(retarget);
        fs::remove_dir(template).unwrap();
    }

    #[test]
    fn active_copy_staging_resists_retargeting_and_is_cleaned_after_cancellation() {
        use crate::platform::windows_reparse_tests::{Retarget, junction};
        let fixture = Fixture::new();
        let outside = fixture.root.path().join("outside");
        fs::create_dir(&outside).unwrap();
        fs::write(outside.join("witness"), b"unchanged").unwrap();
        let template = fixture.root.path().join("template");
        junction(&template, &outside);
        let mut checked = false;
        let error = copy(&fixture.source, &fixture.destination, false, &mut |bytes| {
            if bytes > 0 {
                let staging = fs::read_dir(fixture.root.path())?
                    .find_map(|entry| {
                        let entry = entry.unwrap();
                        entry
                            .file_name()
                            .to_string_lossy()
                            .starts_with(".oxyde-copy-")
                            .then(|| entry.path())
                    })
                    .unwrap();
                assert!(fs::remove_file(staging.join(".pin")).is_err());
                let retarget = Retarget::new(&staging, &template);
                assert!(retarget.try_apply().is_err());
                checked = true;
                return Err(io::Error::new(ErrorKind::Interrupted, "test stop"));
            }
            Ok(())
        })
        .unwrap_err();
        assert!(checked);
        assert_eq!(error.kind(), ErrorKind::Interrupted);
        assert_eq!(fs::read(outside.join("witness")).unwrap(), b"unchanged");
        assert_eq!(fs::read_dir(&outside).unwrap().count(), 1);
        fixture.no_staging();
        assert!(!fixture.destination.exists());
        fs::remove_dir(template).unwrap();
    }

    #[test]
    fn copy_stays_in_the_selected_parent_after_junction_retargeting() {
        use crate::platform::windows_reparse_tests::{Retarget, junction};
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
            assert!(fs::read_dir(&selected).unwrap().all(|entry| {
                !entry
                    .unwrap()
                    .file_name()
                    .to_string_lossy()
                    .starts_with(".oxyde-copy-")
            }));
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
            assert!(fs::read_dir(self.root.path()).unwrap().all(|entry| {
                !entry
                    .unwrap()
                    .file_name()
                    .to_string_lossy()
                    .starts_with(".oxyde-copy-")
            }));
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
        assert!(
            fs::metadata(&fixture.source)
                .unwrap()
                .permissions()
                .readonly()
        );
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
        assert!(
            fs::metadata(&fixture.destination)
                .unwrap()
                .permissions()
                .readonly()
        );
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
            assert!(
                fs::File::options()
                    .write(true)
                    .open(&fixture.source)
                    .is_err()
            );
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
