//! One independent archive listing admission, bounded before the parser opens.
use crate::models::error::{ArchiveLimitFailure, ArchiveLimitKind, ArchiveListingFailure};
use crate::models::{CommandError, FileEntry};
use crate::utils::archive_listing_protocol;
use std::path::Path;
use std::process::Command;
use std::sync::Mutex;
use std::time::Duration;

static SLOT: Mutex<()> = Mutex::new(());
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
