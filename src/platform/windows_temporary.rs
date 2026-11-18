//! Temporary folder for staging that needs access rights and file identities.
use std::fs::{File, OpenOptions};
use std::io;
use std::os::windows::fs::OpenOptionsExt;
use std::os::windows::io::AsRawHandle;
use std::path::{Path, PathBuf};
use windows::Win32::Foundation::HANDLE;
use windows::Win32::Storage::FileSystem::GetVolumeInformationByHandleW;

const FILE_PERSISTENT_ACLS: u32 = 0x0000_0008;

/// The configured temporary folder, or the system's when portable mode puts it
/// on a volume without access rights (FAT32, exFAT). The worker sandbox and the
/// archive cache protect their files with a DACL and identify them by file ID:
/// there they cannot work at all. The system folder holds transient files only.
pub(super) fn protected_dir() -> io::Result<PathBuf> {
    let configured = crate::app_paths::temporary_dir()?;
    if keeps_access_rights(&configured)? {
        return Ok(configured);
    }
    let system = crate::app_paths::system_temporary_dir()?;
    if system != configured && keeps_access_rights(&system)? {
        log::info!(
            "Temporary folder {} keeps no access rights; staging in {}",
            configured.display(),
            system.display()
        );
        return Ok(system);
    }
    Err(io::Error::other(
        "The temporary folder is on a volume without access rights",
    ))
}

fn keeps_access_rights(path: &Path) -> io::Result<bool> {
    Ok(volume_flags(&open(path)?)? & FILE_PERSISTENT_ACLS != 0)
}

fn open(path: &Path) -> io::Result<File> {
    OpenOptions::new()
        .access_mode(0x80) // FILE_READ_ATTRIBUTES
        .share_mode(7)
        .custom_flags(0x0200_0000) // FILE_FLAG_BACKUP_SEMANTICS
        .open(path)
}

fn volume_flags(file: &File) -> io::Result<u32> {
    let mut flags = 0;
    unsafe {
        GetVolumeInformationByHandleW(
            HANDLE(file.as_raw_handle()),
            None,
            None,
            None,
            Some(&mut flags),
            None,
        )
    }
    .map_err(io::Error::from)?;
    Ok(flags)
}

#[cfg(test)]
#[path = "windows_temporary/tests/volumes.rs"]
mod tests;
