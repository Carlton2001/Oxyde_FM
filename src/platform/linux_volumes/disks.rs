//! Physical disks behind paths, which pace copies, scans and searches: the mount table
//! gives a path's device, sysfs its whole disk and whether that disk rotates.
use super::mounts::{self, MountEntry};
use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

const REMOTE: &[&str] = &["nfs", "nfs4", "cifs", "smb3", "smbfs", "fuse.sshfs"];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct Disk {
    /// 0 when unknown; network shares set the high bit, as on Windows.
    pub id: u64,
    pub ssd: bool,
}

/// Callers query every file of a copy or search: one answer per filesystem, kept briefly
/// because the kernel reuses device numbers after a remount.
pub(super) fn disk(path: &Path) -> Disk {
    const FRESH: Duration = Duration::from_secs(10);
    static CACHE: Mutex<Option<HashMap<u64, (Instant, Disk)>>> = Mutex::new(None);
    let Some((existing, device)) = path.ancestors().find_map(|path| Some((path, path.metadata().ok()?.dev()))) else {
        return Disk::default();
    };
    let now = Instant::now();
    if let Some((time, disk)) = CACHE.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
        .as_ref()
        .and_then(|cache| cache.get(&device))
    {
        if now.duration_since(*time) < FRESH {
            return *disk;
        }
    }
    let path = std::fs::canonicalize(existing).unwrap_or_else(|_| existing.to_path_buf());
    let disk = resolve(&path, &mounts::table(), Path::new("/sys"), &|source| {
        let device = std::fs::metadata(source).ok()?.rdev();
        Some((rustix::fs::major(device), rustix::fs::minor(device)))
    });
    let mut cache = CACHE.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let cache = cache.get_or_insert_with(HashMap::new);
    cache.retain(|_, (time, _)| now.duration_since(*time) < FRESH);
    cache.insert(device, (now, disk));
    disk
}

/// `device` gives the major and minor numbers of a `/dev` node.
pub(super) fn resolve(
    path: &Path,
    entries: &[MountEntry],
    sys: &Path,
    device: &dyn Fn(&str) -> Option<(u32, u32)>,
) -> Disk {
    // The deepest mount holding the path; a later mount on the same point hides earlier ones.
    let Some(entry) = entries
        .iter()
        .filter(|entry| path.starts_with(&entry.point))
        .max_by_key(|entry| Path::new(&entry.point).components().count())
    else {
        return Disk::default();
    };
    if REMOTE.contains(&entry.fs_type.as_str()) || entry.source.starts_with("//") {
        let host = entry.source.trim_start_matches("//");
        let host = host.split(['/', ':']).next().unwrap_or(host);
        let host = host.rsplit('@').next().unwrap_or(host);
        let mut hasher = DefaultHasher::new();
        host.hash(&mut hasher);
        return Disk { id: hasher.finish() | (1 << 63), ssd: false };
    }
    if !entry.source.starts_with("/dev/") {
        return Disk::default();
    }
    let Some(disk) = device(&entry.source).and_then(|(major, minor)| whole_disk(sys, major, minor)) else {
        return Disk::default();
    };
    let id = std::fs::read_to_string(disk.join("dev"))
        .ok()
        .and_then(|dev| {
            let (major, minor) = dev.trim().split_once(':')?;
            Some((major.parse::<u64>().ok()? << 20 | minor.parse::<u64>().ok()?) + 1)
        })
        .unwrap_or(0);
    let ssd = std::fs::read_to_string(disk.join("queue/rotational")).is_ok_and(|rotational| rotational.trim() == "0");
    Disk { id, ssd }
}

/// The sysfs directory of the disk holding a block device: a partition's parent, and for a
/// device mapper volume (LUKS, LVM), the disk of its first underlying device.
fn whole_disk(sys: &Path, major: u32, minor: u32) -> Option<PathBuf> {
    let mut device = std::fs::canonicalize(sys.join(format!("dev/block/{major}:{minor}"))).ok()?;
    for _ in 0..8 {
        let slave = std::fs::read_dir(device.join("slaves")).ok()
            .and_then(|mut slaves| slaves.find_map(Result::ok));
        match slave {
            Some(slave) => device = std::fs::canonicalize(slave.path()).ok()?,
            None if device.join("partition").exists() => return device.parent().map(Path::to_path_buf),
            None => return Some(device),
        }
    }
    None
}

#[cfg(test)]
#[path = "tests/disks.rs"]
mod tests;
