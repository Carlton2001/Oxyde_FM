//! Which UDisks2 volumes the drive list shows, following GVfs and Nautilus.
use super::udisks::{Block, Drive};
use crate::models::DriveInfo;
use crate::platform::UnmountedVolume;

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

/// Mounted volumes, `/` first as "Root", then internal, removable and optical drives, each
/// group by name. `HintIgnore` hides EFI, recovery and swap;
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
                volume_bytes: block.size,
            })
        })
        .collect();
    drives.sort_by(|a, b| {
        kind_rank(&a.drive_type).cmp(&kind_rank(&b.drive_type))
            .then_with(|| by_name(&a.label, &b.label))
    });
    let root = drives
        .iter()
        .position(|drive| drive.path == "/")
        .map(|index| drives.remove(index))
        .unwrap_or_else(super::fallback::root);
    drives.insert(0, root);
    drives
}

/// Filesystems UDisks2 could mount, as the GVfs volume monitor lists them: `HintIgnore`
/// hides EFI and recovery partitions, and a filesystem that `/etc/fstab` mounts outside the
/// user's places stays hidden unless its options carry `x-gvfs-show`.
pub(super) fn unmounted(blocks: &[Block], session: &Session) -> Vec<UnmountedVolume> {
    let mut volumes = blocks
        .iter()
        .filter(|block| block.mount_points.as_ref().is_some_and(Vec::is_empty))
        .filter(|block| block.id_usage == "filesystem" && !block.hint_ignore && !block.partitioned)
        .filter(|block| match &block.fstab {
            Some((point, options)) => {
                let options: Vec<&str> = options.split(',').collect();
                !options.contains(&"x-gvfs-hide") && (options.contains(&"x-gvfs-show") || session.shows(point))
            }
            None => true,
        })
        .map(|block| UnmountedVolume {
            id: block.object.clone(),
            // Empty: the interface names it by its size, as Nautilus.
            label: block.label.clone(),
            size: block.size,
            removable: block.drive.as_ref().is_some_and(|drive| drive.removable || drive.usb),
            file_system: file_system(&block.id_type, &block.id_version),
        })
        .collect::<Vec<_>>();
    // Internal volumes first, then removable ones, each group by name, as mounted drives.
    volumes.sort_by(|a, b| a.removable.cmp(&b.removable).then_with(|| by_name(&a.label, &b.label)));
    volumes
}

/// Named volumes first; unnamed ones keep their device order.
fn by_name(a: &str, b: &str) -> std::cmp::Ordering {
    a.is_empty().cmp(&b.is_empty()).then_with(|| crate::utils::compare_natural(a, b))
}

fn kind_rank(drive_type: &str) -> u8 {
    match drive_type {
        "fixed" => 0,
        "removable" => 1,
        _ => 2,
    }
}

/// Empty without a filesystem label: the interface names the volume by its size, as
/// Nautilus, rather than by the UUID UDisks2 mounted it under.
fn label(block: &Block, point: &str) -> String {
    if point == "/" { "Root".to_string() } else { block.label.clone() }
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
