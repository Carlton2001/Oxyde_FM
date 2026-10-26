use std::path::Path;

#[cfg(target_os = "windows")]
use std::{collections::hash_map::DefaultHasher, hash::{Hash, Hasher}};
#[cfg(target_os = "windows")]
use windows::core::PCWSTR;
#[cfg(target_os = "windows")]
use windows::Win32::Storage::FileSystem::{
    CreateFileW, GetDriveTypeW, FILE_FLAG_BACKUP_SEMANTICS, FILE_SHARE_READ, FILE_SHARE_WRITE,
    OPEN_EXISTING,
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

pub fn is_network_path(path: &Path) -> bool {
    #[cfg(not(target_os = "windows"))]
    let _ = path;
    #[cfg(target_os = "windows")]
    {
        let path_str = plain_path(path);
        if path_str.starts_with("\\\\") {
            return true;
        }

        if path_str.len() >= 2 && path_str.chars().nth(1) == Some(':') {
            let root_path = format!("{}:\\", &path_str[0..1]);
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

pub fn get_physical_disk_id(path: &Path) -> u64 {
    #[cfg(not(target_os = "windows"))]
    let _ = path;
    #[cfg(target_os = "windows")]
    {
        let path_str = plain_path(path);
        if let Some(stripped) = path_str.strip_prefix("\\\\") {
            // UNC Path: Hash the host part to throttle per-server
            let parts: Vec<&str> = stripped.split('\\').collect();
            if let Some(host) = parts.first() {
                let mut hasher = DefaultHasher::new();
                host.hash(&mut hasher);
                return hasher.finish() | (1u64 << 63); // Set high bit for Network IDs
            }
            return 0;
        }

        if path_str.len() < 2 {
            return 0;
        }

        let drive_root = if path_str.chars().nth(1) == Some(':') {
            format!("\\\\.\\{}:", &path_str[0..1])
        } else {
            return 0;
        };

        let wide_path: Vec<u16> = drive_root
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        unsafe {
            let handle = CreateFileW(
                PCWSTR(wide_path.as_ptr()),
                0,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                None,
                OPEN_EXISTING,
                FILE_FLAG_BACKUP_SEMANTICS,
                None,
            );

            if let Ok(h) = handle {
                if !h.is_invalid() {
                    let mut device_number = STORAGE_DEVICE_NUMBER::default();
                    let mut bytes_returned = 0u32;
                    if DeviceIoControl(
                        h,
                        IOCTL_STORAGE_GET_DEVICE_NUMBER,
                        None,
                        0,
                        Some(&mut device_number as *mut _ as *mut _),
                        std::mem::size_of::<STORAGE_DEVICE_NUMBER>() as u32,
                        Some(&mut bytes_returned),
                        None,
                    )
                    .is_ok()
                    {
                        let id = (device_number.DeviceNumber + 1) as u64;
                        let _ = windows::Win32::Foundation::CloseHandle(h);
                        return id;
                    }
                    let _ = windows::Win32::Foundation::CloseHandle(h);
                }
            }
        }

        if path_str.len() >= 2 && path_str.chars().nth(1) == Some(':') {
            return (path_str.chars().next().unwrap().to_ascii_uppercase() as u64) + 1000;
        }
    }
    0
}

pub fn is_ssd(path: &Path) -> bool {
    #[cfg(not(target_os = "windows"))]
    let _ = path;
    #[cfg(target_os = "windows")]
    {
        if is_network_path(path) {
            return false;
        }

        let path_str = plain_path(path);
        if path_str.len() < 2 {
            return false;
        }

        let drive_root = if path_str.chars().nth(1) == Some(':') {
            format!("\\\\.\\{}:", &path_str[0..1])
        } else {
            return false;
        };

        let wide_path: Vec<u16> = drive_root
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        unsafe {
            let handle = CreateFileW(
                PCWSTR(wide_path.as_ptr()),
                0,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                None,
                OPEN_EXISTING,
                FILE_FLAG_BACKUP_SEMANTICS,
                None,
            );

            if let Ok(h) = handle {
                if !h.is_invalid() {
                    let mut query = STORAGE_PROPERTY_QUERY {
                        PropertyId: StorageDeviceSeekPenaltyProperty,
                        QueryType: PropertyStandardQuery,
                        ..Default::default()
                    };
                    let mut descriptor = DEVICE_SEEK_PENALTY_DESCRIPTOR::default();
                    let mut bytes_returned = 0u32;
                    let result = if DeviceIoControl(
                        h,
                        IOCTL_STORAGE_QUERY_PROPERTY,
                        Some(&mut query as *mut _ as *mut _),
                        std::mem::size_of::<STORAGE_PROPERTY_QUERY>() as u32,
                        Some(&mut descriptor as *mut _ as *mut _),
                        std::mem::size_of::<DEVICE_SEEK_PENALTY_DESCRIPTOR>() as u32,
                        Some(&mut bytes_returned),
                        None,
                    )
                    .is_ok()
                    {
                        !descriptor.IncursSeekPenalty
                    } else {
                        false
                    };
                    let _ = windows::Win32::Foundation::CloseHandle(h);
                    return result;
                }
            }
        }
    }
    false
}

#[cfg(all(test, target_os = "windows"))]
#[path = "hardware/tests/paths.rs"]
mod tests;
