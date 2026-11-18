//! Where an item sits: its file system, by device, and its mount, from Linux 5.8. A bind
//! mount shows a folder of the same file system, so only its mount tells it apart.
use rustix::fs::{AtFlags, FileType, StatxFlags};
use std::os::fd::AsFd;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Place {
    device: (u32, u32),
    /// Unknown before Linux 5.8.
    mount: Option<u64>,
}

impl Place {
    /// Whether `inner` is on this file system and, when both are known, this mount: a walk
    /// that stays inside never reaches another file system nor a bind mount.
    pub(super) fn holds(self, inner: Place) -> bool {
        self.device == inner.device && match (self.mount, inner.mount) {
            (Some(own), Some(other)) => own == other,
            _ => true,
        }
    }

    /// Whether an item at this place, inside a folder at `parent`, is mounted there. Btrfs
    /// subvolumes share a mount but not a device: the device decides only without mounts.
    pub(super) fn mounted_on(self, parent: Place) -> bool {
        match (self.mount, parent.mount) {
            (Some(own), Some(other)) => own != other,
            _ => self.device != parent.device,
        }
    }
}

pub(super) struct Status {
    pub(super) kind: FileType,
    /// Type and permission bits.
    pub(super) mode: u32,
    pub(super) inode: u64,
    pub(super) place: Place,
}

/// `name` in `folder`, not followed if it is a link.
pub(super) fn status(folder: impl AsFd, name: impl rustix::path::Arg) -> rustix::io::Result<Status> {
    read(folder, name, AtFlags::SYMLINK_NOFOLLOW)
}

/// The item `item` holds, which may be opened with `O_PATH`.
pub(super) fn status_of(item: impl AsFd) -> rustix::io::Result<Status> {
    read(item, "", AtFlags::EMPTY_PATH)
}

fn read(folder: impl AsFd, name: impl rustix::path::Arg, flags: AtFlags) -> rustix::io::Result<Status> {
    let found = rustix::fs::statx(folder, name, flags, StatxFlags::BASIC_STATS | StatxFlags::MNT_ID)?;
    let mode = u32::from(found.stx_mode);
    Ok(Status {
        kind: FileType::from_raw_mode(mode),
        mode,
        inode: found.stx_ino,
        place: Place {
            device: (found.stx_dev_major, found.stx_dev_minor),
            mount: (found.stx_mask & StatxFlags::MNT_ID.bits() != 0).then_some(found.stx_mnt_id),
        },
    })
}

#[cfg(test)]
#[path = "linux_mount_place/tests/place.rs"]
mod tests;
