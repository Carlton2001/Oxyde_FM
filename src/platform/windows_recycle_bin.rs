//! Whether Windows can recycle on a volume, known before asking the user.
//! The Shell still checks each item: this only chooses the question.
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::path::{Path, PathBuf};
use windows::core::PCWSTR;
use windows::Win32::Storage::FileSystem::{GetDriveTypeW, GetVolumePathNameW};

const DRIVE_UNKNOWN: u32 = 0;
const DRIVE_NO_ROOT_DIR: u32 = 1;
const DRIVE_FIXED: u32 = 3;

pub(super) fn available(path: &Path) -> bool {
    let Some(root) = volume_root(path) else { return true };
    // A SUBST drive reports the type of the drive it maps, but has no Recycle Bin.
    if let Some(letter) = drive_letter(&root) {
        if super::windows_final_path::dos_device(letter).is_some_and(|device| device.starts_with(r"\??\")) {
            return false;
        }
    }
    let wide: Vec<u16> = root.as_os_str().encode_wide().chain(Some(0)).collect();
    recycles(unsafe { GetDriveTypeW(PCWSTR(wide.as_ptr())) })
}

/// Shares, removable, optical and RAM drives have no Recycle Bin. An unknown
/// type leaves the decision to the Shell.
fn recycles(drive_type: u32) -> bool {
    matches!(drive_type, DRIVE_FIXED | DRIVE_UNKNOWN | DRIVE_NO_ROOT_DIR)
}

/// `C:\`, `\\server\share\` or a mount folder, as Windows resolves it.
fn volume_root(path: &Path) -> Option<PathBuf> {
    let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    let mut buffer = vec![0u16; wide.len() + 2];
    unsafe { GetVolumePathNameW(PCWSTR(wide.as_ptr()), &mut buffer) }.ok()?;
    let length = buffer.iter().position(|c| *c == 0)?;
    Some(std::ffi::OsString::from_wide(&buffer[..length]).into())
}

fn drive_letter(root: &Path) -> Option<char> {
    let text = root.to_str()?;
    let text = text.strip_prefix(r"\\?\").unwrap_or(text);
    let bytes = text.as_bytes();
    (bytes.len() <= 3 && bytes.get(1) == Some(&b':') && bytes[0].is_ascii_alphabetic()).then(|| bytes[0] as char)
}

#[cfg(test)]
#[path = "windows_recycle_bin/tests/volumes.rs"]
mod tests;
