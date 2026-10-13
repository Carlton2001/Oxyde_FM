//! Opened archive identities and sharing policies, kept behind Platform.
use super::FileIdentity;
use std::fs::{File, OpenOptions};
use std::io;
use std::os::windows::{fs::OpenOptionsExt, io::AsRawHandle};
use std::path::Path;
use windows::Win32::Foundation::HANDLE;
use windows::Win32::Storage::FileSystem::{
    FileIdInfo, GetFileInformationByHandle, GetFileInformationByHandleEx,
    BY_HANDLE_FILE_INFORMATION, FILE_FLAG_OPEN_REPARSE_POINT, FILE_ID_INFO, FILE_SHARE_DELETE,
    FILE_SHARE_READ, FILE_SHARE_WRITE,
};

fn io_error(error: windows::core::Error) -> io::Error {
    io::Error::from_raw_os_error((error.code().0 as u32 & 0xffff) as i32)
}

pub(super) fn path(file: &File) -> io::Result<std::path::PathBuf> {
    use std::os::windows::ffi::OsStringExt;
    use windows::Win32::Storage::FileSystem::{GetFinalPathNameByHandleW, GETFINALPATHNAMEBYHANDLE_FLAGS};
    let mut buffer = vec![0u16; 32768];
    let count = unsafe { GetFinalPathNameByHandleW(HANDLE(file.as_raw_handle()), &mut buffer, GETFINALPATHNAMEBYHANDLE_FLAGS(0)) };
    if count == 0 { return Err(io::Error::last_os_error()); }
    if count as usize >= buffer.len() { return Err(io::Error::other("Retained path exceeds native limit")); }
    Ok(std::ffi::OsString::from_wide(&buffer[..count as usize]).into())
}

pub(super) fn open(path: &Path, allow_changes: bool) -> io::Result<File> {
    let share = if allow_changes {
        FILE_SHARE_READ.0 | FILE_SHARE_WRITE.0 | FILE_SHARE_DELETE.0
    } else {
        FILE_SHARE_READ.0
    };
    let file = OpenOptions::new()
        .read(true)
        .share_mode(share)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT.0)
        .open(path)?;
    let metadata = file.metadata()?;
    if !metadata.is_file() || super::platform().file_attributes(&metadata, "").2 {
        return Err(io::Error::other("Archive source is not an ordinary file"));
    }
    Ok(file)
}

pub(super) fn identity(file: &File) -> io::Result<FileIdentity> {
    let handle = HANDLE(file.as_raw_handle());
    let mut extended = FILE_ID_INFO::default();
    let result = unsafe {
        GetFileInformationByHandleEx(
            handle,
            FileIdInfo,
            (&mut extended as *mut FILE_ID_INFO).cast(),
            std::mem::size_of_val(&extended) as u32,
        )
    };
    match result.map_err(io_error) {
        Ok(()) if extended.FileId.Identifier == [0; 16] => Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "Filesystem did not provide a stable file identity",
        )),
        Ok(()) => Ok(FileIdentity { volume: extended.VolumeSerialNumber, value: extended.FileId.Identifier, kind: 1 }),
        // Older filesystem drivers may provide only the legacy 64-bit identity.
        Err(error) if matches!(error.raw_os_error(), Some(1 | 50 | 87)) => {
            let mut basic = BY_HANDLE_FILE_INFORMATION::default();
            unsafe { GetFileInformationByHandle(handle, &mut basic) }.map_err(io_error)?;
            let id = (u64::from(basic.nFileIndexHigh) << 32) | u64::from(basic.nFileIndexLow);
            if id == 0 {
                return Err(io::Error::new(
                    io::ErrorKind::Unsupported,
                    "Filesystem did not provide a stable file identity",
                ));
            }
            let mut value = [0u8; 16];
            value[..8].copy_from_slice(&id.to_le_bytes());
            Ok(FileIdentity {
                volume: u64::from(basic.dwVolumeSerialNumber),
                value,
                kind: 2,
            })
        }
        Err(error) => Err(error),
    }
}

#[cfg(test)]
#[path = "windows_archive_source/tests/retention.rs"]
mod tests;
