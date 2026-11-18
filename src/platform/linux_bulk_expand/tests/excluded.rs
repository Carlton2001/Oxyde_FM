use super::*;

#[test]
fn kernel_and_session_trees_stay_closed() {
    for root in SYSTEM_ROOTS {
        assert!(excluded(root, &root[1..]), "{root}");
    }
    assert!(excluded("/sys/kernel", "kernel"));
    assert!(excluded("/proc/self", "self"));
    let folder = tempfile::tempdir().unwrap();
    let proc_named = folder.path().join("proc");
    std::fs::create_dir(&proc_named).unwrap();
    assert!(!excluded(&proc_named.to_string_lossy(), "proc"));
    assert!(excluded(&folder.path().join("$RECYCLE.BIN").to_string_lossy(), "$RECYCLE.BIN"));
}

/// A network or FUSE mount: `OXYDE_TEST_REMOTE_MOUNT=<folder>` (WSL: `/mnt/c`).
#[test]
fn network_mounts_stay_closed() {
    let Some(folder) = std::env::var("OXYDE_TEST_REMOTE_MOUNT").ok() else { return };
    assert!(excluded(&folder, "mount"));
}
