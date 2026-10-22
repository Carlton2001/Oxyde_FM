//! Native virtual-storage calls. Callers already run these blocking operations
//! outside GPUI. No shell, subprocess, or process-wide COM state is needed.
use super::MountedDiskImage;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;
use windows::core::{PCWSTR, PWSTR};
use windows::Win32::Foundation::{
    CloseHandle, ERROR_DEV_NOT_EXIST, ERROR_INSUFFICIENT_BUFFER, ERROR_MORE_DATA, ERROR_NOT_READY,
    HANDLE, WIN32_ERROR,
};
use windows::Win32::Storage::FileSystem::{
    CreateFileW, GetLogicalDrives, GetVolumePathNamesForVolumeNameW, QueryDosDeviceW,
    FILE_FLAGS_AND_ATTRIBUTES, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
};
use windows::Win32::Storage::Vhd::*;
use windows::Win32::System::Ioctl::{IOCTL_STORAGE_GET_DEVICE_NUMBER, STORAGE_DEVICE_NUMBER};
use windows::Win32::System::IO::DeviceIoControl;

const MAX_BUFFER_BYTES: usize = 1024 * 1024;

struct DiskHandle(HANDLE);
impl Drop for DiskHandle {
    fn drop(&mut self) {
        let _ = unsafe { CloseHandle(self.0) };
    }
}

fn status(result: WIN32_ERROR) -> Result<(), String> {
    result.ok().map_err(|error| error.to_string())
}

fn wide(path: &Path) -> Result<Vec<u16>, String> {
    let mut text: Vec<_> = path.as_os_str().encode_wide().collect();
    if text.contains(&0) {
        return Err("A disk image path cannot contain NUL".into());
    }
    text.push(0);
    Ok(text)
}

fn open_device(path: &str) -> Result<DiskHandle, String> {
    let path = wide(Path::new(path))?;
    unsafe {
        CreateFileW(
            PCWSTR(path.as_ptr()),
            0,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            None,
            OPEN_EXISTING,
            FILE_FLAGS_AND_ATTRIBUTES(0),
            None,
        )
    }
    .map(DiskHandle)
    .map_err(|error| error.to_string())
}

fn is_optical(path: &Path) -> bool {
    path.extension().is_some_and(|extension| {
        extension.eq_ignore_ascii_case("iso") || extension.eq_ignore_ascii_case("img")
    })
}

fn open_image(path: &Path, access: VIRTUAL_DISK_ACCESS_MASK) -> Result<DiskHandle, String> {
    // IMG must explicitly select the ISO provider, just like the previous
    // StorageType=ISO query; UNKNOWN would infer the provider from its extension.
    let storage = VIRTUAL_STORAGE_TYPE {
        DeviceId: if is_optical(path) {
            VIRTUAL_STORAGE_TYPE_DEVICE_ISO
        } else {
            VIRTUAL_STORAGE_TYPE_DEVICE_UNKNOWN
        },
        VendorId: VIRTUAL_STORAGE_TYPE_VENDOR_MICROSOFT,
    };
    let parameters = OPEN_VIRTUAL_DISK_PARAMETERS {
        Version: OPEN_VIRTUAL_DISK_VERSION_1,
        Anonymous: OPEN_VIRTUAL_DISK_PARAMETERS_0 {
            Version1: OPEN_VIRTUAL_DISK_PARAMETERS_0_0 {
                RWDepth: OPEN_VIRTUAL_DISK_RW_DEPTH_DEFAULT,
            },
        },
    };
    let path = wide(path)?;
    let mut handle = HANDLE::default();
    status(unsafe {
        OpenVirtualDisk(
            &storage,
            PCWSTR(path.as_ptr()),
            access,
            OPEN_VIRTUAL_DISK_FLAG_NONE,
            Some(&parameters),
            &mut handle,
        )
    })?;
    Ok(DiskHandle(handle))
}

fn physical_path(handle: &DiskHandle) -> Result<String, WIN32_ERROR> {
    let mut buffer = vec![0u16; 512];
    loop {
        let mut bytes = (buffer.len() * 2) as u32;
        let result =
            unsafe { GetVirtualDiskPhysicalPath(handle.0, &mut bytes, PWSTR(buffer.as_mut_ptr())) };
        if result == ERROR_INSUFFICIENT_BUFFER
            && bytes as usize > buffer.len() * 2
            && bytes as usize <= MAX_BUFFER_BYTES
        {
            buffer.resize((bytes as usize).div_ceil(2), 0);
        } else if result.is_ok() {
            return Ok(String::from_utf16_lossy(
                buffer.split(|c| *c == 0).next().unwrap_or_default(),
            ));
        } else {
            return Err(result);
        }
    }
}

fn device_number(handle: &DiskHandle) -> Result<STORAGE_DEVICE_NUMBER, String> {
    let mut number = STORAGE_DEVICE_NUMBER::default();
    let mut returned = 0;
    unsafe {
        DeviceIoControl(
            handle.0,
            IOCTL_STORAGE_GET_DEVICE_NUMBER,
            None,
            0,
            Some(std::ptr::addr_of_mut!(number).cast()),
            std::mem::size_of_val(&number) as u32,
            Some(&mut returned),
            None,
        )
    }
    .map_err(|error| error.to_string())?;
    Ok(number)
}

fn drive_numbers() -> Vec<(String, STORAGE_DEVICE_NUMBER)> {
    let mask = unsafe { GetLogicalDrives() };
    (0..26)
        .filter(|index| mask & (1 << index) != 0)
        .filter_map(|index| {
            let letter = (b'A' + index) as char;
            let handle = open_device(&format!(r"\\.\{letter}:")).ok()?;
            Some((format!("{letter}:\\"), device_number(&handle).ok()?))
        })
        .collect()
}

fn roots_for(
    number: &STORAGE_DEVICE_NUMBER,
    drives: &[(String, STORAGE_DEVICE_NUMBER)],
) -> Vec<String> {
    drives
        .iter()
        .filter(|(_, drive)| {
            drive.DeviceType == number.DeviceType && drive.DeviceNumber == number.DeviceNumber
        })
        .map(|(root, _)| root.clone())
        .collect()
}

pub(super) fn mount(path: &Path) -> Result<Vec<String>, String> {
    // GET_INFO alone can fail for a permanently attached image; DETACH is
    // explicitly required by OpenVirtualDisk for reopening such an image.
    let info = open_image(
        path,
        VIRTUAL_DISK_ACCESS_GET_INFO | VIRTUAL_DISK_ACCESS_DETACH,
    )?;
    let info = match physical_path(&info) {
        Ok(_) => info,
        Err(ERROR_NOT_READY | ERROR_DEV_NOT_EXIST) => {
            drop(info);
            let access = if is_optical(path) {
                VIRTUAL_DISK_ACCESS_ATTACH_RO
            } else {
                VIRTUAL_DISK_ACCESS_ATTACH_RW
            };
            let handle = open_image(
                path,
                access | VIRTUAL_DISK_ACCESS_GET_INFO | VIRTUAL_DISK_ACCESS_DETACH,
            )?;
            let parameters = ATTACH_VIRTUAL_DISK_PARAMETERS {
                Version: ATTACH_VIRTUAL_DISK_VERSION_1,
                ..Default::default()
            };
            let flags = ATTACH_VIRTUAL_DISK_FLAG_PERMANENT_LIFETIME
                | if is_optical(path) {
                    ATTACH_VIRTUAL_DISK_FLAG_READ_ONLY
                } else {
                    ATTACH_VIRTUAL_DISK_FLAG_NONE
                };
            status(unsafe {
                AttachVirtualDisk(handle.0, None, flags, 0, Some(&parameters), None)
            })?;
            handle
        }
        Err(error) => {
            return Err(windows::core::Error::from_hresult(error.to_hresult()).to_string())
        }
    };
    // Drive-letter assignment may lag behind attachment. Preserve the previous
    // 20 x 100 ms settling window, including an empty result for an unformatted VHD.
    for _ in 0..20 {
        if let Ok(device) = physical_path(&info) {
            let number = device_number(&open_device(&device)?)?;
            let roots = roots_for(&number, &drive_numbers());
            if !roots.is_empty() {
                return Ok(roots);
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    Ok(Vec::new())
}

pub(super) fn unmount(path: &Path) -> Result<(), String> {
    let handle = open_image(path, VIRTUAL_DISK_ACCESS_DETACH)?;
    status(unsafe { DetachVirtualDisk(handle.0, DETACH_VIRTUAL_DISK_FLAG_NONE, 0) })
}

fn multi_strings(buffer: &[u16]) -> impl Iterator<Item = String> + '_ {
    buffer
        .split(|c| *c == 0)
        .take_while(|part| !part.is_empty())
        .map(String::from_utf16_lossy)
}

fn device_names() -> Result<Vec<String>, String> {
    let mut buffer = vec![0u16; 4096];
    loop {
        let length = unsafe { QueryDosDeviceW(PCWSTR::null(), Some(&mut buffer)) } as usize;
        if length != 0 {
            return Ok(multi_strings(&buffer[..length])
                .filter(|name| {
                    let lower = name.to_ascii_lowercase();
                    lower
                        .strip_prefix("physicaldrive")
                        .or_else(|| lower.strip_prefix("cdrom"))
                        .is_some_and(|suffix| {
                            !suffix.is_empty() && suffix.bytes().all(|c| c.is_ascii_digit())
                        })
                })
                .collect());
        }
        let error = windows::core::Error::from_thread();
        if error.code() != ERROR_INSUFFICIENT_BUFFER.to_hresult()
            || buffer.len() * 4 > MAX_BUFFER_BYTES
        {
            return Err(error.to_string());
        }
        buffer.resize(buffer.len() * 2, 0);
    }
}

fn host_path(volume: &str) -> String {
    let volume_root = format!("{}\\", volume.trim_end_matches('\\'));
    let Ok(name) = wide(Path::new(&volume_root)) else {
        return volume.to_string();
    };
    let mut buffer = vec![0u16; 512];
    loop {
        let mut required = 0;
        match unsafe {
            GetVolumePathNamesForVolumeNameW(
                PCWSTR(name.as_ptr()),
                Some(&mut buffer),
                &mut required,
            )
        } {
            Ok(()) => {
                return multi_strings(&buffer)
                    .min_by_key(String::len)
                    .unwrap_or_else(|| volume.to_string())
            }
            Err(error)
                if error.code() == ERROR_MORE_DATA.to_hresult()
                    && required as usize > buffer.len()
                    && required as usize * 2 <= MAX_BUFFER_BYTES =>
            {
                buffer.resize(required as usize, 0)
            }
            Err(_) => return volume.to_string(),
        }
    }
}

/// The native API writes pointers inside its variable-length output buffer.
/// Check both bounds and UTF-16 termination before consuming each string.
fn dependency_string(buffer: &[usize], pointer: PWSTR) -> Result<String, String> {
    let start = buffer.as_ptr() as usize;
    let end = start + std::mem::size_of_val(buffer);
    let address = pointer.0 as usize;
    if address < start || address >= end || !address.is_multiple_of(2) {
        return Err("Invalid virtual-storage string pointer".into());
    }
    let text = unsafe { std::slice::from_raw_parts(pointer.0, (end - address) / 2) };
    let length = text
        .iter()
        .position(|c| *c == 0)
        .ok_or("Unterminated virtual-storage string")?;
    String::from_utf16(&text[..length]).map_err(|error| error.to_string())
}

fn image_path(handle: &DiskHandle) -> Result<Option<String>, String> {
    let mut buffer = vec![0usize; 512];
    loop {
        let info = buffer.as_mut_ptr().cast::<STORAGE_DEPENDENCY_INFO>();
        // usize allocation preserves the alignment of the structure and entries.
        unsafe {
            (*info).Version = STORAGE_DEPENDENCY_INFO_VERSION_2;
        }
        let mut used = 0;
        let result = unsafe {
            GetStorageDependencyInformation(
                handle.0,
                GET_STORAGE_DEPENDENCY_FLAG_DISK_HANDLE | GET_STORAGE_DEPENDENCY_FLAG_HOST_VOLUMES,
                std::mem::size_of_val(buffer.as_slice()) as u32,
                info,
                Some(&mut used),
            )
        };
        if result == ERROR_INSUFFICIENT_BUFFER
            && used as usize > std::mem::size_of_val(buffer.as_slice())
            && used as usize <= MAX_BUFFER_BYTES
        {
            buffer.resize((used as usize).div_ceil(std::mem::size_of::<usize>()), 0);
            continue;
        }
        // Ordinary disks, physical optical drives and disappearing devices are
        // not mounted images. No image file is opened during this enumeration.
        if result.is_err() {
            return Ok(None);
        }
        let count = unsafe { (*info).NumberEntries } as usize;
        let offset = std::mem::offset_of!(STORAGE_DEPENDENCY_INFO, Anonymous);
        if count
            > (std::mem::size_of_val(buffer.as_slice()) - offset)
                / std::mem::size_of::<STORAGE_DEPENDENCY_INFO_TYPE_2>()
        {
            return Err("Invalid virtual-storage entry count".into());
        }
        let entries = unsafe {
            std::slice::from_raw_parts(
                std::ptr::addr_of!((*info).Anonymous.Version2Entries)
                    .cast::<STORAGE_DEPENDENCY_INFO_TYPE_2>(),
                count,
            )
        };
        // A nested VHD also reports its host chain. Only the immediate image
        // belongs to this device; never attribute its volumes to an ancestor.
        let Some(entry) = entries.iter().min_by_key(|entry| entry.AncestorLevel) else {
            return Ok(None);
        };
        let host = dependency_string(&buffer, entry.HostVolumeName)?;
        let relative = dependency_string(&buffer, entry.DependentVolumeRelativePath)?;
        return Ok(Some(format!(
            "{}\\{}",
            host_path(&host).trim_end_matches('\\'),
            relative.trim_start_matches('\\')
        )));
    }
}

pub(super) fn mounted_images() -> Result<Vec<MountedDiskImage>, String> {
    let drives = drive_numbers();
    let mut images: Vec<MountedDiskImage> = Vec::new();
    for device in device_names()? {
        let Ok(handle) = open_device(&format!(r"\\.\{device}")) else {
            continue;
        };
        let Some(image_path) = image_path(&handle)? else {
            continue;
        };
        let Ok(number) = device_number(&handle) else {
            continue;
        };
        let roots = roots_for(&number, &drives);
        if let Some(image) = images
            .iter_mut()
            .find(|image| image.image_path.eq_ignore_ascii_case(&image_path))
        {
            image.roots.extend(roots);
            image.roots.sort();
            image.roots.dedup();
        } else {
            images.push(MountedDiskImage { image_path, roots });
        }
    }
    images.sort_by_key(|image| image.image_path.to_lowercase());
    Ok(images)
}

#[cfg(test)]
#[path = "tests/native.rs"]
mod tests;
