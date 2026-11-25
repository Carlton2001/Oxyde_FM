use super::*;
use std::os::unix::fs::PermissionsExt;
use std::sync::atomic::AtomicUsize;

fn tree() -> (tempfile::TempDir, PathBuf) {
    let root = tempfile::tempdir().unwrap();
    let folder = root.path().join("folder");
    std::fs::create_dir_all(folder.join("sub/deep")).unwrap();
    std::fs::create_dir_all(folder.join("other/more")).unwrap();
    for (index, path) in ["a", "sub/b", "sub/deep/c", "sub/deep/d", "other/e", "other/more/f"].iter().enumerate() {
        std::fs::write(folder.join(path), [index as u8]).unwrap();
    }
    std::fs::write(root.path().join("outside"), b"kept").unwrap();
    std::os::unix::fs::symlink(root.path().join("outside"), folder.join("link")).unwrap();
    std::os::unix::fs::symlink(root.path(), folder.join("sub/up")).unwrap();
    (root, folder)
}

fn parent(path: &Path) -> File {
    File::open(path.parent().unwrap()).unwrap()
}

fn walk(path: &Path) -> usize {
    std::fs::read_dir(path)
        .unwrap()
        .map(|entry| {
            let entry = entry.unwrap();
            1 + if entry.file_type().unwrap().is_dir() { walk(&entry.path()) } else { 0 }
        })
        .sum()
}

#[test]
fn a_tree_goes_without_following_its_links() {
    for workers in [1, 4] {
        let (root, folder) = tree();
        let checks = AtomicUsize::new(0);
        remove(&parent(&folder), &folder, workers, &|| {
            checks.fetch_add(1, Ordering::Relaxed);
            Ok(())
        })
        .unwrap();
        assert!(!folder.exists());
        assert_eq!(std::fs::read(root.path().join("outside")).unwrap(), b"kept");
        // One check per item and per folder emptied: pause and cancel reach every step.
        assert!(checks.load(Ordering::Relaxed) >= 15, "{workers}: {checks:?}");
    }
}

#[test]
fn cancellation_stops_between_items_and_keeps_the_rest() {
    for workers in [1, 4] {
        let (_root, folder) = tree();
        let checks = AtomicUsize::new(0);
        let error = remove(&parent(&folder), &folder, workers, &|| {
            // Five folders at most are entered first: the sixth step always removes something.
            if checks.fetch_add(1, Ordering::Relaxed) >= 6 {
                Err(io::Error::new(io::ErrorKind::Interrupted, "Operation cancelled"))
            } else {
                Ok(())
            }
        })
        .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::Interrupted);
        let remaining = walk(&folder);
        assert!(remaining > 0 && remaining < 12, "{workers}: {remaining}");
    }
}

#[test]
fn files_and_links_are_removed_alone() {
    let (root, folder) = tree();
    remove(&parent(&folder.join("link")), &folder.join("link"), 4, &|| Ok(())).unwrap();
    remove(&parent(&folder.join("a")), &folder.join("a"), 4, &|| Ok(())).unwrap();
    assert!(!folder.join("a").exists() && std::fs::symlink_metadata(folder.join("link")).is_err());
    assert!(root.path().join("outside").exists() && folder.join("sub").exists());
}

#[test]
fn an_unreadable_folder_stops_the_deletion_with_its_name() {
    if rustix::process::geteuid().is_root() { return }
    for workers in [1, 4] {
        let (_root, folder) = tree();
        let deep = folder.join("sub/deep");
        std::fs::set_permissions(&deep, std::fs::Permissions::from_mode(0o000)).unwrap();
        let error = remove(&parent(&folder), &folder, workers, &|| Ok(())).unwrap_err();
        std::fs::set_permissions(&deep, std::fs::Permissions::from_mode(0o700)).unwrap();
        assert_eq!(error.kind(), io::ErrorKind::PermissionDenied, "{workers}");
        assert!(error.to_string().contains("deep"), "{error}");
        assert_eq!(std::fs::read(deep.join("c")).unwrap(), [2]);
    }
}

#[test]
fn only_btrfs_removes_one_folder_at_a_time() {
    let here = std::env::temp_dir();
    let btrfs = rustix::fs::statfs(&here).unwrap().f_type as i64 == BTRFS;
    assert_eq!(parallel(&here.join("item")), !btrfs);
    assert!(parallel(Path::new("/proc/self")));
}

/// A file system mounted inside the tree: `OXYDE_TEST_MOUNTED=<folder>:<mount inside it>`.
#[test]
fn another_file_system_inside_is_never_entered() {
    let Some(paths) = std::env::var_os("OXYDE_TEST_MOUNTED") else { return };
    let paths: Vec<_> = std::env::split_paths(&paths).collect();
    let marker = paths[1].join("oxyde-mounted-marker");
    for workers in [1, 4] {
        std::fs::write(&marker, b"kept").unwrap();
        let error = remove(&parent(&paths[0]), &paths[0], workers, &|| Ok(())).unwrap_err();
        assert!(error.to_string().contains("not entered"), "{error}");
        assert_eq!(std::fs::read(&marker).unwrap(), b"kept");
    }
    std::fs::remove_file(marker).unwrap();
}

/// Binds `source` on `target`; only an administrator of a mount namespace can:
/// `unshare --map-root-user --mount` runs these tests as one.
fn bind(source: &Path, target: &Path) -> bool {
    use std::os::unix::ffi::OsStrExt;
    let source = std::ffi::CString::new(source.as_os_str().as_bytes()).unwrap();
    let target = std::ffi::CString::new(target.as_os_str().as_bytes()).unwrap();
    // SAFETY: two paths; no file system type nor data for a bind mount.
    unsafe { libc::mount(source.as_ptr(), target.as_ptr(), std::ptr::null(), libc::MS_BIND, std::ptr::null()) == 0 }
}

fn unbind(target: &Path) {
    use std::os::unix::ffi::OsStrExt;
    let target = std::ffi::CString::new(target.as_os_str().as_bytes()).unwrap();
    // SAFETY: a path mounted by `bind`.
    unsafe { libc::umount2(target.as_ptr(), libc::MNT_DETACH) };
}

#[test]
fn a_bind_mount_inside_is_never_entered() {
    for workers in [1, 4] {
        let (root, folder) = tree();
        let shown = root.path().join("shown");
        std::fs::create_dir(&shown).unwrap();
        std::fs::write(shown.join("data"), b"kept").unwrap();
        let mount = folder.join("sub/mount");
        std::fs::create_dir(&mount).unwrap();
        if !bind(&shown, &mount) {
            return;
        }
        let error = remove(&parent(&folder), &folder, workers, &|| Ok(())).unwrap_err();
        unbind(&mount);
        assert!(error.to_string().contains("not entered"), "{workers}: {error}");
        assert_eq!(std::fs::read(shown.join("data")).unwrap(), b"kept");
    }
}

#[test]
fn a_selected_mount_point_is_not_emptied() {
    for workers in [1, 4] {
        let root = tempfile::tempdir().unwrap();
        let shown = root.path().join("shown");
        std::fs::create_dir_all(shown.join("inner")).unwrap();
        std::fs::write(shown.join("inner/data"), b"kept").unwrap();
        let mount = root.path().join("mount");
        std::fs::create_dir(&mount).unwrap();
        if !bind(&shown, &mount) {
            return;
        }
        let error = remove(&parent(&mount), &mount, workers, &|| Ok(())).unwrap_err();
        unbind(&mount);
        assert!(error.to_string().contains("not entered"), "{workers}: {error}");
        assert_eq!(std::fs::read(shown.join("inner/data")).unwrap(), b"kept");
    }
}

#[test]
fn an_entry_the_listing_passed_is_found_by_another_pass() {
    for workers in [1, 4] {
        let (_root, folder) = tree();
        let deep = folder.join("sub/deep");
        let planted = AtomicBool::new(false);
        remove(&parent(&folder), &folder, workers, &|| {
            // Once deep is gone, sub gains a file its listing has most likely read past;
            // another worker may remove sub meanwhile, and a later call tries again.
            if !deep.exists() && !planted.load(Ordering::Relaxed) && std::fs::write(folder.join("sub/late"), b"late").is_ok() {
                planted.store(true, Ordering::Relaxed);
            }
            Ok(())
        })
        .unwrap();
        assert!(planted.load(Ordering::Relaxed), "{workers}");
        assert!(!folder.exists(), "{workers}");
    }
}
