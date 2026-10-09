//! Source children are opened relative to an already selected parent.
use std::{fs::File, io, path::Path};
use windows::Win32::{
    Foundation::GENERIC_READ,
    Storage::FileSystem::{
        FILE_READ_ATTRIBUTES, FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE,
    },
};
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
        GENERIC_READ.0,
        FILE_SHARE_READ.0 | if directory { FILE_SHARE_WRITE.0 } else { 0 },
        1,
    )
}
