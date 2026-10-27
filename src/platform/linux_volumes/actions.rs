//! Unmounting and ejection through UDisks2. The calls let polkit ask for authentication,
//! as GNOME does; they block until the user answers.
use super::udisks::{self, Block};
use crate::platform::VolumeError;

const FILESYSTEM: &str = "org.freedesktop.UDisks2.Filesystem";
const DRIVE: &str = "org.freedesktop.UDisks2.Drive";

/// The filesystems to unmount, then the drive call that releases the device.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct Release {
    pub unmount: Vec<String>,
    pub drive: Option<(String, &'static str)>,
}

pub(super) fn unmount(path: &str) -> Result<(), VolumeError> {
    let blocks = udisks::blocks().map_err(volume_error)?;
    let block = mounted_at(&blocks, path)?;
    udisks::call(&block.object, FILESYSTEM, "Unmount").map_err(volume_error)
}

pub(super) fn eject(path: &str) -> Result<(), VolumeError> {
    let blocks = udisks::blocks().map_err(volume_error)?;
    let release = release(&blocks, path)?;
    for object in &release.unmount {
        match udisks::call(object, FILESYSTEM, "Unmount") {
            // A partition unmounted meanwhile is already released.
            Err(zbus::Error::MethodError(name, ..)) if name.as_str() == NOT_MOUNTED => {}
            result => result.map_err(volume_error)?,
        }
    }
    match release.drive {
        Some((object, method)) => udisks::call(&object, DRIVE, method).map_err(volume_error),
        None => Ok(()),
    }
}

/// Every mounted partition of the drive, so that powering it off cuts no filesystem.
/// Optical media are ejected; other drives are powered off as Windows' safe removal.
pub(super) fn release(blocks: &[Block], path: &str) -> Result<Release, VolumeError> {
    let block = mounted_at(blocks, path)?;
    let Some(drive) = &block.drive else {
        return Ok(Release { unmount: vec![block.object.clone()], drive: None });
    };
    let unmount = blocks
        .iter()
        .filter(|other| other.drive.as_ref().is_some_and(|other| other.object == drive.object))
        .filter(|other| other.mount_points.as_ref().is_some_and(|points| !points.is_empty()))
        .map(|other| other.object.clone())
        .collect();
    let method = if drive.optical && drive.ejectable {
        Some("Eject")
    } else if drive.can_power_off {
        Some("PowerOff")
    } else if drive.ejectable {
        Some("Eject")
    } else {
        None
    };
    Ok(Release { unmount, drive: method.map(|method| (drive.object.clone(), method)) })
}

fn mounted_at<'a>(blocks: &'a [Block], path: &str) -> Result<&'a Block, VolumeError> {
    blocks
        .iter()
        .find(|block| block.mount_points.as_ref().is_some_and(|points| points.iter().any(|point| point == path)))
        .ok_or_else(|| VolumeError::Other(format!("{path} is not a UDisks2 volume")))
}

const NOT_MOUNTED: &str = "org.freedesktop.UDisks2.Error.NotMounted";

pub(super) fn volume_error(error: zbus::Error) -> VolumeError {
    let zbus::Error::MethodError(name, detail, _) = &error else {
        return VolumeError::Other(error.to_string());
    };
    match name.as_str().strip_prefix("org.freedesktop.UDisks2.Error.") {
        Some("NotAuthorizedDismissed" | "Cancelled") => VolumeError::Cancelled,
        Some("NotAuthorized" | "NotAuthorizedCanObtain") => VolumeError::NotAuthorized,
        Some("DeviceBusy") => VolumeError::Busy,
        _ => VolumeError::Other(detail.clone().unwrap_or_else(|| name.to_string())),
    }
}

#[cfg(test)]
#[path = "tests/actions.rs"]
mod tests;
