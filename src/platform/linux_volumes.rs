//! Linux volume domain: UDisks2 inventory, unmounting and ejection, mount table fallback
//! and notifications.
use super::{DriveActions, MountedDiskImage, UnmountedVolume, VolumeError, Volumes};
use crate::models::DriveInfo;

mod actions;
mod disks;
mod fallback;
mod holders;
mod inventory;
mod mounts;
mod signals;
mod udisks;
mod watch;

pub struct LinuxVolumes;

/// Shares GVfs mounted, for Oxyde or Nautilus, as GTK's sidebar lists them.
fn gvfs_shares() -> Vec<DriveInfo> {
    super::linux_gio::mounts().into_iter().map(|mount| DriveInfo {
        file_system: protocol(&mount.uri).map(str::to_string),
        path: mount.path.to_string_lossy().into_owned(),
        label: mount.name,
        drive_type: "remote".into(),
        total_bytes: 0,
        free_bytes: 0,
        media_type: None,
        physical_id: None,
        remote_path: Some(mount.uri),
        volume_bytes: 0,
    }).collect()
}

fn protocol(uri: &str) -> Option<&'static str> {
    let scheme = uri.split_once("://")?.0.to_ascii_lowercase();
    Some(match scheme.as_str() {
        "smb" => "SMB",
        "sftp" => "SFTP",
        "ftp" | "ftps" => "FTP",
        "dav" | "davs" => "WebDAV",
        "afp" => "AFP",
        "nfs" => "NFS",
        _ => return None,
    })
}

fn gvfs_share(path: &str) -> bool {
    let path = std::path::Path::new(path);
    super::linux_gio::fuse_root().is_some_and(|root| path.parent() == Some(root.as_path()))
        && path.file_name().is_some_and(super::linux_gio::network_mount_name)
}

impl Volumes for LinuxVolumes {
    fn list(&self, skip_hardware_info: bool) -> Vec<DriveInfo> {
        let table = mounts::table();
        let mut drives = match udisks::blocks() {
            Ok(blocks) => {
                let mut drives = inventory::mounted(&blocks, &inventory::Session::current());
                drives.extend(fallback::remote(&table));
                drives.extend(gvfs_shares());
                drives
            }
            Err(error) => {
                static REPORTED: std::sync::Once = std::sync::Once::new();
                REPORTED.call_once(|| log::warn!("UDisks2 unavailable, listing the mount table: {error}"));
                let mut drives = fallback::drives(&table);
                drives.extend(gvfs_shares());
                drives
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

    /// inotify keeps nothing open and sees local changes, FUSE mounts included; only a
    /// network share changed by another machine needs polling.
    fn polls(&self, drive: &DriveInfo) -> bool {
        drive.drive_type == "remote"
    }

    /// As GTK's places sidebar: a removable or optical drive is ejected, another volume
    /// unmounted. Without UDisks2, the mount table offers neither.
    fn actions(&self, drive: &DriveInfo) -> DriveActions {
        if gvfs_share(&drive.path) {
            return DriveActions { unmount: true, eject: false };
        }
        if !udisks::available() || drive.path == "/" || drive.drive_type == "remote" {
            return DriveActions::default();
        }
        let eject = matches!(drive.drive_type.as_str(), "removable" | "cdrom");
        DriveActions { unmount: !eject, eject }
    }

    fn unmounted(&self) -> Vec<UnmountedVolume> {
        udisks::blocks()
            .map(|blocks| inventory::unmounted(&blocks, &inventory::Session::current()))
            .unwrap_or_default()
    }

    fn mount(&self, id: &str) -> Result<String, VolumeError> {
        actions::mount(id)
    }

    fn unmount(&self, path: &str) -> Result<(), VolumeError> {
        if gvfs_share(path) {
            return super::linux_gio::unmount(std::path::Path::new(path));
        }
        actions::unmount(path)
    }

    fn eject(&self, path: &str) -> Result<(), VolumeError> {
        actions::eject(path)
    }

    fn physical_disk_id(&self, path: &std::path::Path) -> u64 {
        disks::disk(path).id
    }

    fn is_ssd(&self, path: &std::path::Path) -> bool {
        disks::disk(path).ssd
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
