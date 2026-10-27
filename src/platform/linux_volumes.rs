//! Linux volume domain: UDisks2 inventory, unmounting and ejection, mount table fallback
//! and notifications.
use super::{DriveActions, MountedDiskImage, VolumeError, Volumes};
use crate::models::DriveInfo;

mod actions;
mod fallback;
mod holders;
mod inventory;
mod mounts;
mod udisks;
mod watch;

pub struct LinuxVolumes;

impl Volumes for LinuxVolumes {
    fn list(&self, skip_hardware_info: bool) -> Vec<DriveInfo> {
        let table = mounts::table();
        let mut drives = match udisks::blocks() {
            Ok(blocks) => {
                let mut drives = inventory::mounted(&blocks, &inventory::Session::current());
                drives.extend(fallback::remote(&table));
                drives
            }
            Err(error) => {
                static REPORTED: std::sync::Once = std::sync::Once::new();
                REPORTED.call_once(|| log::warn!("UDisks2 unavailable, listing the mount table: {error}"));
                fallback::drives(&table)
            }
        };
        if !skip_hardware_info {
            // Network mounts can block statvfs indefinitely; they keep unknown capacities.
            for drive in drives.iter_mut().filter(|drive| drive.drive_type != "remote") {
                if let Ok(stats) = rustix::fs::statvfs(drive.path.as_str()) {
                    drive.total_bytes = stats.f_blocks.saturating_mul(stats.f_frsize);
                    drive.free_bytes = stats.f_bavail.saturating_mul(stats.f_frsize);
                }
            }
        }
        drives
    }

    fn mount_points(&self) -> Vec<String> {
        mounts::mount_points()
    }

    fn watch(&self, revision: std::sync::Arc<std::sync::atomic::AtomicU64>)
        -> Result<Option<Box<dyn Send>>, String> {
        watch::start(revision).map(|watch| Some(Box::new(watch) as Box<dyn Send>))
    }

    /// As GTK's places sidebar: a removable or optical drive is ejected, another volume
    /// unmounted. Without UDisks2, the mount table offers neither.
    fn actions(&self, drive: &DriveInfo) -> DriveActions {
        if !udisks::available() || drive.path == "/" || drive.drive_type == "remote" {
            return DriveActions::default();
        }
        let eject = matches!(drive.drive_type.as_str(), "removable" | "cdrom");
        DriveActions { unmount: !eject, eject }
    }

    fn unmount(&self, path: &str) -> Result<(), VolumeError> {
        actions::unmount(path)
    }

    fn eject(&self, path: &str) -> Result<(), VolumeError> {
        actions::eject(path)
    }

    fn physical_disk_id(&self, path: &std::path::Path) -> u64 {
        super::hardware::get_physical_disk_id(path)
    }

    fn is_ssd(&self, path: &std::path::Path) -> bool {
        super::hardware::is_ssd(path)
    }

    fn mounted_disk_images(&self) -> Result<Vec<MountedDiskImage>, String> {
        Ok(Vec::new())
    }
    fn mount_disk_image(&self, _path: &str) -> Result<Vec<String>, String> {
        Err("Disk image mounting is only supported on Windows".into())
    }
    fn unmount_disk_image(&self, _path: &str) -> Result<(), String> {
        Err("Disk image unmounting is only supported on Windows".into())
    }
}
