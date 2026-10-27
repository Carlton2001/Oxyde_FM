//! Opening only what an operation needs. Holding a folder, renaming an item and
//! comparing identities need no read permission: a folder that cannot be listed, or
//! that is only traversable, is held by an `O_PATH` descriptor instead.
use std::{
    fs::{File, OpenOptions},
    io,
    os::unix::fs::OpenOptionsExt,
    path::Path,
};

const O_PATH: i32 = 0x200000;
const O_NOFOLLOW: i32 = 0x20000;
const O_DIRECTORY: i32 = 0x10000;

/// Readable when allowed, so that listing and attributes keep working; `O_PATH`
/// otherwise. `follow` lets an ancestor be a link, as the namespace resolves it.
pub(super) fn hold_directory(path: &Path, follow: bool) -> io::Result<File> {
    let flags = O_DIRECTORY | if follow { 0 } else { O_NOFOLLOW };
    match OpenOptions::new().read(true).custom_flags(flags).open(path) {
        Err(error) if error.kind() == io::ErrorKind::PermissionDenied => {
            OpenOptions::new().read(true).custom_flags(flags | O_PATH).open(path)
        }
        result => result,
    }
}

/// The file itself, never its link, without reading it.
pub(super) fn open_unread(path: &Path) -> io::Result<File> {
    OpenOptions::new().read(true).custom_flags(O_PATH | O_NOFOLLOW).open(path)
}
