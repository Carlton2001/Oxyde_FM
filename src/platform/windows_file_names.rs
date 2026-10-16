//! Directory name rules used by transfer preflight, without creating probe files.
use super::FileNameRules;
use std::{
    ffi::{OsStr, OsString},
    fs::File,
    io,
    os::windows::{
        ffi::{OsStrExt, OsStringExt},
        io::AsRawHandle,
    },
};
use windows::Win32::{
    Foundation::HANDLE,
    Storage::FileSystem::{
        FileCaseSensitiveInfo, GetFileInformationByHandleEx, FILE_CASE_SENSITIVE_INFO,
    },
};

#[link(name = "ntdll")]
unsafe extern "system" {
    fn RtlUpcaseUnicodeChar(character: u16) -> u16;
}

pub(super) fn rules(directory: &File) -> io::Result<FileNameRules> {
    let mut info = FILE_CASE_SENSITIVE_INFO::default();
    let result = unsafe {
        GetFileInformationByHandleEx(
            HANDLE(directory.as_raw_handle()),
            FileCaseSensitiveInfo,
            (&mut info as *mut FILE_CASE_SENSITIVE_INFO).cast(),
            std::mem::size_of_val(&info) as u32,
        )
    };
    match result {
        Ok(()) if info.Flags & 1 != 0 => Ok(FileNameRules::sensitive()),
        Ok(()) => Ok(FileNameRules::windows_insensitive()),
        Err(error) => {
            let code = (error.code().0 as u32 & 0xffff) as i32;
            // Older Windows/filesystem drivers without this information class
            // use the ordinary case-insensitive Windows namespace.
            if matches!(code, 1 | 50 | 87) {
                Ok(FileNameRules::windows_insensitive())
            } else {
                Err(io::Error::from_raw_os_error(code))
            }
        }
    }
}

pub(super) fn fold(name: &OsStr) -> OsString {
    // Match native one-code-unit uppercase, including non-ASCII names. Unicode
    // String::to_uppercase may expand characters and merge distinct filenames.
    let wide: Vec<_> = name
        .encode_wide()
        .map(|unit| unsafe { RtlUpcaseUnicodeChar(unit) })
        .collect();
    OsString::from_wide(&wide)
}

#[cfg(test)]
#[path = "windows_file_names/tests/comparison.rs"]
mod tests;

#[cfg(test)]
pub(super) fn set_fixture_case_sensitive(path: &std::path::Path) -> io::Result<()> {
    use std::os::windows::fs::OpenOptionsExt;
    use windows::Win32::Storage::FileSystem::{
        SetFileInformationByHandle, FILE_FLAG_BACKUP_SEMANTICS, FILE_READ_ATTRIBUTES,
        FILE_WRITE_ATTRIBUTES,
    };
    let directory = std::fs::OpenOptions::new()
        .access_mode(FILE_READ_ATTRIBUTES.0 | FILE_WRITE_ATTRIBUTES.0)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS.0)
        .open(path)?;
    let info = FILE_CASE_SENSITIVE_INFO { Flags: 1 };
    unsafe {
        SetFileInformationByHandle(
            HANDLE(directory.as_raw_handle()),
            FileCaseSensitiveInfo,
            (&info as *const FILE_CASE_SENSITIVE_INFO).cast(),
            std::mem::size_of_val(&info) as u32,
        )
    }
    .map_err(|error| io::Error::from_raw_os_error((error.code().0 as u32 & 0xffff) as i32))
}
