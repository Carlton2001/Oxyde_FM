//! Volumes mounted in a folder (`C:\Data`), reached by their volume root.
//!
//! The mount folder is a mount point reparse, which every lookup refuses. The
//! mount manager only registers mount points made by an administrator, so a
//! junction aimed at a volume root by anyone else is never rebased.
use std::{
    ffi::OsString,
    os::windows::ffi::{OsStrExt, OsStringExt},
    path::{Component, Path, PathBuf, Prefix},
};
use windows::{
    core::PCWSTR,
    Win32::Storage::FileSystem::{
        GetVolumeNameForVolumeMountPointW, GetVolumePathNameW, GetVolumePathNamesForVolumeNameW,
    },
};

/// The same path below the root of the volume mounted in one of its folders,
/// or None when no registered mount folder is crossed.
pub(super) fn rebase(absolute: &Path) -> Option<PathBuf> {
    let dos = drive_path(absolute)?;
    let mount = volume_path(&dos)?;
    let volume = volume_name(&mount)?;
    rebased(&dos, &mount, &volume, &registered_mounts(&volume)?)
}

/// `C:\…` or `\\?\C:\…` as `C:\…`; other forms never cross a mount folder
/// the mount manager knows by a drive path.
fn drive_path(absolute: &Path) -> Option<PathBuf> {
    let mut components = absolute.components();
    let drive = match components.next()? {
        Component::Prefix(prefix) => match prefix.kind() {
            Prefix::Disk(letter) | Prefix::VerbatimDisk(letter) => letter,
            _ => return None,
        },
        _ => return None,
    };
    let mut dos = PathBuf::from(format!("{}:\\", drive as char));
    dos.extend(components.filter(|component| !matches!(component, Component::RootDir)));
    Some(dos)
}

/// The rebased path when `mount` is a registered folder of `volume`, not a
/// drive root, and an ancestor of `dos`.
fn rebased(dos: &Path, mount: &Path, volume: &Path, registered: &[PathBuf]) -> Option<PathBuf> {
    let key = |path: &Path| path.to_string_lossy().trim_end_matches('\\').to_lowercase();
    let mount_key = key(mount);
    if mount.parent().is_none() || !registered.iter().any(|path| key(path) == mount_key) {
        return None;
    }
    let depth = mount.components().count();
    if key(&dos.components().take(depth).collect::<PathBuf>()) != mount_key {
        return None;
    }
    Some(dos.components().skip(depth).fold(volume.to_path_buf(), |path, name| path.join(name)))
}

fn wide(path: &Path) -> Vec<u16> {
    path.as_os_str().encode_wide().chain(Some(0)).collect()
}

fn until_nul(buffer: &[u16]) -> PathBuf {
    let length = buffer.iter().position(|c| *c == 0).unwrap_or(buffer.len());
    PathBuf::from(OsString::from_wide(&buffer[..length]))
}

/// The folder where the volume holding `dos` is mounted, e.g. `C:\Data\`.
fn volume_path(dos: &Path) -> Option<PathBuf> {
    let name = wide(dos);
    let mut buffer = vec![0u16; name.len() + 1];
    unsafe { GetVolumePathNameW(PCWSTR(name.as_ptr()), &mut buffer) }.ok()?;
    Some(until_nul(&buffer))
}

/// `\\?\Volume{…}\` for a mount folder.
fn volume_name(mount: &Path) -> Option<PathBuf> {
    let name = wide(mount);
    let mut buffer = [0u16; 64];
    unsafe { GetVolumeNameForVolumeMountPointW(PCWSTR(name.as_ptr()), &mut buffer) }.ok()?;
    Some(until_nul(&buffer))
}

/// Every drive and folder the mount manager records for `volume`.
fn registered_mounts(volume: &Path) -> Option<Vec<PathBuf>> {
    let name = wide(volume);
    let mut buffer = vec![0u16; 1024];
    loop {
        let mut required = 0;
        match unsafe {
            GetVolumePathNamesForVolumeNameW(PCWSTR(name.as_ptr()), Some(&mut buffer), &mut required)
        } {
            Ok(()) => break,
            Err(_) if required as usize > buffer.len() && required <= 1 << 20 => {
                buffer.resize(required as usize, 0)
            }
            Err(_) => return None,
        }
    }
    Some(
        buffer
            .split(|c| *c == 0)
            .take_while(|name| !name.is_empty())
            .map(|name| PathBuf::from(OsString::from_wide(name)))
            .collect(),
    )
}

#[cfg(test)]
#[path = "tests/mounted.rs"]
mod tests;
