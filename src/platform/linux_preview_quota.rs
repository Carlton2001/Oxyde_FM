//! Bounded disk use of the preview cache. Its keys hold each source's date and size, so a
//! changed file leaves its old entry behind: before an entry is published, the least
//! recently written entries Oxyde recognizes go until the folder fits its budget. Other
//! files count but stay.
use super::preview_cache_names::{recognized, recognized_staging, LEASE};
use rustix::fs::{AtFlags, FileType, FlockOperation, Mode, OFlags};
use rustix::io::Errno;
use std::ffi::{OsStr, OsString};
use std::io;
use std::os::fd::{AsFd, OwnedFd};
use std::os::unix::ffi::OsStrExt;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[derive(Clone, Copy)]
pub(super) struct Policy {
    pub bytes: u64,
    pub entries: usize,
    pub entry_bytes: u64,
}

impl Default for Policy {
    /// As on Windows.
    fn default() -> Self {
        Self { bytes: 256 * 1024 * 1024, entries: 4096, entry_bytes: 4 * 1024 * 1024 }
    }
}

/// A staging file this old belongs to a writer that ended without publishing it.
const ABANDONED: Duration = Duration::from_secs(60 * 60);

struct Entry {
    name: OsString,
    bytes: u64,
    modified: (i64, i64),
    inode: u64,
    staging: bool,
}

fn entry(folder: &OwnedFd, name: &OsStr) -> rustix::io::Result<rustix::fs::Stat> {
    rustix::fs::statat(folder, name, AtFlags::SYMLINK_NOFOLLOW)
}

/// Regular files of the folder: their total size and count, and those Oxyde may evict.
fn scan(folder: &OwnedFd, destination: &OsStr, temporary: &OsStr) -> io::Result<(u64, usize, Vec<Entry>)> {
    let abandoned = SystemTime::now().checked_sub(ABANDONED).and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |time| time.as_secs() as i64);
    let mut listing = rustix::fs::Dir::read_from(folder)?;
    let (mut total, mut count, mut candidates) = (0u64, 0usize, Vec::new());
    while let Some(read) = listing.read() {
        let name = OsStr::from_bytes(read?.file_name().to_bytes()).to_os_string();
        if name == "." || name == ".." || name == LEASE || name == temporary || name == destination {
            continue;
        }
        let stat = match entry(folder, &name) {
            Ok(stat) => stat,
            Err(Errno::NOENT) => continue,
            Err(error) => return Err(error.into()),
        };
        if FileType::from_raw_mode(stat.st_mode) != FileType::RegularFile {
            continue;
        }
        let bytes = stat.st_size as u64;
        total = total.saturating_add(bytes);
        count += 1;
        let staging = recognized_staging(&name);
        if recognized(&name) || (staging && stat.st_mtime < abandoned) {
            candidates.push(Entry { name, bytes, modified: (stat.st_mtime, stat.st_mtime_nsec as i64), inode: stat.st_ino, staging });
        }
    }
    Ok((total, count, candidates))
}

/// Removes the entry unless it changed since the scan.
fn evict(folder: &OwnedFd, candidate: &Entry) -> bool {
    let unchanged = entry(folder, &candidate.name).is_ok_and(|stat| {
        (stat.st_ino, stat.st_size as u64, (stat.st_mtime, stat.st_mtime_nsec as i64))
            == (candidate.inode, candidate.bytes, candidate.modified)
    });
    unchanged && rustix::fs::unlinkat(folder, candidate.name.as_os_str(), AtFlags::empty()).is_ok()
}

/// Makes room for `incoming` bytes beside `destination`, whose previous version is
/// replaced. The lease returned keeps other writers out until the entry is published.
pub(super) fn admit(folder: &OwnedFd, destination: &OsStr, temporary: &OsStr, incoming: u64, policy: Policy) -> io::Result<OwnedFd> {
    if incoming > policy.entry_bytes || incoming > policy.bytes || policy.entries == 0 {
        return Err(io::Error::other("Preview cache entry exceeds its disk budget"));
    }
    let lease = rustix::fs::openat(folder, LEASE, OFlags::RDWR | OFlags::CREATE | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::from_raw_mode(0o600))?;
    rustix::fs::flock(lease.as_fd(), FlockOperation::NonBlockingLockExclusive).map_err(|error| match error {
        Errno::WOULDBLOCK => io::Error::new(io::ErrorKind::WouldBlock, "Preview cache is being maintained"),
        error => error.into(),
    })?;
    let (mut bytes, mut entries, mut candidates) = scan(folder, destination, temporary)?;
    let fits = |entries: usize, bytes: u64| entries < policy.entries && bytes.saturating_add(incoming) <= policy.bytes;
    if fits(entries, bytes) {
        return Ok(lease);
    }
    // Abandoned staging files first, then the oldest entries.
    candidates.sort_by(|a, b| b.staging.cmp(&a.staging).then(a.modified.cmp(&b.modified)).then(a.name.cmp(&b.name)));
    for candidate in candidates {
        if evict(folder, &candidate) {
            bytes -= candidate.bytes;
            entries -= 1;
            if fits(entries, bytes) {
                return Ok(lease);
            }
        }
    }
    Err(io::Error::other("Preview cache disk budget is occupied by other files"))
}

/// `admit` for a cache file about to replace `path` from its sibling `temporary`; nothing
/// for a name the quota does not own.
pub(super) fn admit_beside(path: &std::path::Path, temporary: &std::path::Path, incoming: u64) -> io::Result<Option<OwnedFd>> {
    let (Some(folder), Some(destination), Some(temporary)) = (path.parent(), path.file_name(), temporary.file_name()) else {
        return Err(io::Error::other("Cache has no folder"));
    };
    if !recognized(destination) {
        return Ok(None);
    }
    let folder = rustix::fs::open(folder, OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC, Mode::empty())?;
    admit(&folder, destination, temporary, incoming, Policy::default()).map(Some)
}

#[cfg(test)]
#[path = "linux_preview_quota/tests/quota.rs"]
mod tests;
