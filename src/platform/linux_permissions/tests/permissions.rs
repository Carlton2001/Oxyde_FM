use super::*;
use crate::models::permissions::{with_executable, Access, PermissionClass};
use crate::models::permissions::Levels;
use std::os::unix::fs::{MetadataExt, PermissionsExt};

fn write(path: &Path, change: &PermissionChange) -> Result<(), PermissionError> {
    super::write(path, change, &AtomicBool::new(false))
}

fn set_mode(path: &Path, mode: u32) {
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)).unwrap();
}

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
    let user = if rustix::process::geteuid().is_root() { "root".into() } else { std::env::var("USER").unwrap() };
    assert_eq!(permissions.owner.name, user);
    assert!(permissions.groups.iter().any(|group| group.id == permissions.group.id));
    assert!(permissions.group.name.parse::<u32>().is_err(), "{:?}", permissions.group);
    // Only root may give the item to another account.
    assert_eq!(permissions.owners.is_empty(), !rustix::process::geteuid().is_root());
}

#[test]
fn a_change_applies_levels_and_execution_to_the_object_read() {
    let (_root, path) = file(0o640);
    let expected = read(&path).unwrap();
    let mode_wanted = with_executable(Access::ReadOnly.apply(expected.mode, false, PermissionClass::Others), true);
    write(&path, &PermissionChange { mode: mode_wanted, ..PermissionChange::of(expected) }).unwrap();
    assert_eq!(mode(&path), 0o755);
}

#[test]
fn a_change_after_another_change_or_a_replacement_is_refused() {
    let (root, path) = file(0o640);
    let expected = read(&path).unwrap();
    let change = PermissionChange { mode: 0o600, ..PermissionChange::of(expected) };
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
    assert_eq!(write(&path, &change), Err(PermissionError::Changed));
    assert_eq!(mode(&path), 0o644);

    let expected = read(&path).unwrap();
    let change = PermissionChange { mode: 0o600, ..PermissionChange::of(expected) };
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
    write(&link, &PermissionChange { mode: 0o640, ..PermissionChange::of(expected) }).unwrap();
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
    write(&folder, &PermissionChange { mode: wanted, ..PermissionChange::of(expected) }).unwrap();
    assert_eq!(mode(&folder), 0o3770);
}

/// Needs a second group: `OXYDE_TEST_GROUP=<gid>` among the user's groups.
#[test]
fn the_group_changes_to_one_of_the_users_groups() {
    let Some(gid) = std::env::var("OXYDE_TEST_GROUP").ok().and_then(|gid| gid.parse::<u32>().ok()) else { return };
    let (_root, path) = file(0o2750);
    let expected = read(&path).unwrap();
    assert!(expected.groups.iter().any(|group| group.id == gid));
    write(&path, &PermissionChange { group: gid, ..PermissionChange::of(expected) }).unwrap();
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
    assert_eq!(write(&path, &PermissionChange { mode: wanted, ..PermissionChange::of(expected) }), Err(PermissionError::NotKept));
}

fn tree() -> (tempfile::TempDir, std::path::PathBuf) {
    let root = tempfile::tempdir().unwrap();
    let folder = root.path().join("folder");
    let deep = folder.join("sub/deep");
    std::fs::create_dir_all(&deep).unwrap();
    std::fs::write(folder.join("a.txt"), b"a").unwrap();
    std::fs::write(folder.join("sub/b.sh"), b"b").unwrap();
    std::fs::write(deep.join("c"), b"c").unwrap();
    std::fs::write(root.path().join("outside"), b"o").unwrap();
    std::os::unix::fs::symlink(root.path().join("outside"), folder.join("link")).unwrap();
    for (path, mode) in [("folder", 0o755), ("folder/a.txt", 0o644), ("folder/sub", 0o755), ("folder/sub/b.sh", 0o755),
        ("folder/sub/deep", 0o700), ("folder/sub/deep/c", 0o600), ("outside", 0o666)] {
        set_mode(&root.path().join(path), mode);
    }
    (root, folder)
}

fn enclosed(folder: &Path, change: Levels) -> Result<(), PermissionError> {
    write(folder, &PermissionChange { enclosed: change, ..PermissionChange::of(read(folder).unwrap()) })
}

#[test]
fn enclosed_items_change_at_every_depth_without_following_links() {
    use Access::{AccessFiles, None as NoAccess, ReadOnly};
    let (root, folder) = tree();
    enclosed(&folder, Levels {
        files: [None, Some(ReadOnly), Some(NoAccess)],
        folders: [None, Some(AccessFiles), Some(NoAccess)],
    })
    .unwrap();
    for (path, wanted) in [("folder", 0o755), ("folder/a.txt", 0o640), ("folder/sub", 0o750), ("folder/sub/b.sh", 0o751),
        ("folder/sub/deep", 0o750), ("folder/sub/deep/c", 0o640), ("outside", 0o666)] {
        assert_eq!(mode(&root.path().join(path)), wanted, "{path}");
    }
}

#[test]
fn a_folder_closing_to_its_owner_is_changed_after_its_content() {
    let (root, folder) = tree();
    enclosed(&folder, Levels {
        files: [Some(Access::ReadOnly), None, None],
        folders: [Some(Access::ListFiles), None, None],
    })
    .unwrap();
    assert_eq!(mode(&root.path().join("folder/sub")), 0o455);
    for path in ["folder/sub", "folder/sub/deep"] {
        set_mode(&root.path().join(path), 0o755);
    }
    assert_eq!(mode(&root.path().join("folder/sub/deep/c")), 0o400);
}

#[test]
fn a_folder_closed_to_its_owner_is_opened_then_walked() {
    let (root, folder) = tree();
    set_mode(&root.path().join("folder/sub"), 0o000);
    enclosed(&folder, Levels {
        files: [None, None, Some(Access::ReadOnly)],
        folders: [Some(Access::CreateDelete), None, None],
    })
    .unwrap();
    assert_eq!(mode(&root.path().join("folder/sub")), 0o700);
    assert_eq!(mode(&root.path().join("folder/sub/deep/c")), 0o604);
}

#[test]
fn enclosed_failures_are_counted_and_the_rest_still_changes() {
    if rustix::process::geteuid().is_root() { return }
    let (root, folder) = tree();
    // A folder no one may enter, and that the change does not open, keeps its content.
    set_mode(&root.path().join("folder/sub/deep"), 0o000);
    let error = enclosed(&folder, Levels { files: [None, None, Some(Access::None)], ..Default::default() }).unwrap_err();
    set_mode(&root.path().join("folder/sub/deep"), 0o700);
    let PermissionError::Incomplete { failed, first } = error else { panic!("{error:?}") };
    assert_eq!(failed, 1);
    assert!(first.contains("deep"), "{first}");
    assert_eq!(mode(&root.path().join("folder/a.txt")), 0o640);
    assert_eq!(mode(&root.path().join("folder/sub/deep/c")), 0o600);
}

/// Root only: `unshare --map-auto --map-root-user` runs the tests as an administrator
/// of a user namespace, where the accounts of /etc/subuid can own files.
#[test]
fn root_gives_the_item_to_any_account_and_group() {
    if !rustix::process::geteuid().is_root() { return }
    let (_root, path) = file(0o4755);
    let expected = read(&path).unwrap();
    assert!(expected.owners.iter().any(|account| account.id == 1 && account.name.starts_with("bin")), "{:?}", expected.owners);
    assert!(expected.owners.iter().any(|account| account.id == 0 && account.name == "root – Super User"));
    assert!(expected.groups.iter().any(|group| group.name == "bin"));
    write(&path, &PermissionChange { owner: 1, group: 1, ..PermissionChange::of(expected) }).unwrap();
    let metadata = std::fs::metadata(&path).unwrap();
    assert_eq!((metadata.uid(), metadata.gid()), (1, 1));
    // The kernel clears setuid on a change of owner, root's included.
    assert_eq!(metadata.mode() & 0o7777, 0o755);
    let expected = read(&path).unwrap();
    assert_eq!(expected.owner.name, "bin");
    let stale = PermissionChange { owner: 0, ..PermissionChange::of(expected.clone()) };
    std::os::unix::fs::chown(&path, Some(2), None).unwrap();
    assert_eq!(write(&path, &stale), Err(PermissionError::Changed));
}

#[test]
fn gecos_names_follow_nautilus() {
    assert_eq!(real_name("ana", "Ana Ruiz,Office,,"), Some("Ana Ruiz".into()));
    assert_eq!(real_name("bob", "& the builder"), Some("Bob the builder".into()));
    assert_eq!(real_name("svc", ""), None);
    assert_eq!(real_name("svc", ",,,"), None);
}

#[test]
fn the_umask_read_is_the_one_new_files_receive() {
    use std::os::unix::fs::OpenOptionsExt;
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("new");
    std::fs::OpenOptions::new().write(true).create_new(true).mode(0o666).open(&path).unwrap();
    assert_eq!(mode(&path), 0o666 & !umask().unwrap());
}
