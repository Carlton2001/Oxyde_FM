use std::fs::{File, OpenOptions};
use std::io::{Error, ErrorKind, Result};
use std::os::windows::ffi::OsStrExt;
use std::os::windows::fs::OpenOptionsExt;
use std::os::windows::io::AsRawHandle;
use std::path::Path;
use windows::Win32::Foundation::{GENERIC_READ, HANDLE};
use windows::Win32::Storage::FileSystem::{
    FileDispositionInfo, FileRenameInfo, SetFileInformationByHandle, DELETE, FILE_DISPOSITION_INFO,
    FILE_FLAG_OPEN_REPARSE_POINT, FILE_RENAME_INFO, FILE_SHARE_READ,
};

fn io_error(error: windows::core::Error) -> Error {
    // These APIs report HRESULT_FROM_WIN32(GetLastError()).
    Error::from_raw_os_error((error.code().0 as u32 & 0xffff) as i32)
}

pub(super) fn open(path: &Path) -> Result<File> {
    OpenOptions::new()
        .access_mode(GENERIC_READ.0 | DELETE.0)
        .share_mode(FILE_SHARE_READ.0)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT.0)
        .open(path)
}

pub(super) fn rename(file: &File, destination: &Path) -> Result<()> {
    if !destination.is_absolute() {
        return Err(Error::new(
            ErrorKind::InvalidInput,
            "Move destination must be absolute",
        ));
    }
    let name: Vec<u16> = destination.as_os_str().encode_wide().collect();
    if name.contains(&0) {
        return Err(Error::new(
            ErrorKind::InvalidInput,
            "Move destination contains a null character",
        ));
    }
    let name_bytes = name
        .len()
        .checked_mul(2)
        .and_then(|size| u32::try_from(size).ok())
        .ok_or_else(|| Error::new(ErrorKind::InvalidInput, "Move destination is too long"))?;
    let offset = std::mem::offset_of!(FILE_RENAME_INFO, FileName);
    let size = (offset + name_bytes as usize + 2).max(std::mem::size_of::<FILE_RENAME_INFO>());
    let size_u32 = u32::try_from(size)
        .map_err(|_| Error::new(ErrorKind::InvalidInput, "Move destination is too long"))?;
    // Pointer-sized storage provides FILE_RENAME_INFO alignment, including its
    // variable-length UTF-16 tail. The extra terminator remains zero-filled.
    let mut storage = vec![0usize; size.div_ceil(std::mem::size_of::<usize>())];
    unsafe {
        let info = storage.as_mut_ptr().cast::<FILE_RENAME_INFO>();
        (*info).Anonymous.ReplaceIfExists = false;
        (*info).RootDirectory = HANDLE::default();
        (*info).FileNameLength = name_bytes;
        std::ptr::copy_nonoverlapping(
            name.as_ptr(),
            storage.as_mut_ptr().cast::<u8>().add(offset).cast::<u16>(),
            name.len(),
        );
        SetFileInformationByHandle(
            HANDLE(file.as_raw_handle()),
            FileRenameInfo,
            info.cast(),
            size_u32,
        )
        .map_err(io_error)
    }
}

pub(super) fn remove(file: &File) -> Result<()> {
    let info = FILE_DISPOSITION_INFO { DeleteFile: true };
    unsafe {
        SetFileInformationByHandle(
            HANDLE(file.as_raw_handle()),
            FileDispositionInfo,
            (&info as *const FILE_DISPOSITION_INFO).cast(),
            std::mem::size_of_val(&info) as u32,
        )
        .map_err(io_error)
    }
}
