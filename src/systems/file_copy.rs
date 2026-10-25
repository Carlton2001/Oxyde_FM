//! Copy into a private staging directory before publishing the completed file.
use crate::platform::platform;
use super::copy_diagnostics::{Phase, Timer};
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
        let _timer = Timer::start(Phase::StagingCreate);
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
        let release_timer = Timer::start(Phase::WriterRelease);
        let expected = self
            .release_output()?
            .ok_or_else(|| io::Error::other("Copy output ownership unavailable"))?;
        drop(release_timer);
        let _open_timer = Timer::start(Phase::OutputOpen);
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
        let _timer = Timer::start(Phase::Cleanup);
        if let Err(error) = self.clean_owned_output() {
            log::warn!("Copy output preserved at {}: {error}", self.file.display());
            super::archive_recovery::retained(&platform().retained_file_path(&self.directory, &self.path).unwrap_or(self.path.clone()));
        }
        if let Some(pin) = self.pin.take() {
            if let Err(error) = platform().remove_copy_staging(&self.directory, pin, &self.path) {
                log::warn!("Cannot clean copy staging {}: {error}", self.path.display());
                super::archive_recovery::retained(&platform().retained_file_path(&self.directory, &self.path).unwrap_or(self.path.clone()));
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
        None,
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
    copy_with_parents_observed(source, destination, replace, held_source, destination_parents, progress, None)
}
pub(super) fn copy_with_parents_observed(
    source: &Path, destination: &Path, replace: bool,
    held_source: Option<&std::fs::File>, destination_parents: super::path_parents::HeldParents,
    progress: &mut dyn FnMut(u64) -> io::Result<()>,
    before_replace: Option<&mut dyn FnMut() -> io::Result<bool>>,
) -> io::Result<CopiedFile> {
    copy_with_parents_checked(
        source,
        destination,
        replace,
        held_source,
        None,
        destination_parents,
        progress,
        before_replace,
    )
}

#[allow(clippy::too_many_arguments)]
fn copy_with_parents_checked(
    source: &Path,
    destination: &Path,
    replace: bool,
    held_source: Option<&std::fs::File>,
    expected: Option<&str>,
    destination_parents: super::path_parents::HeldParents,
    progress: &mut dyn FnMut(u64) -> io::Result<()>,
    before_replace: Option<&mut dyn FnMut() -> io::Result<bool>>,
) -> io::Result<CopiedFile> {
    let mut timing = super::copy_diagnostics::FileTiming::start(source, destination);
    let parents_timer = Timer::start(Phase::Parents);
    let source_parents = super::path_parents::HeldParents::new(source)?;
    let source = source_parents.resolved_path(source)?;
    let destination = destination_parents.resolved_path(destination)?;
    drop(parents_timer);
    let checks_timer = Timer::start(Phase::Checks);
    progress(0)?;
    if replace {
        check_replacement(&destination)?;
    }
    drop(checks_timer);
    let staging = Staging::new(&destination_parents)?;
    let native_timer = super::copy_diagnostics::Timer::start(super::copy_diagnostics::Phase::Native);
    platform().copy_file_with_progress(
        &source,
        &staging.file,
        held_source,
        &staging.directory,
        &mut |file| staging.capture_output(file),
        progress,
    )?;
    drop(native_timer);
    let mut output = staging.open_output()?;
    timing.bytes = output.metadata()?.len();
    let hash_timer = super::copy_diagnostics::Timer::start(super::copy_diagnostics::Phase::Hash);
    let hash = hash(&mut output, progress)?;
    drop(hash_timer);
    let checks_timer = Timer::start(Phase::Checks);
    if expected.is_some_and(|expected| expected != hash) {
        // The output is still private: failure never publishes unexpected bytes.
        return Err(history_source_changed(&source));
    }
    let exclusive = if replace { if let Some(before_replace) = before_replace { before_replace()? } else { false } } else { false };
    if replace && !exclusive {
        check_replacement(&destination)?;
    }
    drop(checks_timer);
    let publish_timer = super::copy_diagnostics::Timer::start(super::copy_diagnostics::Phase::Publish);
    destination_parents.rename(&output, &staging.file, &destination, replace && !exclusive)?;
    drop(publish_timer);
    timing.published = true;
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
#[path = "file_copy/tests/tests.rs"]
mod tests;
