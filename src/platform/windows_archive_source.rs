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
    FILE_SHARE_READ, FILE_SHARE_WRITE, GETFINALPATHNAMEBYHANDLE_FLAGS,
};

fn io_error(error: windows::core::Error) -> io::Error {
    io::Error::from_raw_os_error((error.code().0 as u32 & 0xffff) as i32)
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

/// FAT32 and exFAT give their root directory the ID 0. A volume's root cannot
/// be renamed or replaced, so the volume itself identifies it.
fn root_identity(file: &File, volume: u64) -> io::Result<FileIdentity> {
    if super::windows_final_path::is_volume_root(file)? {
        return Ok(FileIdentity { volume, value: [0; 16], kind: 3 });
    }
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        crate::models::error::FILE_IDENTITY_UNAVAILABLE,
    ))
}

/// An object without a file ID. A network volume with neither IDs nor a serial (the WebDAV
/// redirector, through which SharePoint is mapped) is identified by its path, as Explorer
/// does: a file replaced under the same name is no longer told apart. Other volumes keep
/// refusing, except at their root.
fn zero_identity(file: &File, volume: u64) -> io::Result<FileIdentity> {
    if volume == 0 {
        use std::os::windows::ffi::OsStrExt;
        let path = super::windows_final_path::final_name(file, GETFINALPATHNAMEBYHANDLE_FLAGS(0));
        if let Some(identity) = path.ok().and_then(|path| path_identity(&path.encode_wide().collect::<Vec<_>>())) {
            return Ok(identity);
        }
    }
    root_identity(file, volume)
}

/// Kind 4: `volume` from the `\\?\UNC\server\share` prefix, `value` from the whole path in
/// its exact case (a WebDAV server may tell `a` and `A` apart). None outside a UNC path.
pub(super) fn path_identity(final_path: &[u16]) -> Option<FileIdentity> {
    let text = String::from_utf16(final_path).ok()?;
    let head = text.get(..8)?;
    if !head.eq_ignore_ascii_case(r"\\?\UNC\") {
        return None;
    }
    let mut parts = text[8..].splitn(3, '\\');
    let (server, share) = (parts.next()?, parts.next()?);
    if server.is_empty() || share.is_empty() {
        return None;
    }
    let share_root = format!(r"{server}\{share}").to_lowercase();
    let volume = blake3::hash(share_root.as_bytes());
    let value = blake3::hash(text.trim_end_matches('\\').as_bytes());
    Some(FileIdentity {
        volume: u64::from_le_bytes(volume.as_bytes()[..8].try_into().unwrap()),
        value: value.as_bytes()[..16].try_into().unwrap(),
        kind: 4,
    })
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
        Ok(()) if extended.FileId.Identifier == [0; 16] => {
            zero_identity(file, extended.VolumeSerialNumber)
        }
        Ok(()) => Ok(FileIdentity { volume: extended.VolumeSerialNumber, value: extended.FileId.Identifier, kind: 1 }),
        // Older filesystem drivers may provide only the legacy 64-bit identity.
        Err(error) if matches!(error.raw_os_error(), Some(1 | 50 | 87)) => {
            let mut basic = BY_HANDLE_FILE_INFORMATION::default();
            unsafe { GetFileInformationByHandle(handle, &mut basic) }.map_err(io_error)?;
            let id = (u64::from(basic.nFileIndexHigh) << 32) | u64::from(basic.nFileIndexLow);
            if id == 0 {
                return zero_identity(file, u64::from(basic.dwVolumeSerialNumber));
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
#[cfg(test)]
#[path = "windows_archive_source/tests/roots.rs"]
mod root_tests;
#[cfg(test)]
#[path = "windows_archive_source/tests/path_identity.rs"]
mod path_identity_tests;
