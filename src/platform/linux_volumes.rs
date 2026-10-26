//! Linux volume domain: UDisks2 inventory, mount table fallback and udisks2 unmounting.
use super::{MountedDiskImage, Volumes};
use crate::models::DriveInfo;

mod fallback;
mod inventory;
mod mounts;
mod udisks;

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

    fn eject(&self, path: &str) -> Result<(), String> {
        let output = std::process::Command::new("findmnt")
            .args(["--noheadings", "--output", "SOURCE", "--mountpoint", path])
            .output().map_err(|error| error.to_string())?;
        let device = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !output.status.success() || !device.starts_with("/dev/") {
            return Err("Cannot identify the mounted device".into());
        }
        let output = std::process::Command::new("udisksctl")
            .args(["unmount", "--block-device", &device, "--no-user-interaction"])
            .output().map_err(|error| error.to_string())?;
        if output.status.success() { Ok(()) } else {
            Err(String::from_utf8_lossy(&output.stderr).trim().into())
        }
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
