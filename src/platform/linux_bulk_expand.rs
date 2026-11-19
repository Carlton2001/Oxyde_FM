//! Folders « Expand all » leaves closed on Linux: the kernel's pseudo file systems, whose
//! trees are huge and change constantly, and network or FUSE mounts (GVfs, NFS, SMB, WSL
//! drives), where every folder costs a round trip. They still open one by one.
use std::path::Path;

/// Root folders whose whole tree belongs to the kernel or to the running session.
const SYSTEM_ROOTS: [&str; 4] = ["/proc", "/sys", "/dev", "/run"];

/// `statfs` types: proc, sysfs, devpts, cgroup, cgroup2, debugfs, tracefs, securityfs, bpf,
/// pstore, autofs, then NFS, SMB2, CIFS, 9p, FUSE, AFS and Ceph.
const SKIPPED_TYPES: [i64; 17] = [
    0x9fa0, 0x6265_6572, 0x1cd1, 0x0027_e0eb, 0x6367_7270, 0x6462_6720, 0x7472_6163,
    0x7363_6673, 0xcafe_4a11, 0x6165_676c, 0x0187, 0x6969, 0xfe53_4d42, 0xff53_4d42,
    0x0102_1997, 0x6573_5546, 0x5346_414f,
];

pub(super) fn excluded(path: &str, name: &str) -> bool {
    if matches!(name.to_ascii_lowercase().as_str(), "$recycle.bin" | "system volume information") {
        return true;
    }
    SYSTEM_ROOTS.contains(&path) || skipped_type(Path::new(path))
}

#[allow(clippy::unnecessary_cast, reason = "`f_type` is narrower on 32-bit targets")]
fn skipped_type(path: &Path) -> bool {
    rustix::fs::statfs(path).is_ok_and(|found| SKIPPED_TYPES.contains(&(found.f_type as i64)))
}

#[cfg(test)]
#[path = "linux_bulk_expand/tests/excluded.rs"]
mod tests;
