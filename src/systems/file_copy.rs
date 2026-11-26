//! Copy a file into a new output, verified by re-reading it, then kept.
use crate::platform::platform;
use super::copy_diagnostics::{Phase, Timer};
use std::io;
use std::io::Read;
use std::path::Path;

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
        Some(&source_parents),
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

/// Copies `source` into a new output for `destination`: the file itself, created
/// exclusively, or a temporary beside it for a replacement. A temporary name already
/// taken is drawn again.
fn write_output(
    source: &Path,
    destination: &Path,
    replace: bool,
    held_source: Option<&std::fs::File>,
    parents: &super::path_parents::HeldParents,
    progress: &mut dyn FnMut(u64) -> io::Result<()>,
) -> io::Result<super::copy_output::Output> {
    let mut attempt = 1;
    loop {
        let output = super::copy_output::Output::new(parents, destination, replace)?;
        let native_timer = Timer::start(Phase::Native);
        let result = platform().copy_file_with_progress(
            source,
            output.path(),
            held_source,
            parents.directory(),
            &mut |file| output.capture(file),
            progress,
        );
        drop(native_timer);
        match result {
            Ok(()) => return Ok(output),
            Err(error)
                if replace
                    && error.kind() == io::ErrorKind::AlreadyExists
                    && !output.captured()
                    && attempt < super::copy_output::TEMPORARY_ATTEMPTS =>
            {
                attempt += 1;
            }
            Err(error) => {
                // The output's cleanup may fail in turn and hide this cause.
                log::warn!("Copy to {} failed: {error}", output.path().display());
                return Err(error);
            }
        }
    }
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
    copy_with_parents_observed(source, destination, replace, held_source, None, destination_parents, progress, None)
}
/// `source_parents`, when given, already holds the parents of `source`, which
/// is then their resolved path: they are not opened again for each file.
#[allow(clippy::too_many_arguments)]
pub(super) fn copy_with_parents_observed(
    source: &Path, destination: &Path, replace: bool,
    held_source: Option<&std::fs::File>,
    source_parents: Option<&super::path_parents::HeldParents>,
    destination_parents: super::path_parents::HeldParents,
    progress: &mut dyn FnMut(u64) -> io::Result<()>,
    before_replace: Option<&mut dyn FnMut() -> io::Result<bool>>,
) -> io::Result<CopiedFile> {
    copy_with_parents_checked(
        source,
        destination,
        replace,
        held_source,
        source_parents,
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
    source_parents: Option<&super::path_parents::HeldParents>,
    expected: Option<&str>,
    destination_parents: super::path_parents::HeldParents,
    progress: &mut dyn FnMut(u64) -> io::Result<()>,
    before_replace: Option<&mut dyn FnMut() -> io::Result<bool>>,
) -> io::Result<CopiedFile> {
    let mut timing = super::copy_diagnostics::FileTiming::start(source, destination);
    let parents_timer = Timer::start(Phase::Parents);
    let held_parents = match source_parents {
        Some(_) => None,
        None => Some(super::path_parents::HeldParents::new(source)?),
    };
    let source = match &held_parents {
        Some(parents) => parents.resolved_path(source)?,
        None => source.to_path_buf(),
    };
    let destination = destination_parents.resolved_path(destination)?;
    drop(parents_timer);
    let checks_timer = Timer::start(Phase::Checks);
    progress(0)?;
    if replace {
        check_replacement(&destination)?;
    }
    drop(checks_timer);
    let written = write_output(&source, &destination, replace, held_source, &destination_parents, progress)?;
    let mut output = written.open_verified()?;
    let metadata = output.metadata()?;
    timing.bytes = metadata.len();
    let hash_timer = super::copy_diagnostics::Timer::start(super::copy_diagnostics::Phase::Hash);
    let hash = hash(&mut output, progress)?;
    drop(hash_timer);
    if let Err(error) = metadata
        .accessed()
        .and_then(|accessed| platform().restore_access_date(&output, accessed))
    {
        log::warn!("Access date not kept for {}: {error}", destination.display());
    }
    let checks_timer = Timer::start(Phase::Checks);
    if expected.is_some_and(|expected| expected != hash) {
        // Not kept: unexpected bytes are removed with the output.
        return Err(history_source_changed(&source));
    }
    let exclusive = if replace { if let Some(before_replace) = before_replace { before_replace()? } else { false } } else { false };
    if replace && !exclusive {
        check_replacement(&destination)?;
    }
    drop(checks_timer);
    let publish_timer = super::copy_diagnostics::Timer::start(super::copy_diagnostics::Phase::Publish);
    written.keep(&destination_parents, &output, replace && !exclusive)?;
    drop(publish_timer);
    timing.published = true;
    Ok(CopiedFile {
        file: output,
        hash,
        _parents: destination_parents,
    })
}

#[cfg(all(test, target_os = "windows"))]
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
    let metadata = match std::fs::symlink_metadata(path) {
        // Removed since the conflict was answered: there is nothing left to replace, and
        // the publication creates the file instead of ending the whole operation.
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        result => result?,
    };
    if !metadata.is_file()
        || metadata.file_type().is_symlink()
        || platform().file_attributes(&metadata, "").2
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            crate::models::error::COPY_TARGET_NOT_REPLACEABLE,
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
