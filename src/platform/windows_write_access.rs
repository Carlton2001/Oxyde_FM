//! Folders whose permissions refuse the user's writes, as Nautilus greys its write
//! actions: Windows checks the process token, so an Oxyde run without elevation is refused
//! what an administrator's filtered token cannot do (System Volume Information, Windows,
//! Program Files), ACLs and write-protected media included.
use std::os::windows::fs::OpenOptionsExt;
use std::path::Path;
use windows::Win32::Foundation::{ERROR_ACCESS_DENIED, ERROR_WRITE_PROTECT};
use windows::Win32::Storage::FileSystem::{
    FILE_ACCESS_RIGHTS, FILE_ADD_FILE, FILE_ADD_SUBDIRECTORY, FILE_FLAG_BACKUP_SEMANTICS, FILE_SHARE_DELETE,
    FILE_SHARE_READ, FILE_SHARE_WRITE,
};

/// Nothing can be added, neither a file nor a folder: `C:\`, where users may only create
/// folders, stays writable. A path Windows cannot judge (shell folder, vanished folder,
/// empty drive) is left writable.
pub(super) fn denies_writes(path: &Path) -> bool {
    path.is_absolute() && [FILE_ADD_FILE, FILE_ADD_SUBDIRECTORY].into_iter().all(|access| refused(path, access))
}

/// Opening the folder for `access` alone asks Windows' access check, and keeps no handle.
fn refused(path: &Path, access: FILE_ACCESS_RIGHTS) -> bool {
    let opened = std::fs::OpenOptions::new()
        .access_mode(access.0)
        .share_mode((FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE).0)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS.0)
        .open(path);
    opened.is_err_and(|error| {
        error.raw_os_error().is_some_and(|code| [ERROR_ACCESS_DENIED.0, ERROR_WRITE_PROTECT.0].contains(&(code as u32)))
    })
}

/// The listed subfolders that refuse writes. A share is not asked: each question would
/// cost a round trip.
pub(super) fn denied_subfolders(folder: &Path, subfolders: Vec<String>) -> Vec<String> {
    if super::hardware::is_network_path(folder) {
        return Vec::new();
    }
    subfolders.into_iter().filter(|path| denies_writes(Path::new(path))).collect()
}

#[cfg(test)]
#[path = "windows_write_access/tests/access.rs"]
mod tests;
