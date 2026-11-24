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
        .filter(|point| contains(point, path))
        .max_by_key(|point| point.trim_end_matches(['/', '\\']).len())
        .map(String::as_str)
}

/// Whole components, with the platform's rules (case and prefixes on Windows).
#[cfg(target_os = "windows")]
fn contains(point: &str, path: &str) -> bool {
    crate::ui::paths::path_within(std::path::Path::new(path), std::path::Path::new(point))
}

#[cfg(not(target_os = "windows"))]
fn contains(point: &str, path: &str) -> bool {
    let point = point.trim_end_matches('/');
    path == point || path.strip_prefix(point).is_some_and(|rest| rest.starts_with('/'))
}

/// Drive letter or share of each path, whatever its spelling (`\\?\`, case, `/`).
#[cfg(target_os = "windows")]
pub(super) fn roots_differ(src: &str, dst: &str) -> bool {
    use oxyde_lib::utils::path_text::without_verbatim_prefix;
    let root = |path: &str| {
        let path = without_verbatim_prefix(&path.replace('/', "\\")).to_lowercase();
        std::path::Path::new(&path).components().next().map(|root| root.as_os_str().to_owned())
    };
    root(src) != root(dst)
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
