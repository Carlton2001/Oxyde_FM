//! Mount table snapshot for drag defaults: refreshed in the background, read while rendering.
use std::sync::{Arc, RwLock};

static MOUNT_POINTS: RwLock<Option<Arc<[String]>>> = RwLock::new(None);

pub(crate) fn replace_mount_points(points: Vec<String>) {
    if let Ok(mut current) = MOUNT_POINTS.write() {
        if current.as_deref() != Some(points.as_slice()) {
            *current = Some(points.into());
        }
    }
}

fn snapshot() -> Option<Arc<[String]>> {
    MOUNT_POINTS.read().ok().and_then(|points| points.clone())
}

/// Deepest mount point containing `path`, compared lexically: symbolic links are not resolved.
fn mount_of<'a>(points: &'a [String], path: &str) -> Option<&'a str> {
    points
        .iter()
        .filter(|point| {
            let point = point.trim_end_matches('/');
            path == point
                || path.strip_prefix(point).is_some_and(|rest| rest.starts_with('/'))
        })
        .max_by_key(|point| point.len())
        .map(String::as_str)
}

/// Before the first snapshot, paths are treated as one volume, as `/` roots used to be.
pub(super) fn mounts_differ(src: &str, dst: &str) -> bool {
    snapshot().is_some_and(|points| volumes_differ(&points, src, dst))
}

fn volumes_differ(points: &[String], src: &str, dst: &str) -> bool {
    mount_of(points, src) != mount_of(points, dst)
}

#[cfg(test)]
#[path = "tests/volumes.rs"]
mod tests;
