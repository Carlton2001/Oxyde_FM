use super::platform;
use std::fs::File;
use std::io;
use std::path::Path;
use windows::Win32::Foundation::GENERIC_READ;
use windows::Win32::Storage::FileSystem::{DELETE, FILE_SHARE_READ};

pub(super) fn lock_file(path: &Path) -> io::Result<File> {
    let file = super::windows_no_reparse::open(
        path,
        false,
        GENERIC_READ.0 | windows::Win32::Foundation::GENERIC_WRITE.0,
        0,
        3,
    )
    .map_err(|error| {
        if matches!(error.raw_os_error(), Some(32 | 33)) {
            io::Error::new(io::ErrorKind::WouldBlock, error)
        } else {
            error
        }
    })?;
    let metadata = file.metadata()?;
    if !metadata.is_file() || platform().file_attributes(&metadata, "").2 {
        return Err(io::Error::other("Cache lease is not an ordinary file"));
    }
    Ok(file)
}

pub(super) fn open_cleanup(path: &Path, directory: bool) -> io::Result<File> {
    let file = super::windows_no_reparse::open(
        path,
        directory,
        GENERIC_READ.0 | DELETE.0,
        if directory { FILE_SHARE_READ.0 } else { 0 },
        1,
    )?;
    let metadata = file.metadata()?;
    if metadata.is_dir() != directory
        || platform().file_attributes(&metadata, "").2
        || metadata.permissions().readonly()
    {
        return Err(io::Error::other(
            "Cache cleanup refuses reparse points and readonly entries",
        ));
    }
    Ok(file)
}

pub(super) fn read_file(path: &Path) -> io::Result<File> {
    super::windows_no_reparse::open(path, false, GENERIC_READ.0, 7, 1)
}
