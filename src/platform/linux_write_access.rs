//! Folders whose permissions refuse the user's writes, as Nautilus greys its write
//! actions: the kernel answers for this user, ACLs and read-only mounts included.
use std::path::Path;

/// A path the kernel cannot judge (archive member, vanished folder) is left writable.
pub(super) fn denies_writes(path: &Path) -> bool {
    use rustix::io::Errno;
    matches!(rustix::fs::access(path, rustix::fs::Access::WRITE_OK), Err(Errno::ACCESS | Errno::ROFS))
}

/// The listed subfolders that refuse writes. Kernel pseudo file systems and network or
/// FUSE mounts, where each question costs a round trip, are not asked.
pub(super) fn denied_subfolders(folder: &Path, subfolders: Vec<String>) -> Vec<String> {
    if super::linux_bulk_expand::skipped_type(folder) {
        return Vec::new();
    }
    subfolders.into_iter().filter(|path| denies_writes(Path::new(path))).collect()
}

#[cfg(test)]
#[path = "linux_write_access/tests/access.rs"]
mod tests;
