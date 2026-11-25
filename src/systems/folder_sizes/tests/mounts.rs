use super::*;

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
fn a_folder_bound_inside_is_counted_but_not_entered() {
    let root = tempfile::tempdir().unwrap();
    let (shown, measured) = (root.path().join("shown"), root.path().join("measured"));
    std::fs::create_dir_all(shown.join("deep")).unwrap();
    std::fs::write(shown.join("deep/data"), vec![0; 4096]).unwrap();
    std::fs::create_dir_all(measured.join("mount")).unwrap();
    std::fs::write(measured.join("own"), b"own").unwrap();
    if !bind(&shown, &measured.join("mount")) {
        return;
    }
    let result = scan(
        &measured,
        ScanCancellation {
            requested: Arc::new(AtomicBool::new(false)),
            abandoned: Arc::new(AtomicBool::new(false)),
        },
        make_pool(2).unwrap(),
        Arc::new(AtomicBool::new(false)),
        || {},
    );
    unbind(&measured.join("mount"));
    let result = result.unwrap();
    assert_eq!((result.files_count, result.folders_count, result.size), (1, 1, 3));
}
