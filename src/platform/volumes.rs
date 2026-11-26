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

/// Where writes are refused, read in the background and consulted while rendering.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WriteProtection {
    /// Each mount with whether it refuses writes: the innermost holding a path decides.
    pub mounts: Vec<(String, bool)>,
    /// Symbolic links at the top of a read-only drive, with their lexical target: the
    /// read-only `/` of Fedora Atomic reaches the writable `/var/home` through `/home`.
    pub links: Vec<(String, String)>,
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
    /// Which of `roots`, and of the mounts the list hides, refuse writes. Never stats a
    /// network drive.
    fn write_protection(&self, roots: &[DriveInfo]) -> WriteProtection {
        let platform = super::platform();
        WriteProtection {
            mounts: roots.iter().map(|drive| {
                let read_only = drive.drive_type != "remote"
                    && platform.volume_is_read_only(std::path::Path::new(&drive.path));
                (drive.path.clone(), read_only)
            }).collect(),
            links: Vec::new(),
        }
    }
    /// The returned guard owns the registration. Callbacks only notify `revision`.
    fn watch(&self, _revision: std::sync::Arc<crate::utils::change_signal::ChangeSignal>)
        -> Result<Option<Box<dyn Send>>, String> { Ok(None) }
    /// Whether folders on `drive` are watched by polling rather than system events.
    /// Windows polls removable storage so that no handle blocks its ejection.
    fn polls(&self, drive: &DriveInfo) -> bool {
        drive.is_usb() || drive.media_type.is_none()
    }
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
