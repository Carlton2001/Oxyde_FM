//! The root listing of an archive probed for a password before it is opened,
//! kept for the navigation that follows: the archive is not listed twice.
use crate::models::{CommandError, FileEntry, FileStamp};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

struct Probed {
    archive: PathBuf,
    stamp: FileStamp,
    at: Instant,
    entries: Vec<FileEntry>,
}

/// One archive at a time: only the last probe precedes a navigation.
static PROBED: Mutex<Option<Probed>> = Mutex::new(None);
/// A navigation follows its probe at once; later, the archive is listed again.
const FRESH: Duration = Duration::from_secs(10);

fn stamp(archive: &Path) -> Option<FileStamp> {
    FileStamp::from_metadata(&std::fs::metadata(archive).ok()?)
}

fn slot() -> std::sync::MutexGuard<'static, Option<Probed>> {
    PROBED.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Lists the root of `archive`, keeping the listing for [`take`].
pub fn probe(archive: String) -> Result<(), CommandError> {
    let path = PathBuf::from(&archive);
    // Taken before the listing: an archive changed during it is listed again.
    let before = stamp(&path);
    let entries = super::archive::list_archive_contents(archive, String::new())?;
    *slot() = before.map(|stamp| Probed { archive: path, stamp, at: Instant::now(), entries });
    Ok(())
}

/// Drops a kept listing whose navigation did not happen.
pub fn forget() {
    slot().take();
}

/// The listing kept for the root of `archive`, while it is fresh and the
/// archive unchanged.
pub(crate) fn take(archive: &Path) -> Option<Vec<FileEntry>> {
    let mut slot = slot();
    if slot.as_ref()?.archive != archive {
        return None;
    }
    let probed = slot.take()?;
    drop(slot);
    (probed.at.elapsed() < FRESH && stamp(archive).as_ref() == Some(&probed.stamp))
        .then_some(probed.entries)
}

#[cfg(test)]
#[path = "archive_probe/tests/handoff.rs"]
mod tests;
