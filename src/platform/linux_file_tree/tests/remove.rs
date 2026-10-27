use super::*;
use std::cell::Cell;
use std::os::unix::fs::PermissionsExt;

fn tree() -> (tempfile::TempDir, PathBuf) {
    let root = tempfile::tempdir().unwrap();
    let folder = root.path().join("folder");
    std::fs::create_dir_all(folder.join("sub/deep")).unwrap();
    for (index, path) in ["a", "sub/b", "sub/deep/c", "sub/deep/d"].iter().enumerate() {
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

#[test]
fn a_tree_goes_without_following_its_links() {
    let (root, folder) = tree();
    let checks = Cell::new(0);
    remove(&parent(&folder), &folder, &|| {
        checks.set(checks.get() + 1);
        Ok(())
    })
    .unwrap();
    assert!(!folder.exists());
    assert_eq!(std::fs::read(root.path().join("outside")).unwrap(), b"kept");
    // One check per item and per folder emptied: pause and cancel reach every step.
    assert!(checks.get() >= 10, "{}", checks.get());
}

#[test]
fn cancellation_stops_between_items_and_keeps_the_rest() {
    let (_root, folder) = tree();
    let checks = Cell::new(0);
    let error = remove(&parent(&folder), &folder, &|| {
        checks.set(checks.get() + 1);
        // At most three folders are entered first: the fourth step always removes something.
        if checks.get() > 4 { Err(io::Error::new(io::ErrorKind::Interrupted, "Operation cancelled")) } else { Ok(()) }
    })
    .unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::Interrupted);
    assert!(folder.exists());
    let remaining = walk(&folder);
    assert!(remaining > 0 && remaining < 8, "{remaining}");
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
fn files_and_links_are_removed_alone() {
    let (root, folder) = tree();
    remove(&parent(&folder.join("link")), &folder.join("link"), &|| Ok(())).unwrap();
    remove(&parent(&folder.join("a")), &folder.join("a"), &|| Ok(())).unwrap();
    assert!(!folder.join("a").exists() && std::fs::symlink_metadata(folder.join("link")).is_err());
    assert!(root.path().join("outside").exists() && folder.join("sub").exists());
}

#[test]
fn an_unreadable_folder_stops_the_deletion_with_its_name() {
    if rustix::process::geteuid().is_root() { return }
    let (_root, folder) = tree();
    let deep = folder.join("sub/deep");
    std::fs::set_permissions(&deep, std::fs::Permissions::from_mode(0o000)).unwrap();
    let error = remove(&parent(&folder), &folder, &|| Ok(())).unwrap_err();
    std::fs::set_permissions(&deep, std::fs::Permissions::from_mode(0o700)).unwrap();
    assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
    assert!(error.to_string().contains("deep"), "{error}");
    assert_eq!(std::fs::read(deep.join("c")).unwrap(), [2]);
}

/// A file system mounted inside the tree: `OXYDE_TEST_MOUNTED=<folder>:<mount inside it>`.
#[test]
fn another_file_system_inside_is_never_entered() {
    let Some(paths) = std::env::var_os("OXYDE_TEST_MOUNTED") else { return };
    let paths: Vec<_> = std::env::split_paths(&paths).collect();
    let marker = paths[1].join("oxyde-mounted-marker");
    std::fs::write(&marker, b"kept").unwrap();
    let error = remove(&parent(&paths[0]), &paths[0], &|| Ok(())).unwrap_err();
    assert!(error.to_string().contains("not entered"), "{error}");
    assert_eq!(std::fs::read(&marker).unwrap(), b"kept");
    std::fs::remove_file(marker).unwrap();
}
