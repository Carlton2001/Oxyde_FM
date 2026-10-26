//! Linux volume domain: mount table inventory and udisks2 unmounting.
use super::{MountedDiskImage, Volumes};
use crate::models::DriveInfo;

mod mounts;

pub struct LinuxVolumes;

impl Volumes for LinuxVolumes {
    fn list(&self, _skip_hardware_info: bool) -> Vec<DriveInfo> {
        // Systèmes de fichiers virtuels à ignorer
        const SKIP_FS: &[&str] = &[
            "proc",
            "sysfs",
            "devtmpfs",
            "devpts",
            "tmpfs",
            "cgroup",
            "cgroup2",
            "pstore",
            "bpf",
            "tracefs",
            "debugfs",
            "hugetlbfs",
            "mqueue",
            "fusectl",
            "configfs",
            "ramfs",
            "overlay",
            "aufs",
            "squashfs",
            "efivarfs",
            "securityfs",
            "autofs",
        ];

        let content = std::fs::read_to_string("/proc/mounts").unwrap_or_default();
        let mut drives: Vec<DriveInfo> = content
            .lines()
            .filter_map(|line| {
                let mut parts = line.splitn(4, ' ');
                let device = parts.next()?;
                let mount = parts.next()?;
                let fs_type = parts.next()?;

                if SKIP_FS.contains(&fs_type) {
                    return None;
                }
                if mount.starts_with("/proc")
                    || mount.starts_with("/sys")
                    || mount.starts_with("/dev/pts")
                    // udisks2 mounts removable volumes under /run/media/$USER.
                    || (mount.starts_with("/run") && !mount.starts_with("/run/media/"))
                {
                    return None;
                }

                let label = if mount == "/" {
                    "Root".to_string()
                } else {
                    std::path::Path::new(mount)
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| mount.to_string())
                };

                let drive_type = if device.starts_with("//")
                    || device.starts_with("cifs")
                    || fs_type == "nfs"
                    || fs_type == "nfs4"
                    || fs_type == "smbfs"
                {
                    "remote"
                } else if device.starts_with("/dev/sr")
                    || device.starts_with("/dev/cdrom")
                    || fs_type == "iso9660"
                    || fs_type == "udf"
                {
                    "cdrom"
                } else if device.starts_with("/dev/sd")
                    || device.starts_with("/dev/nvme")
                    || device.starts_with("/dev/mmcblk")
                    || device.starts_with("/dev/vd")
                {
                    "fixed"
                } else {
                    "fixed"
                }
                .to_string();

                Some(DriveInfo {
                    path: mount.to_string(),
                    label,
                    drive_type,
                    // TODO: remplir via statvfs (nécessite la crate `nix`)
                    total_bytes: 0,
                    free_bytes: 0,
                    media_type: None,
                    physical_id: None,
                    remote_path: if device.starts_with("//") {
                        Some(device.to_string())
                    } else {
                        None
                    },
                })
            })
            .collect();

        if drives.is_empty() {
            drives.push(DriveInfo {
                path: "/".to_string(),
                label: "Root".to_string(),
                drive_type: "fixed".to_string(),
                total_bytes: 0,
                free_bytes: 0,
                media_type: None,
                physical_id: None,
                remote_path: None,
            });
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
