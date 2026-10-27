use super::*;
use crate::models::permissions::{with_executable, Access, PermissionClass};
use std::os::unix::fs::{MetadataExt, PermissionsExt};

fn file(mode: u32) -> (tempfile::TempDir, std::path::PathBuf) {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("script.sh");
    std::fs::write(&path, b"#!/bin/sh\n").unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(mode)).unwrap();
    (root, path)
}

fn mode(path: &Path) -> u32 {
    std::fs::metadata(path).unwrap().mode() & 0o7777
}

#[test]
fn an_owned_file_is_editable_and_named_through_nss() {
    let (_root, path) = file(0o640);
    let permissions = read(&path).unwrap();
    assert_eq!(permissions.mode, 0o640);
    assert!(permissions.editable && !permissions.is_dir);
    assert_eq!(permissions.owner.id, rustix::process::geteuid().as_raw());
    let user = std::env::var("USER").unwrap();
    assert_eq!(permissions.owner.name, user);
    assert!(permissions.groups.iter().any(|group| group.id == permissions.group.id));
    assert!(permissions.group.name.parse::<u32>().is_err(), "{:?}", permissions.group);
}

#[test]
fn a_change_applies_levels_and_execution_to_the_object_read() {
    let (_root, path) = file(0o640);
    let expected = read(&path).unwrap();
    let mode_wanted = with_executable(Access::ReadOnly.apply(expected.mode, false, PermissionClass::Others), true);
    let group = expected.group.id;
    write(&path, &PermissionChange { expected, mode: mode_wanted, group }).unwrap();
    assert_eq!(mode(&path), 0o755);
}

#[test]
fn a_change_after_another_change_or_a_replacement_is_refused() {
    let (root, path) = file(0o640);
    let expected = read(&path).unwrap();
    let change = PermissionChange { mode: 0o600, group: expected.group.id, expected };
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
    assert_eq!(write(&path, &change), Err(PermissionError::Changed));
    assert_eq!(mode(&path), 0o644);

    let expected = read(&path).unwrap();
    let change = PermissionChange { mode: 0o600, group: expected.group.id, expected };
    std::fs::rename(&path, root.path().join("old")).unwrap();
    std::fs::write(&path, b"replacement").unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
    assert_eq!(write(&path, &change), Err(PermissionError::Changed));
    assert_eq!(mode(&path), 0o644);
}

#[test]
fn a_link_shows_and_changes_its_target() {
    let (root, path) = file(0o600);
    let link = root.path().join("link");
    std::os::unix::fs::symlink(&path, &link).unwrap();
    let expected = read(&link).unwrap();
    assert_eq!(expected.mode, 0o600);
    let group = expected.group.id;
    write(&link, &PermissionChange { expected, mode: 0o640, group }).unwrap();
    assert_eq!(mode(&path), 0o640);
}

#[test]
fn special_bits_are_kept_by_a_level_change() {
    let root = tempfile::tempdir().unwrap();
    let folder = root.path().join("shared");
    std::fs::create_dir(&folder).unwrap();
    std::fs::set_permissions(&folder, std::fs::Permissions::from_mode(0o3775)).unwrap();
    let expected = read(&folder).unwrap();
    assert!(expected.is_dir);
    let wanted = Access::None.apply(expected.mode, true, PermissionClass::Others);
    let group = expected.group.id;
    write(&folder, &PermissionChange { expected, mode: wanted, group }).unwrap();
    assert_eq!(mode(&folder), 0o3770);
}

/// Needs a second group: `OXYDE_TEST_GROUP=<gid>` among the user's groups.
#[test]
fn the_group_changes_to_one_of_the_users_groups() {
    let Some(gid) = std::env::var("OXYDE_TEST_GROUP").ok().and_then(|gid| gid.parse::<u32>().ok()) else { return };
    let (_root, path) = file(0o2750);
    let expected = read(&path).unwrap();
    assert!(expected.groups.iter().any(|group| group.id == gid));
    write(&path, &PermissionChange { mode: expected.mode, group: gid, expected }).unwrap();
    let metadata = std::fs::metadata(&path).unwrap();
    assert_eq!(metadata.gid(), gid);
    // The setgid bit stays only if the kernel kept it.
    assert_eq!(metadata.mode() & 0o777, 0o750);
}

#[test]
fn an_item_of_another_user_is_read_only() {
    if rustix::process::geteuid().is_root() { return }
    let permissions = read(Path::new("/etc/passwd")).unwrap();
    assert!(!permissions.editable);
    assert_eq!((permissions.owner.name.as_str(), permissions.owner.id), ("root", 0));
}

/// ntfs-3g mounted without its `permissions` option accepts chmod and keeps nothing:
/// `OXYDE_TEST_NTFS=<mount point>`.
#[test]
fn a_file_system_that_ignores_permissions_reports_it() {
    let Some(mount) = std::env::var_os("OXYDE_TEST_NTFS") else { return };
    let root = tempfile::tempdir_in(mount).unwrap();
    let path = root.path().join("ignored.txt");
    std::fs::write(&path, b"x").unwrap();
    let expected = read(&path).unwrap();
    let wanted = Access::ReadOnly.apply(expected.mode, false, PermissionClass::Group);
    let group = expected.group.id;
    assert_eq!(write(&path, &PermissionChange { expected, mode: wanted, group }), Err(PermissionError::NotKept));
}
