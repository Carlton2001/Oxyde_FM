use super::*;
use std::os::unix::fs::PermissionsExt;

#[test]
fn a_sticky_folder_guards_entries_only_from_users_who_own_neither_it_nor_them() {
    assert!(sticky_guards(0o1777, 0, 1000));
    assert!(!sticky_guards(0o1777, 1000, 1000), "the folder's owner removes any entry");
    assert!(!sticky_guards(0o1777, 0, 0), "root removes any entry");
    assert!(!sticky_guards(0o777, 0, 1000), "without the sticky bit, write access decides");
}

#[test]
fn a_folder_of_ones_own_guards_no_entry() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("file"), b"").unwrap();
    std::fs::set_permissions(root.path(), std::fs::Permissions::from_mode(0o1777)).unwrap();
    assert!(undeletable_entries(root.path()).is_empty());
}

#[test]
fn only_subfolders_the_user_cannot_write_are_listed() {
    // The kernel never refuses root.
    if rustix::process::geteuid().is_root() {
        return;
    }
    let root = tempfile::tempdir().unwrap();
    let (open, locked) = (root.path().join("open"), root.path().join("locked"));
    std::fs::create_dir(&open).unwrap();
    std::fs::create_dir(&locked).unwrap();
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o555)).unwrap();
    let paths = [&open, &locked, &root.path().join("missing")]
        .map(|path| path.to_string_lossy().into_owned());
    let denied = denied_subfolders(root.path(), paths.to_vec());
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert_eq!(denied, [paths[1].clone()]);
    assert!(!denies_writes(root.path()));
}
