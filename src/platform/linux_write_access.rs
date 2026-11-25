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

/// The entries of a sticky folder (`/tmp`) the user can neither rename nor remove: the
/// kernel lets only their owner, the folder's owner or root do it, as GIO's `can-delete`.
pub(super) fn undeletable_entries(folder: &Path) -> Vec<String> {
    use std::os::unix::fs::MetadataExt;
    let euid = rustix::process::geteuid().as_raw();
    let Ok(metadata) = std::fs::metadata(folder) else {
        return Vec::new();
    };
    if !sticky_guards(metadata.mode(), metadata.uid(), euid) || super::linux_bulk_expand::skipped_type(folder) {
        return Vec::new();
    }
    let Ok(entries) = std::fs::read_dir(folder) else {
        return Vec::new();
    };
    entries
        .flatten()
        .filter(|entry| entry.metadata().is_ok_and(|metadata| metadata.uid() != euid))
        .map(|entry| crate::utils::path_text::encode(&entry.path()))
        .collect()
}

/// Whether a folder of this mode and owner keeps the user from removing others' entries.
fn sticky_guards(mode: u32, owner: u32, euid: u32) -> bool {
    mode & 0o1000 != 0 && euid != 0 && owner != euid
}

#[cfg(test)]
#[path = "linux_write_access/tests/access.rs"]
mod tests;
