use super::*;
use std::os::unix::fs::PermissionsExt;

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
