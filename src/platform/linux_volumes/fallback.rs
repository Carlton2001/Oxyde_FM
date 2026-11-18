//! Drives read from the kernel mount table, without UDisks2 (WSL, containers), and the
//! network mounts that UDisks2 does not manage.
use super::mounts::MountEntry;
use crate::models::DriveInfo;

/// Virtual file systems, never drives.
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

/// Network file systems: `statvfs` on one out of reach blocks without end. FUSE mounts
/// other than block devices (sshfs, rclone, gvfsd-fuse) are as remote.
const REMOTE_FS: &[&str] = &[
    "nfs", "nfs4", "smbfs", "cifs", "smb3", "9p", "ceph", "glusterfs", "afs", "davfs", "lustre", "ncpfs", "sshfs",
];

fn remote_fs(fs_type: &str, device: &str) -> bool {
    // WSL mounts the Windows drives through 9p, named by their letter.
    let windows_drive = fs_type == "9p" && device.len() == 3 && device.ends_with(":\\");
    !windows_drive && (REMOTE_FS.contains(&fs_type) || (fs_type.starts_with("fuse.") && fs_type != "fuseblk"))
}

/// Places the system keeps for itself, never drives.
const SYSTEM: &[&str] = &["/boot", "/etc", "/usr", "/var", "/opt", "/root", "/home", "/tmp", "/init"];

/// What the desktop lists, as GIO decides without UDisks2: the root, removable volumes,
/// direct mounts under `/mnt` (WSL's drives among them) and mounts inside the home; not
/// `/boot`, `/home`, `/var/log` nor Btrfs snapshots. Network mounts may live elsewhere.
fn displayed(mount: &str, remote: bool) -> bool {
    let under = |root: &str| mount.strip_prefix(root).is_some_and(|rest| rest.starts_with('/'));
    if mount == "/" {
        return true;
    }
    if mount.split('/').any(|component| component.starts_with('.')) {
        return false;
    }
    if under("/media") || under("/run/media") {
        return true;
    }
    if mount.strip_prefix("/mnt/").is_some_and(|name| !name.is_empty() && !name.contains('/')) {
        return true;
    }
    if let Some(home) = dirs::home_dir().and_then(|home| home.to_str().map(str::to_owned)) {
        if under(&home) {
            return true;
        }
    }
    remote && !SYSTEM.iter().any(|system| mount == *system || under(system)) && !under("/mnt")
        && !["/proc", "/sys", "/dev", "/run"].iter().any(|system| under(system))
}

fn drive(entry: &MountEntry) -> Option<DriveInfo> {
    let (device, mount, fs_type) = (entry.source.as_str(), entry.point.as_str(), entry.fs_type.as_str());
    if SKIP_FS.contains(&fs_type) {
        return None;
    }
    let remote = device.starts_with("//") || device.starts_with("cifs") || remote_fs(fs_type, device);
    if !displayed(mount, remote) {
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

    let drive_type = if remote {
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

#[cfg(test)]
#[path = "tests/fallback.rs"]
mod tests;
