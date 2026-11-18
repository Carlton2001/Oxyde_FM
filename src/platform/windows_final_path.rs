//! Final paths of opened objects, including volumes the mount manager does
//! not know (ImDisk RAM disks, some virtual or encrypted volumes, redirectors).
use std::ffi::OsString;
use std::fs::{File, OpenOptions};
use std::io;
use std::os::windows::{
    ffi::{OsStrExt, OsStringExt},
    fs::OpenOptionsExt,
    io::AsRawHandle,
};
use std::path::{Component, Path, PathBuf, Prefix};
use windows::core::PCWSTR;
use windows::Win32::Foundation::HANDLE;
use windows::Win32::Storage::FileSystem::{
    GetFinalPathNameByHandleW, QueryDosDeviceW, FILE_FLAG_BACKUP_SEMANTICS,
    GETFINALPATHNAMEBYHANDLE_FLAGS, VOLUME_NAME_NT,
};

/// `std::fs::canonicalize`, with the fallback of `of_handle`.
pub(super) fn canonicalize(path: &Path) -> io::Result<PathBuf> {
    // The same open as the standard library: no access, all sharing, links followed.
    let file = OpenOptions::new()
        .access_mode(0)
        .share_mode(7)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS.0)
        .open(path)?;
    of_handle(&file, path)
}

/// The `\\?\` DOS path of an opened object. Without a DOS name from the
/// mount manager, the native name is translated through the drive or share
/// the caller named, and kept only if it opens the same object.
pub(super) fn of_handle(file: &File, named: &Path) -> io::Result<PathBuf> {
    match final_name(file, GETFINALPATHNAMEBYHANDLE_FLAGS(0)) {
        Ok(path) => Ok(with_root_separator(path.into())),
        Err(error) => translated(file, named).map(with_root_separator).ok_or(error),
    }
}

/// Windows names a share root `\\?\UNC\server\share`, without the separator
/// it gives a drive root (`\\?\C:\`). Without it the path has no root
/// component, and a child joined to it no longer starts with it.
fn with_root_separator(path: PathBuf) -> PathBuf {
    let mut components = path.components();
    let share_root = matches!(
        components.next(),
        Some(Component::Prefix(prefix)) if matches!(prefix.kind(), Prefix::VerbatimUNC(..) | Prefix::UNC(..))
    ) && components.next().is_none();
    if !share_root {
        return path;
    }
    let mut text = path.into_os_string();
    text.push("\\");
    text.into()
}

fn translated(file: &File, named: &Path) -> Option<PathBuf> {
    let native = final_name(file, VOLUME_NAME_NT).ok()?;
    let absolute = std::path::absolute(named).ok()?;
    let drive = match absolute.components().next()? {
        Component::Prefix(prefix) => match prefix.kind() {
            Prefix::Disk(letter) | Prefix::VerbatimDisk(letter) => Some(letter),
            _ => None,
        },
        _ => None,
    };
    let device = drive.and_then(|letter| dos_device(letter as char));
    let candidate = dos_form(
        &native.to_string_lossy(),
        drive.zip(device.as_deref()).map(|(letter, device)| (letter as char, device)),
    )?;
    let candidate = PathBuf::from(candidate);
    let reopened = OpenOptions::new()
        .access_mode(0)
        .share_mode(7)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS.0)
        .open(&candidate)
        .ok()?;
    let same = super::windows_archive_source::identity(file).ok()?
        == super::windows_archive_source::identity(&reopened).ok()?;
    same.then_some(candidate)
}

/// `\Device\Mup\server\share\…` as `\\?\UNC\server\share\…`, and a path on
/// the named drive's device as `\\?\X:\…`.
fn dos_form(native: &str, drive: Option<(char, &str)>) -> Option<String> {
    let below = |device: &str| {
        let head = native.get(..device.len())?;
        let rest = &native[device.len()..];
        (head.eq_ignore_ascii_case(device) && (rest.is_empty() || rest.starts_with('\\'))).then_some(rest)
    };
    if let Some(rest) = below(r"\Device\Mup").filter(|rest| !rest.is_empty()) {
        return Some(format!(r"\\?\UNC{rest}"));
    }
    let (letter, device) = drive?;
    let rest = below(device)?;
    Some(format!(r"\\?\{letter}:{}", if rest.is_empty() { "\\" } else { rest }))
}

fn final_name(file: &File, flags: GETFINALPATHNAMEBYHANDLE_FLAGS) -> io::Result<OsString> {
    let mut buffer = vec![0u16; 32768];
    let count = unsafe { GetFinalPathNameByHandleW(HANDLE(file.as_raw_handle()), &mut buffer, flags) } as usize;
    if count == 0 {
        return Err(io::Error::last_os_error());
    }
    if count >= buffer.len() {
        return Err(io::Error::other("Final path exceeds native limit"));
    }
    Ok(OsString::from_wide(&buffer[..count]))
}

/// The first native target of a drive letter, e.g. `\Device\ImDisk0`.
pub(super) fn dos_device(letter: char) -> Option<String> {
    let name: Vec<u16> = std::ffi::OsStr::new(&format!("{letter}:")).encode_wide().chain(Some(0)).collect();
    let mut buffer = vec![0u16; 1024];
    let count = unsafe { QueryDosDeviceW(PCWSTR(name.as_ptr()), Some(&mut buffer)) } as usize;
    let first = buffer[..count.min(buffer.len())].split(|c| *c == 0).next()?;
    (!first.is_empty()).then(|| String::from_utf16_lossy(first))
}

#[cfg(test)]
#[path = "windows_final_path/tests/translation.rs"]
mod tests;
