//! Volume domain: drive inventory, change notifications, ejection and disk images.
use super::MountedDiskImage;
use crate::models::DriveInfo;

/// What the drive menu offers for a volume.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DriveActions {
    pub unmount: bool,
    pub eject: bool,
}

/// A filesystem the system knows but has not mounted; `id` is what `mount` takes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnmountedVolume {
    pub id: String,
    pub label: String,
    pub size: u64,
    pub removable: bool,
    pub file_system: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VolumeError {
    /// The user dismissed the authentication dialog: nothing to report.
    Cancelled,
    /// Files are still open on the volume, by the named processes when they are known.
    Busy(Vec<String>),
    NotAuthorized,
    /// Windows hibernated, or shut down with Fast Startup, and left its NTFS volume in use.
    WindowsHibernated,
    Other(String),
}

impl std::fmt::Display for VolumeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Cancelled => formatter.write_str("Cancelled"),
            // The failure messages already say that an application uses the volume.
            Self::Busy(holders) => formatter.write_str(&holders.join(", ")),
            Self::NotAuthorized => formatter.write_str("Not authorized"),
            Self::WindowsHibernated => formatter.write_str("Windows is hibernated"),
            Self::Other(detail) => formatter.write_str(detail),
        }
    }
}

/// Calls may block on the system: run them on a background worker, `actions` excepted.
pub trait Volumes: Send + Sync {
    /// Liste les points de montage / lecteurs disponibles.
    fn list(&self, skip_hardware_info: bool) -> Vec<DriveInfo>;
    /// Fast mount snapshot; detailed hardware/capacity queries run separately.
    fn roots(&self) -> Vec<DriveInfo> { self.list(true) }
    /// Every mount point, hidden ones included, when roots alone do not separate volumes.
    fn mount_points(&self) -> Vec<String> { Vec::new() }
    /// The returned guard owns the registration. Callbacks only increment revision.
    fn watch(&self, _revision: std::sync::Arc<std::sync::atomic::AtomicU64>)
        -> Result<Option<Box<dyn Send>>, String> { Ok(None) }
    /// Decided from the listed drive alone, without system calls.
    fn actions(&self, drive: &DriveInfo) -> DriveActions {
        DriveActions { unmount: false, eject: drive.is_usb() }
    }
    fn unmounted(&self) -> Vec<UnmountedVolume> { Vec::new() }
    /// Mounts an `UnmountedVolume` and returns its mount point.
    fn mount(&self, _id: &str) -> Result<String, VolumeError> {
        Err(VolumeError::Other("Mounting is not supported".into()))
    }
    fn unmount(&self, _path: &str) -> Result<(), VolumeError> {
        Err(VolumeError::Other("Unmounting is not supported".into()))
    }
    fn eject(&self, path: &str) -> Result<(), VolumeError>;
    fn physical_disk_id(&self, path: &std::path::Path) -> u64;
    fn is_ssd(&self, path: &std::path::Path) -> bool;
    fn mounted_disk_images(&self) -> Result<Vec<MountedDiskImage>, String>;
    fn mount_disk_image(&self, path: &str) -> Result<Vec<String>, String>;
    fn unmount_disk_image(&self, path: &str) -> Result<(), String>;
}
