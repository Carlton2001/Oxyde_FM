use std::path::Path;

#[cfg(target_os = "windows")]
use std::{collections::hash_map::DefaultHasher, hash::{Hash, Hasher}};
#[cfg(target_os = "windows")]
use windows::core::PCWSTR;
#[cfg(target_os = "windows")]
use windows::Win32::Foundation::{CloseHandle, HANDLE};
#[cfg(target_os = "windows")]
use windows::Win32::Storage::FileSystem::{
    CreateFileW, GetDriveTypeW, GetVolumeNameForVolumeMountPointW, GetVolumePathNameW,
    FILE_FLAG_BACKUP_SEMANTICS, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
};
#[cfg(target_os = "windows")]
use windows::Win32::System::Ioctl::{
    PropertyStandardQuery, StorageDeviceSeekPenaltyProperty, DEVICE_SEEK_PENALTY_DESCRIPTOR,
    IOCTL_STORAGE_GET_DEVICE_NUMBER, IOCTL_STORAGE_QUERY_PROPERTY, STORAGE_DEVICE_NUMBER,
    STORAGE_PROPERTY_QUERY,
};
#[cfg(target_os = "windows")]
use windows::Win32::System::IO::DeviceIoControl;

/// Verbatim prefixes (`\\?\C:\`, `\\?\UNC\host\`) name the same drive or share;
/// strip them so they are not mistaken for UNC network paths.
#[cfg(target_os = "windows")]
fn plain_path(path: &Path) -> std::borrow::Cow<'_, str> {
    let text = path.to_string_lossy();
    if let Some(rest) = text.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{rest}").into()
    } else if let Some(rest) = text.strip_prefix(r"\\?\") {
        rest.to_owned().into()
    } else {
        text
    }
}

/// The ASCII drive letter of `C:...`; `None` for any other start, including a
/// non-ASCII first character that a byte slice would cut in half.
#[cfg(target_os = "windows")]
fn drive_letter(path: &str) -> Option<char> {
    match path.as_bytes() {
        [letter, b':', ..] if letter.is_ascii_alphabetic() => {
            Some(char::from(*letter).to_ascii_uppercase())
        }
        _ => None,
    }
}

pub fn is_network_path(path: &Path) -> bool {
    #[cfg(not(target_os = "windows"))]
    let _ = path;
    #[cfg(target_os = "windows")]
    {
        let path_str = plain_path(path);
        if path_str.starts_with("\\\\") {
            return true;
        }

        if let Some(letter) = drive_letter(&path_str) {
            let root_path = format!("{letter}:\\");
            let wide_root: Vec<u16> = root_path.encode_utf16().chain(std::iter::once(0)).collect();
            unsafe {
                if GetDriveTypeW(PCWSTR(wide_root.as_ptr())) == 4 {
                    // DRIVE_REMOTE
                    return true;
                }
            }
        }
    }
    false
}

/// Linux answers from `linux_volumes::disks`.
#[cfg(target_os = "windows")]
pub fn get_physical_disk_id(path: &Path) -> u64 {
    let path_str = plain_path(path);
    if let Some(stripped) = path_str.strip_prefix(r"\\") {
        // UNC path: one ID per server, high bit set for network IDs.
        let Some(host) = stripped.split('\\').next() else {
            return 0;
        };
        let mut hasher = DefaultHasher::new();
        host.hash(&mut hasher);
        return hasher.finish() | (1u64 << 63);
    }
    let Some(letter) = drive_letter(&path_str) else {
        return 0;
    };
    // Callers ask once per file: the files of one folder share its answer.
    let folder = Path::new(&*path_str).parent().unwrap_or(Path::new(&*path_str)).to_path_buf();
    if let Some(id) = DISK_IDS.get(&folder) {
        return id;
    }
    let volume = folder_mounted_volume(&path_str);
    let device = volume.clone().unwrap_or_else(|| format!(r"\\.\{letter}:"));
    let id = with_device(&device, device_number).unwrap_or_else(|| match volume {
        // A volume mounted in a folder must not share the throttle of its host drive.
        Some(volume) => {
            let mut hasher = DefaultHasher::new();
            volume.hash(&mut hasher);
            hasher.finish() | (1u64 << 62)
        }
        None => letter as u64 + 1000,
    });
    DISK_IDS.insert(folder, id);
    id
}

/// Linux answers from `linux_volumes::disks`.
#[cfg(target_os = "windows")]
pub fn is_ssd(path: &Path) -> bool {
    if is_network_path(path) {
        return false;
    }
    let path_str = plain_path(path);
    let Some(letter) = drive_letter(&path_str) else {
        return false;
    };
    let device = folder_mounted_volume(&path_str).unwrap_or_else(|| format!(r"\\.\{letter}:"));
    with_device(&device, no_seek_penalty).unwrap_or(false)
}

/// Answers per folder, kept briefly: a drive can be replugged or remounted.
#[cfg(target_os = "windows")]
struct FolderCache(std::sync::LazyLock<std::sync::Mutex<std::collections::HashMap<std::path::PathBuf, (u64, std::time::Instant)>>>);

#[cfg(target_os = "windows")]
static DISK_IDS: FolderCache = FolderCache(std::sync::LazyLock::new(Default::default));

#[cfg(target_os = "windows")]
impl FolderCache {
    const LIFETIME: std::time::Duration = std::time::Duration::from_secs(30);
    const CAPACITY: usize = 4096;

    fn get(&self, folder: &Path) -> Option<u64> {
        let cache = self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        cache.get(folder).filter(|(_, at)| at.elapsed() < Self::LIFETIME).map(|(id, _)| *id)
    }

    fn insert(&self, folder: std::path::PathBuf, id: u64) {
        let mut cache = self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if cache.len() >= Self::CAPACITY {
            cache.retain(|_, (_, at)| at.elapsed() < Self::LIFETIME);
            if cache.len() >= Self::CAPACITY {
                cache.clear();
            }
        }
        cache.insert(folder, (id, std::time::Instant::now()));
    }
}

/// `\\?\Volume{…}` when `path` lies on a volume mounted in a folder such as
/// `C:\mnt\hdd\`; `None` on a drive root, where the letter names the volume.
#[cfg(target_os = "windows")]
fn folder_mounted_volume(path: &str) -> Option<String> {
    let wide = crate::utils::path_text::wide(path).ok()?;
    let mut mount = vec![0u16; wide.len() + 1];
    unsafe { GetVolumePathNameW(PCWSTR(wide.as_ptr()), &mut mount) }.ok()?;
    if mount.iter().position(|c| *c == 0)? <= 3 {
        return None;
    }
    let mut volume = [0u16; 64];
    unsafe { GetVolumeNameForVolumeMountPointW(PCWSTR(mount.as_ptr()), &mut volume) }.ok()?;
    let length = volume.iter().position(|c| *c == 0)?;
    let name = String::from_utf16(&volume[..length]).ok()?;
    // Without its final separator the name opens the volume device, not its root folder.
    Some(name.trim_end_matches('\\').to_owned())
}

#[cfg(target_os = "windows")]
fn with_device<T>(device: &str, query: impl FnOnce(HANDLE) -> Option<T>) -> Option<T> {
    let wide: Vec<u16> = device.encode_utf16().chain(std::iter::once(0)).collect();
    let handle = unsafe {
        CreateFileW(
            PCWSTR(wide.as_ptr()),
            0,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            None,
            OPEN_EXISTING,
            FILE_FLAG_BACKUP_SEMANTICS,
            None,
        )
    }
    .ok()
    .filter(|handle| !handle.is_invalid())?;
    let result = query(handle);
    let _ = unsafe { CloseHandle(handle) };
    result
}

#[cfg(target_os = "windows")]
fn device_number(handle: HANDLE) -> Option<u64> {
    let mut device_number = STORAGE_DEVICE_NUMBER::default();
    let mut bytes_returned = 0u32;
    unsafe {
        DeviceIoControl(
            handle,
            IOCTL_STORAGE_GET_DEVICE_NUMBER,
            None,
            0,
            Some(&mut device_number as *mut _ as *mut _),
            std::mem::size_of::<STORAGE_DEVICE_NUMBER>() as u32,
            Some(&mut bytes_returned),
            None,
        )
    }
    .ok()?;
    Some(device_number.DeviceNumber as u64 + 1)
}

#[cfg(target_os = "windows")]
fn no_seek_penalty(handle: HANDLE) -> Option<bool> {
    let mut query = STORAGE_PROPERTY_QUERY {
        PropertyId: StorageDeviceSeekPenaltyProperty,
        QueryType: PropertyStandardQuery,
        ..Default::default()
    };
    let mut descriptor = DEVICE_SEEK_PENALTY_DESCRIPTOR::default();
    let mut bytes_returned = 0u32;
    unsafe {
        DeviceIoControl(
            handle,
            IOCTL_STORAGE_QUERY_PROPERTY,
            Some(&mut query as *mut _ as *mut _),
            std::mem::size_of::<STORAGE_PROPERTY_QUERY>() as u32,
            Some(&mut descriptor as *mut _ as *mut _),
            std::mem::size_of::<DEVICE_SEEK_PENALTY_DESCRIPTOR>() as u32,
            Some(&mut bytes_returned),
            None,
        )
    }
    .ok()?;
    Some(!descriptor.IncursSeekPenalty)
}

#[cfg(all(test, target_os = "windows"))]
#[path = "hardware/tests/paths.rs"]
mod tests;
