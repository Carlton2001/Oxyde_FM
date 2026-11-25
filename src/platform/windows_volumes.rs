//! Windows volume domain: assembles drives, hardware queries and disk images.
use super::{MountedDiskImage, VolumeError, Volumes};
use crate::models::DriveInfo;

pub struct WindowsVolumes;

impl Volumes for WindowsVolumes {
    fn list(&self, skip_hardware_info: bool) -> Vec<DriveInfo> {
        super::windows_drives::list_drives(skip_hardware_info)
    }

    fn roots(&self) -> Vec<DriveInfo> {
        super::windows_drives::list_drive_roots()
    }

    fn mount_points(&self) -> Vec<String> {
        mount_points()
    }

    fn watch(&self, revision: std::sync::Arc<std::sync::atomic::AtomicU64>)
        -> Result<Option<Box<dyn Send>>, String> {
        super::windows_drives::watch_drive_changes(revision).map(Some)
    }

    fn eject(&self, path: &str) -> Result<(), VolumeError> {
        super::windows_drives::eject_drive(path)
    }

    fn physical_disk_id(&self, path: &std::path::Path) -> u64 {
        super::hardware::get_physical_disk_id(path)
    }

    fn is_ssd(&self, path: &std::path::Path) -> bool {
        super::hardware::is_ssd(path)
    }

    fn mounted_disk_images(&self) -> Result<Vec<MountedDiskImage>, String> {
        super::windows_disk_images::mounted_images()
    }

    fn mount_disk_image(&self, path: &str) -> Result<Vec<String>, String> {
        super::windows_disk_images::mount(path)
    }

    fn unmount_disk_image(&self, path: &str) -> Result<(), String> {
        super::windows_disk_images::unmount(path)
    }
}

/// Every drive and folder where a local volume is mounted, e.g. `C:\` and `C:\Data\`.
fn mount_points() -> Vec<String> {
    use windows::core::PCWSTR;
    use windows::Win32::Storage::FileSystem::{
        FindFirstVolumeW, FindNextVolumeW, FindVolumeClose, GetVolumePathNamesForVolumeNameW,
    };
    let mut volume = [0u16; 64];
    let Ok(search) = (unsafe { FindFirstVolumeW(&mut volume) }) else {
        return Vec::new();
    };
    let mut points = Vec::new();
    let mut names = vec![0u16; 1024];
    loop {
        let mut required = 0;
        let listed = loop {
            match unsafe {
                GetVolumePathNamesForVolumeNameW(PCWSTR(volume.as_ptr()), Some(&mut names), &mut required)
            } {
                Ok(()) => break true,
                Err(_) if required as usize > names.len() && required <= 1 << 20 => {
                    names.resize(required as usize, 0)
                }
                Err(_) => break false,
            }
        };
        if listed {
            points.extend(
                names
                    .split(|c| *c == 0)
                    .take_while(|name| !name.is_empty())
                    .map(String::from_utf16_lossy),
            );
        }
        if unsafe { FindNextVolumeW(search, &mut volume) }.is_err() {
            break;
        }
    }
    let _ = unsafe { FindVolumeClose(search) };
    points.sort();
    points.dedup();
    points
}
