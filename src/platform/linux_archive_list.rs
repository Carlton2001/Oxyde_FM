//! Parent side of archive navigation and search indexes: parsers run in a confined
//! worker, which sees only the archive. Its reply is untrusted metadata.
use super::linux_sandbox::Profile;
use super::linux_worker::Worker;
use crate::models::error::{ArchiveLimitFailure, ArchiveLimitKind, ArchiveListingFailure};
use crate::models::{CommandError, FileEntry};
use crate::utils::archive_listing_protocol;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, TryLockError};
use std::time::{Duration, Instant};

const MEMORY_BYTES: u64 = 512 * 1024 * 1024;
const DEADLINE: Duration = Duration::from_secs(120);
const LIST_FLAG: &str = "--oxyde-archive-list-worker-v1";
const INDEX_FLAG: &str = "--oxyde-archive-index-worker-v1";
// Independent admissions: a long search never delays navigation.
static SLOT: Mutex<()> = Mutex::new(());
static SEARCH_SLOT: Mutex<()> = Mutex::new(());

fn failed() -> CommandError {
    CommandError::ArchiveError(ArchiveListingFailure::WorkerFailed.to_string())
}

fn executable() -> Result<PathBuf, CommandError> {
    Ok(std::env::current_exe().map_err(|_| failed())?.with_file_name("oxyde-archive-list-worker"))
}

pub(super) fn list(path: &Path, internal: &str) -> Result<Vec<FileEntry>, CommandError> {
    let _slot = SLOT.lock().map_err(|_| failed())?;
    list_in(&executable()?, path, internal, DEADLINE)
}

pub(super) fn index(path: &Path, checkpoint: &dyn Fn() -> io::Result<()>) -> Result<Vec<FileEntry>, CommandError> {
    let _slot = loop {
        checkpoint()?;
        match SEARCH_SLOT.try_lock() {
            Ok(slot) => break slot,
            Err(TryLockError::Poisoned(_)) => return Err(failed()),
            Err(TryLockError::WouldBlock) => std::thread::sleep(Duration::from_millis(10)),
        }
    };
    index_in(&executable()?, path, DEADLINE, checkpoint)
}

fn list_in(executable: &Path, path: &Path, internal: &str, deadline: Duration) -> Result<Vec<FileEntry>, CommandError> {
    let bytes = confined(executable, LIST_FLAG, Some(internal), path, deadline, &|| Ok(()))?;
    archive_listing_protocol::decode(&bytes, path, internal)
}

fn index_in(executable: &Path, path: &Path, deadline: Duration, checkpoint: &dyn Fn() -> io::Result<()>) -> Result<Vec<FileEntry>, CommandError> {
    let bytes = confined(executable, INDEX_FLAG, None, path, deadline, checkpoint)?;
    archive_listing_protocol::decode_index(&bytes, path, checkpoint)
}

fn confined(executable: &Path, flag: &str, internal: Option<&str>, path: &Path, deadline: Duration, checkpoint: &dyn Fn() -> io::Result<()>) -> Result<Vec<u8>, CommandError> {
    checkpoint()?;
    let reply = Worker::launch(executable, Profile::Codec, Some(path), MEMORY_BYTES, |command, source| {
        command.arg(flag);
        command.args(source);
        command.args(internal);
    }).and_then(|mut worker| worker.read_to_end_checked(archive_listing_protocol::MAX_REPLY, Instant::now() + deadline, checkpoint));
    reply.map_err(|error| {
        log::warn!("Confined archive metadata worker failed: {error}");
        match error.kind() {
            io::ErrorKind::Interrupted => CommandError::IoError(error.to_string()),
            io::ErrorKind::TimedOut => CommandError::ArchiveError(
                ArchiveLimitFailure { kind: ArchiveLimitKind::Seconds, limit: deadline.as_secs() }.to_string(),
            ),
            io::ErrorKind::FileTooLarge => CommandError::ArchiveError(ArchiveListingFailure::MetadataLimit.to_string()),
            io::ErrorKind::NotFound | io::ErrorKind::PermissionDenied => CommandError::IoError(error.to_string()),
            _ => failed(),
        }
    })
}

#[cfg(test)]
#[path = "linux_archive_list/tests/workers.rs"]
mod tests;
