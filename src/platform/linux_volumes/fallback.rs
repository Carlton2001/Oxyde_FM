//! Drives read from the kernel mount table, without UDisks2 (WSL, containers), and the
//! network mounts that UDisks2 does not manage.
use super::mounts::MountEntry;
use crate::models::DriveInfo;

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

pub(super) fn root() -> DriveInfo {
    DriveInfo {
        path: "/".to_string(),
        label: "Root".to_string(),
        drive_type: "fixed".to_string(),
        total_bytes: 0,
        free_bytes: 0,
        media_type: None,
        physical_id: None,
        remote_path: None,
        file_system: None,
        volume_bytes: 0,
    }
}

/// Every non-virtual mount, `/` included.
pub(super) fn drives(entries: &[MountEntry]) -> Vec<DriveInfo> {
    let mut drives: Vec<DriveInfo> = entries.iter().filter_map(drive).collect();
    if drives.is_empty() {
        drives.push(root());
    }
    drives
}

/// Network mounts, listed alongside the UDisks2 volumes.
pub(super) fn remote(entries: &[MountEntry]) -> Vec<DriveInfo> {
    entries.iter().filter_map(drive).filter(|drive| drive.drive_type == "remote").collect()
}

fn drive(entry: &MountEntry) -> Option<DriveInfo> {
    let (device, mount, fs_type) = (entry.source.as_str(), entry.point.as_str(), entry.fs_type.as_str());
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
        || fs_type == "cifs"
        || fs_type == "smb3"
    {
        "remote"
    } else if device.starts_with("/dev/sr")
        || device.starts_with("/dev/cdrom")
        || fs_type == "iso9660"
        || fs_type == "udf"
    {
        "cdrom"
    } else {
        "fixed"
    }
    .to_string();

    Some(DriveInfo {
        path: mount.to_string(),
        label,
        drive_type,
        total_bytes: 0,
        free_bytes: 0,
        media_type: None,
        physical_id: None,
        remote_path: device.starts_with("//").then(|| device.to_string()),
        file_system: super::inventory::file_system(fs_type, ""),
        volume_bytes: 0,
    })
}
