//! Rename without replacing anything, including on FUSE file systems such as
//! ntfs-3g that refuse `RENAME_NOREPLACE`.
use rustix::{
    fd::BorrowedFd,
    fs::{AtFlags, RenameFlags},
    io::Errno,
};
use std::{io, path::Path};

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

#[cfg(test)]
#[path = "linux_rename/tests/rename.rs"]
mod tests;
