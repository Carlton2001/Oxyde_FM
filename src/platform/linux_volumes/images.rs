//! ISO and IMG images mounted as GNOME's Disk Image Mounter does: a read-only loop
//! device through UDisks2, each of its filesystems mounted under `/run/media/$USER`, and
//! the device detached once they are all unmounted.
use super::actions::volume_error;
use super::udisks::{self, Block};
use crate::platform::{MountedDiskImage, VolumeError};
use std::path::Path;
use std::time::{Duration, Instant};

const FILESYSTEM: &str = "org.freedesktop.UDisks2.Filesystem";
/// UDisks2 probes the new device in the background: its filesystems appear meanwhile.
const PROBE_TIMEOUT: Duration = Duration::from_secs(5);
const PROBE_INTERVAL: Duration = Duration::from_millis(100);
/// How long an unmounted image's devices have to detach on their own.
const DETACH_TIMEOUT: Duration = Duration::from_secs(1);

/// The filesystems on a loop device, itself or its partitions.
pub(super) fn volumes<'a>(blocks: &'a [Block], loop_object: &str) -> Vec<&'a Block> {
    blocks
        .iter()
        .filter(|block| block.object == loop_object || block.partition_of.as_deref() == Some(loop_object))
        .filter(|block| block.id_usage == "filesystem" && block.mount_points.is_some())
        .collect()
}

/// Each image a loop device reads, with the mount points of its filesystems.
pub(super) fn mounted(blocks: &[Block]) -> Vec<MountedDiskImage> {
    blocks
        .iter()
        .filter_map(|block| Some((block, block.loop_backing.as_ref()?)))
        .map(|(device, image)| MountedDiskImage {
            image_path: image.clone(),
            roots: volumes(blocks, &device.object)
                .into_iter()
                .flat_map(|block| block.mount_points.iter().flatten().cloned())
                .collect(),
        })
        .collect()
}

/// Mounts every filesystem of the image and returns their mount points; an image without
/// one is detached again and gives none.
pub(super) fn mount(path: &Path) -> Result<Vec<String>, VolumeError> {
    if !crate::utils::disk_image::mountable(path) {
        return Err(VolumeError::Other(format!("{} is not an ISO or IMG image", path.display())));
    }
    let image = std::fs::File::open(path).map_err(|error| VolumeError::Other(error.to_string()))?;
    let device = udisks::loop_setup(std::os::fd::AsFd::as_fd(&image)).map_err(volume_error)?;
    drop(image);
    let objects = match probed_volumes(&device) {
        Ok(objects) => objects,
        Err(error) => {
            detach(&device);
            return Err(error);
        }
    };
    let mut roots = Vec::new();
    for object in &objects {
        match udisks::mount(object) {
            Ok(point) => roots.push(point),
            Err(error) => {
                release(&device);
                return Err(volume_error(error));
            }
        }
    }
    if roots.is_empty() {
        detach(&device);
    } else if let Err(error) = udisks::set_autoclear(&device) {
        log::warn!("Loop device {device} stays attached after unmounting: {error}");
    }
    Ok(roots)
}

/// Waits for the probe: the unmounted filesystems once they appear and stop changing,
/// or none at the deadline.
fn probed_volumes(device: &str) -> Result<Vec<String>, VolumeError> {
    let deadline = Instant::now() + PROBE_TIMEOUT;
    let mut previous: Option<Vec<String>> = None;
    loop {
        let blocks = udisks::blocks().map_err(volume_error)?;
        let current: Vec<String> = volumes(&blocks, device)
            .into_iter()
            .filter(|block| block.mount_points.as_ref().is_some_and(Vec::is_empty))
            .map(|block| block.object.clone())
            .collect();
        if (!current.is_empty() && previous.as_ref() == Some(&current)) || Instant::now() >= deadline {
            return Ok(current);
        }
        previous = (!current.is_empty()).then_some(current);
        std::thread::sleep(PROBE_INTERVAL);
    }
}

/// Unmounts every filesystem of each loop device reading the image, then detaches them.
pub(super) fn unmount(path: &Path) -> Result<(), VolumeError> {
    let path = std::fs::canonicalize(path).map_err(|error| VolumeError::Other(error.to_string()))?;
    let blocks = udisks::blocks().map_err(volume_error)?;
    let devices: Vec<&Block> = blocks
        .iter()
        .filter(|block| block.loop_backing.as_deref().is_some_and(|backing| Path::new(backing) == path))
        .collect();
    if devices.is_empty() {
        return Err(VolumeError::Other(format!("{} is not mounted", path.display())));
    }
    for device in devices {
        for volume in volumes(&blocks, &device.object) {
            // As a drive's Unmount, which names the processes keeping it busy.
            if let Some(point) = volume.mount_points.iter().flatten().next() {
                super::actions::unmount(point)?;
            }
        }
    }
    // Autoclear detaches the devices once unmounted. Deleting one the kernel already
    // cleared counts as another user's device, for which polkit asks a password: only
    // those still reading the image after a moment are deleted.
    let deadline = Instant::now() + DETACH_TIMEOUT;
    loop {
        let blocks = udisks::blocks().map_err(volume_error)?;
        let attached: Vec<&Block> = blocks
            .iter()
            .filter(|block| block.loop_backing.as_deref().is_some_and(|backing| Path::new(backing) == path))
            .collect();
        if attached.is_empty() {
            return Ok(());
        }
        if Instant::now() >= deadline {
            attached.into_iter().for_each(|device| detach(&device.object));
            return Ok(());
        }
        std::thread::sleep(PROBE_INTERVAL);
    }
}

/// The image behind a mounted filesystem, when a loop device holds it.
pub(super) fn image_at(blocks: &[Block], point: &str) -> Option<String> {
    let mounted = blocks.iter().find(|block| block.mount_points.iter().flatten().any(|mounted| mounted == point))?;
    let device = match &mounted.partition_of {
        Some(table) => blocks.iter().find(|block| &block.object == table)?,
        None => mounted,
    };
    device.loop_backing.clone()
}

/// Best effort after a failed mount: unmounts what was mounted, then detaches.
fn release(device: &str) {
    if let Ok(blocks) = udisks::blocks() {
        for volume in volumes(&blocks, device) {
            if volume.mount_points.as_ref().is_some_and(|points| !points.is_empty()) {
                let _ = udisks::call(&volume.object, FILESYSTEM, "Unmount");
            }
        }
    }
    detach(device);
}

/// An autoclear device may already be gone once its last filesystem is unmounted.
fn detach(device: &str) {
    if let Err(error) = udisks::call(device, udisks::LOOP, "Delete") {
        log::debug!("Loop device {device} not deleted: {error}");
    }
}

#[cfg(test)]
#[path = "tests/images.rs"]
mod tests;
