//! Which UDisks2 volumes the drive list shows, following GVfs and Nautilus.
use super::udisks::{Block, Drive};
use crate::models::DriveInfo;

/// Mounts are only shown in places the user owns, as `g_unix_mount_guess_should_display`.
pub(super) struct Session {
    pub home: String,
    pub user: String,
}

impl Session {
    pub fn current() -> Self {
        Self {
            home: std::env::var("HOME").unwrap_or_default(),
            user: std::env::var("USER").unwrap_or_default(),
        }
    }

    fn shows(&self, point: &str) -> bool {
        if point == "/" {
            return true;
        }
        let user_media = (!self.user.is_empty()).then(|| format!("/run/media/{}/", self.user));
        let home = (!self.home.is_empty() && self.home != "/").then(|| format!("{}/", self.home.trim_end_matches('/')));
        let shown = [user_media.as_deref(), Some("/media/"), home.as_deref()]
            .into_iter()
            .flatten()
            .filter_map(|prefix| point.strip_prefix(prefix))
            .any(|rest| !rest.is_empty() && !rest.split('/').any(|part| part.starts_with('.')));
        shown
    }
}

/// Mounted volumes, `/` first as "Root". `HintIgnore` hides EFI, recovery and swap;
/// system mounts such as `/boot` stay hidden because of their location.
pub(super) fn mounted(blocks: &[Block], session: &Session) -> Vec<DriveInfo> {
    let mut drives: Vec<DriveInfo> = blocks
        .iter()
        .filter(|block| !block.hint_ignore)
        .filter_map(|block| {
            let point = block.mount_points.as_ref()?.iter().find(|point| session.shows(point))?;
            Some(DriveInfo {
                path: point.clone(),
                label: label(block, point),
                drive_type: drive_type(block.drive.as_ref()).to_string(),
                total_bytes: 0,
                free_bytes: 0,
                media_type: None,
                physical_id: None,
                remote_path: None,
                file_system: file_system(&block.id_type, &block.id_version),
            })
        })
        .collect();
    let root = drives
        .iter()
        .position(|drive| drive.path == "/")
        .map(|index| drives.remove(index))
        .unwrap_or_else(super::fallback::root);
    drives.insert(0, root);
    drives
}

fn label(block: &Block, point: &str) -> String {
    if point == "/" {
        "Root".to_string()
    } else if !block.label.is_empty() {
        block.label.clone()
    } else {
        std::path::Path::new(point)
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| point.to_string())
    }
}

/// Names as file managers show them; FUSE mounts do not tell their real format.
pub(super) fn file_system(kind: &str, version: &str) -> Option<String> {
    let name = match kind {
        "" | "fuseblk" => return None,
        kind if kind.starts_with("fuse") => return None,
        "vfat" if version.starts_with("FAT") => version,
        "vfat" | "msdos" => "FAT",
        "ntfs" | "ntfs3" => "NTFS",
        "exfat" => "exFAT",
        "btrfs" => "Btrfs",
        "xfs" => "XFS",
        "f2fs" => "F2FS",
        "iso9660" => "ISO 9660",
        "udf" => "UDF",
        "hfsplus" => "HFS+",
        "apfs" => "APFS",
        "nfs" | "nfs4" => "NFS",
        "cifs" | "smb3" | "smbfs" => "SMB",
        // ext2/3/4, zfs… keep their usual lowercase names.
        kind => kind,
    };
    Some(name.to_string())
}

fn drive_type(drive: Option<&Drive>) -> &'static str {
    match drive {
        Some(drive) if drive.optical => "cdrom",
        Some(drive) if drive.removable || drive.usb => "removable",
        _ => "fixed",
    }
}

#[cfg(test)]
#[path = "tests/inventory.rs"]
mod tests;
