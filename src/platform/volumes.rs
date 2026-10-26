//! Volume domain: drive inventory, change notifications, ejection and disk images.
use super::MountedDiskImage;
use crate::models::DriveInfo;

/// Calls may block on the system: run them on a background worker.
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
    fn eject(&self, path: &str) -> Result<(), String>;
    fn physical_disk_id(&self, path: &std::path::Path) -> u64;
    fn is_ssd(&self, path: &std::path::Path) -> bool;
    fn mounted_disk_images(&self) -> Result<Vec<MountedDiskImage>, String>;
    fn mount_disk_image(&self, path: &str) -> Result<Vec<String>, String>;
    fn unmount_disk_image(&self, path: &str) -> Result<(), String>;
}
