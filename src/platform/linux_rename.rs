//! Rename without replacing anything, including on FUSE file systems such as
//! ntfs-3g that refuse `RENAME_NOREPLACE`.
use rustix::{
    fd::BorrowedFd,
    fs::{AtFlags, RenameFlags},
    io::Errno,
};
use std::{fs::File, io, path::Path};

/// Paths are relative to their parent descriptors, or absolute with `CWD`.
pub(super) fn exclusive(from_parent: BorrowedFd<'_>, from: &Path, parent: BorrowedFd<'_>, to: &Path) -> io::Result<()> {
    match rustix::fs::renameat_with(from_parent, from, parent, to, RenameFlags::NOREPLACE) {
        Err(Errno::INVAL) => {}
        result => return result.map_err(io::Error::from),
    }
    // The flag is refused: check the name is free, then rename. A name created in
    // between would be replaced, a window this file system gives no way to close.
    match rustix::fs::statat(parent, to, AtFlags::SYMLINK_NOFOLLOW) {
        Err(Errno::NOENT) => {}
        Ok(_) => return Err(io::ErrorKind::AlreadyExists.into()),
        Err(error) => return Err(error.into()),
    }
    rustix::fs::renameat(from_parent, from, parent, to).map_err(io::Error::from)
}

/// Whether a rename between these folders stays on one mount of one file system,
/// so that the kernel accepts it instead of refusing it with `EXDEV`. Btrfs
/// subvolumes share a mount but not a device, bind mounts the reverse.
pub(super) fn same_mount(first: &File, second: &File) -> io::Result<bool> {
    let place = |file: &File| -> io::Result<Option<(u64, u32, u32)>> {
        let status = rustix::fs::statx(file, "", AtFlags::EMPTY_PATH, rustix::fs::StatxFlags::MNT_ID)?;
        Ok((status.stx_mask & rustix::fs::StatxFlags::MNT_ID.bits() != 0)
            .then_some((status.stx_mnt_id, status.stx_dev_major, status.stx_dev_minor)))
    };
    Ok(match (place(first)?, place(second)?) {
        (Some(first), Some(second)) => first == second,
        _ => false,
    })
}

#[cfg(test)]
#[path = "linux_rename/tests/rename.rs"]
mod tests;
