//! Windows volume domain: assembles drives, hardware queries and disk images.
use super::{MountedDiskImage, Volumes};
use crate::models::DriveInfo;

pub struct WindowsVolumes;

impl Volumes for WindowsVolumes {
    fn list(&self, skip_hardware_info: bool) -> Vec<DriveInfo> {
        super::windows_drives::list_drives(skip_hardware_info)
    }

    fn roots(&self) -> Vec<DriveInfo> {
        super::windows_drives::list_drive_roots()
    }

    fn watch(&self, revision: std::sync::Arc<std::sync::atomic::AtomicU64>)
        -> Result<Option<Box<dyn Send>>, String> {
        super::windows_drives::watch_drive_changes(revision).map(Some)
    }

    fn eject(&self, path: &str) -> Result<(), String> {
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
