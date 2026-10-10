//! Independent navigation/search admissions, bounded before archive parsing.
use crate::models::error::{ArchiveLimitFailure, ArchiveLimitKind, ArchiveListingFailure};
use crate::models::{CommandError, FileEntry};
use crate::utils::archive_listing_protocol;
use std::path::Path;
use std::process::Command;
use std::sync::{Mutex, MutexGuard, TryLockError};
use std::time::Duration;

static SLOT: Mutex<()> = Mutex::new(());
static SEARCH_SLOT: Mutex<()> = Mutex::new(());
const MEMORY_BYTES: usize = 512 * 1024 * 1024;
const DEADLINE: Duration = Duration::from_secs(120);
fn failed() -> CommandError {
    CommandError::ArchiveError(ArchiveListingFailure::WorkerFailed.to_string())
}

pub(super) fn run(path: &Path, internal: &str) -> Result<Vec<FileEntry>, CommandError> {
    let _slot = SLOT.lock().map_err(|_| failed())?;
    let executable = std::env::current_exe().map_err(|_| failed())?;
    run_in(&executable, path, internal, DEADLINE)
}
pub(super) fn run_in(
    executable: &Path,
    path: &Path,
    internal: &str,
    deadline: Duration,
) -> Result<Vec<FileEntry>, CommandError> {
    let mut command = Command::new(executable);
    command
        .arg("--oxyde-archive-list-worker-v1")
        .arg(path)
        .arg(internal);
    supervise(command, path, internal, deadline)
}
pub(super) fn supervise(
    command: Command,
    path: &Path,
    internal: &str,
    deadline: Duration,
) -> Result<Vec<FileEntry>, CommandError> {
    let bytes = super::windows_preview_worker::supervise_bounded(
        command,
        deadline,
        MEMORY_BYTES,
        archive_listing_protocol::MAX_REPLY,
        "Archive listing timed out",
    )
    .map_err(|error| {
        log::warn!("Archive listing worker failed: {error}");
        match error.kind() {
            std::io::ErrorKind::TimedOut => CommandError::ArchiveError(
                ArchiveLimitFailure {
                    kind: ArchiveLimitKind::Seconds,
                    limit: deadline.as_secs(),
                }
                .to_string(),
            ),
            std::io::ErrorKind::FileTooLarge => {
                CommandError::ArchiveError(ArchiveListingFailure::MetadataLimit.to_string())
            }
            _ => failed(),
        }
    })?;
    archive_listing_protocol::decode(&bytes, path, internal)
}

fn search_admission(
    checkpoint: &dyn Fn() -> std::io::Result<()>,
) -> Result<MutexGuard<'static, ()>, CommandError> {
    loop {
        checkpoint()?;
        match SEARCH_SLOT.try_lock() {
            Ok(slot) => return Ok(slot),
            Err(TryLockError::Poisoned(_)) => return Err(failed()),
            Err(TryLockError::WouldBlock) => std::thread::sleep(Duration::from_millis(10)),
        }
    }
}
pub(super) fn index(
    path: &Path,
    checkpoint: &dyn Fn() -> std::io::Result<()>,
) -> Result<Vec<FileEntry>, CommandError> {
    let _slot = search_admission(checkpoint)?;
    let executable = std::env::current_exe().map_err(|_| failed())?;
    index_in(&executable, path, DEADLINE, checkpoint)
}
pub(super) fn index_in(
    executable: &Path,
    path: &Path,
    deadline: Duration,
    checkpoint: &dyn Fn() -> std::io::Result<()>,
) -> Result<Vec<FileEntry>, CommandError> {
    let mut command = Command::new(executable);
    command.arg("--oxyde-archive-index-worker-v1").arg(path);
    supervise_index(command, path, deadline, checkpoint)
}
pub(super) fn supervise_index(
    command: Command,
    path: &Path,
    deadline: Duration,
    checkpoint: &dyn Fn() -> std::io::Result<()>,
) -> Result<Vec<FileEntry>, CommandError> {
    let bytes = super::windows_preview_worker::supervise_checked(
        command,
        deadline,
        MEMORY_BYTES,
        archive_listing_protocol::MAX_REPLY,
        "Archive search index timed out",
        checkpoint,
    )
    .map_err(|error| {
        if error.kind() == std::io::ErrorKind::Interrupted {
            return CommandError::IoError(error.to_string());
        }
        log::warn!("Archive search worker failed: {error}");
        match error.kind() {
            std::io::ErrorKind::TimedOut => CommandError::ArchiveError(
                ArchiveLimitFailure {
                    kind: ArchiveLimitKind::Seconds,
                    limit: deadline.as_secs(),
                }
                .to_string(),
            ),
            std::io::ErrorKind::FileTooLarge => {
                CommandError::ArchiveError(ArchiveListingFailure::MetadataLimit.to_string())
            }
            _ => failed(),
        }
    })?;
    archive_listing_protocol::decode_index(&bytes, path, checkpoint)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cancellation_interrupts_search_admission_while_navigation_slot_stays_available() {
        let slot = SEARCH_SLOT.lock().unwrap();
        assert!(SLOT.try_lock().is_ok());
        let calls = std::cell::Cell::new(0);
        let checkpoint = || {
            calls.set(calls.get() + 1);
            if calls.get() > 2 {
                Err(std::io::Error::new(
                    std::io::ErrorKind::Interrupted,
                    "cancelled",
                ))
            } else {
                Ok(())
            }
        };
        assert!(search_admission(&checkpoint).is_err());
        assert_eq!(calls.get(), 3);
        drop(slot);
        assert!(search_admission(&|| Ok(())).is_ok());
    }
}
