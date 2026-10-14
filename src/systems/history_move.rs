//! Verified history moves; copy only when the filesystem reports a volume boundary.
use crate::platform::platform;
use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::Path;

#[cfg(test)]
thread_local! {
    pub(crate) static HASH_BYTES: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

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
        created: &mut dyn FnMut(&File) -> io::Result<()>,
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
        created: &mut dyn FnMut(&File) -> io::Result<()>,
    ) -> io::Result<u64> {
        platform().copy_file_with_progress(
            source,
            destination,
            Some(file),
            parent,
            created,
            &mut |_| Ok(()),
        )?;
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
        #[cfg(test)]
        HASH_BYTES.with(|bytes| bytes.set(bytes.get() + size as u64));
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
    move_bound_controlled(
        ops,
        source,
        destination,
        expected,
        identity,
        None,
        source_parents,
        destination_parents,
    )
}

pub(super) fn move_file_with_journal_identity(
    source: &Path,
    destination: &Path,
    expected: Option<&str>,
    identity: &crate::models::transaction::HistoryMoveIdentity,
    source_parents: &super::path_parents::HeldParents,
    destination_parents: &super::path_parents::HeldParents,
) -> io::Result<String> {
    let captured = identity
        .snapshot()
        .ok_or_else(|| move_identity_failure(source, false))?;
    move_bound_controlled(
        &NativeIo,
        source,
        destination,
        expected,
        Some(captured),
        Some(identity),
        source_parents,
        destination_parents,
    )
}

fn move_bound_controlled(
    ops: &impl MoveIo,
    source: &Path,
    destination: &Path,
    expected: Option<&str>,
    identity: Option<crate::models::transaction::HistoryFileIdentity>,
    journal_identity: Option<&crate::models::transaction::HistoryMoveIdentity>,
    source_parents: &super::path_parents::HeldParents,
    destination_parents: &super::path_parents::HeldParents,
) -> io::Result<String> {
    let source = source_parents.resolved_path(source)?;
    let destination = destination_parents.resolved_path(destination)?;
    let mut original = source_parents.open_file(&source)?;
    if let Some(identity) = identity {
        if !identity.matches(&original)? {
            return Err(if journal_identity.is_some() {
                move_identity_failure(&source, true)
            } else {
                super::file_copy::identity_changed(&source)
            });
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
    // Empty means the initial operation was a native move, without copied
    // content to verify. Identity was checked on the opened object above.
    let expected = expected.filter(|hash| !hash.is_empty());
    let native_attempted = journal_identity.is_some() && expected.is_none();
    if native_attempted {
        match ops.rename(&original, &source, &destination, destination_parents) {
            Ok(()) => {
                refresh_move_identity(journal_identity, &original, &destination);
                return Ok(String::new());
            }
            Err(error) if error.kind() == io::ErrorKind::CrossesDevices => {}
            Err(error) => return Err(error),
        }
    }
    let original_hash = hash(&mut original)?;
    if expected.is_some_and(|expected| expected != original_hash) {
        return Err(io::Error::other(format!(
            "Cannot move a file changed since the operation: {}",
            source.display()
        )));
    }
    if !native_attempted {
        match ops.rename(&original, &source, &destination, destination_parents) {
            Ok(()) => {
                refresh_move_identity(journal_identity, &original, &destination);
                return Ok(original_hash);
            }
            Err(error)
                if error.kind() == io::ErrorKind::CrossesDevices
                    && (identity.is_none() || journal_identity.is_some()) => {}
            Err(error) => return Err(error),
        }
    }

    let staging = super::file_copy::Staging::with_prefix(destination_parents, ".oxyde-history-")?;
    let copied = ops.copy(
        &original,
        &source,
        &staging.file,
        &staging.directory,
        &mut |file| staging.capture_output(file),
    )?;
    if copied != metadata.len() {
        return Err(io::Error::other("Incomplete history move copy"));
    }
    let mut output = staging.open_output()?;
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
    refresh_move_identity(journal_identity, &output, &destination);
    Ok(original_hash)
}

pub(super) fn move_identity_failure(path: &Path, changed: bool) -> io::Error {
    io::Error::other(crate::models::error::HistoryMoveFailure {
        issue: if changed {
            crate::models::error::HistoryMoveIssue::Changed
        } else {
            crate::models::error::HistoryMoveIssue::Unverified
        },
        detail: path.to_string_lossy().into_owned(),
    })
}

fn refresh_move_identity(
    identity: Option<&crate::models::transaction::HistoryMoveIdentity>,
    file: &File,
    path: &Path,
) {
    if let Some(identity) = identity {
        // Publication already succeeded: invalidate an unavailable proof and
        // preserve the completed effect for rollback, rather than report an
        // unrecorded failure after moving user data.
        if let Err(error) = identity.refresh_from_file(file) {
            log::warn!(
                "Move history identity unavailable for {}: {error}",
                path.display()
            );
        }
    }
}

#[cfg(test)]
#[path = "history_move/tests/tests.rs"]
mod tests;
