//! /proc descriptor paths bind enumeration/opening to the selected directory.
use std::{
    fs::{File, OpenOptions},
    io,
    os::{fd::AsRawFd, unix::fs::OpenOptionsExt},
    path::{Path, PathBuf},
};
fn directory_path(directory: &File) -> PathBuf {
    PathBuf::from(format!("/proc/self/fd/{}", directory.as_raw_fd()))
}
pub(super) fn open(parent: &File, path: &Path, directory: Option<bool>) -> io::Result<File> {
    let leaf = path
        .file_name()
        .ok_or_else(|| io::Error::other("Source has no file name"))?;
    // O_NOFOLLOW; O_PATH for inspection, O_DIRECTORY for directory reading.
    let flags = 0x20000
        | match directory {
            None => 0x200000,
            Some(true) => 0x10000,
            Some(false) => 0,
        };
    let path = directory_path(parent).join(leaf);
    match OpenOptions::new().read(true).custom_flags(flags).open(&path) {
        // A folder that cannot be listed can still be moved by one rename; listing it
        // for a copy then fails as `cp` does.
        Err(error) if directory == Some(true) && error.kind() == io::ErrorKind::PermissionDenied => {
            OpenOptions::new().read(true).custom_flags(flags | 0x200000).open(path)
        }
        result => result,
    }
}
pub(super) fn enumerate(
    directory: &File,
    visitor: &mut dyn FnMut(std::ffi::OsString) -> io::Result<()>,
) -> io::Result<()> {
    for entry in std::fs::read_dir(directory_path(directory))? {
        visitor(entry?.file_name())?;
    }
    Ok(())
}
