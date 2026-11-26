//! Source children are opened relative to an already selected parent.
use std::{fs::File, io, path::Path};
use windows::Win32::{
    Storage::FileSystem::{
        FILE_READ_ATTRIBUTES, FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE,
    },
};
/// FILE_READ_DATA | FILE_READ_ATTRIBUTES | SYNCHRONIZE. GENERIC_READ would
/// also ask for READ_CONTROL and FILE_READ_EA, which company shares often
/// leave out of folders the user may still list and read (issue #13).
pub(super) const READ_CONTENTS: u32 = 0x1 | 0x80 | 0x0010_0000;

fn leaf(path: &Path) -> io::Result<&std::ffi::OsStr> {
    path.file_name()
        .ok_or_else(|| io::Error::other("Source has no file name"))
}
pub(super) fn track(parent: &File, path: &Path) -> io::Result<File> {
    super::windows_no_reparse::open_entry(
        parent,
        leaf(path)?,
        FILE_READ_ATTRIBUTES.0,
        FILE_SHARE_READ.0 | FILE_SHARE_WRITE.0 | FILE_SHARE_DELETE.0,
    )
}
pub(super) fn open(parent: &File, path: &Path, directory: bool) -> io::Result<File> {
    super::windows_no_reparse::open_relative(
        parent,
        leaf(path)?,
        directory,
        READ_CONTENTS,
        // Like Explorer, accept a writer that already holds the file (an open
        // Excel workbook); a deleter could still remove it during the copy.
        FILE_SHARE_READ.0 | FILE_SHARE_WRITE.0,
        1,
    )
}
