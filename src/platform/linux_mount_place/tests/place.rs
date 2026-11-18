use super::*;

fn place(device: (u32, u32), mount: Option<u64>) -> Place {
    Place { device, mount }
}

#[test]
fn a_bind_mount_differs_by_its_mount_alone() {
    let folder = place((8, 1), Some(30));
    assert!(folder.holds(place((8, 1), Some(30))));
    assert!(!folder.holds(place((8, 1), Some(31))));
    assert!(!folder.holds(place((8, 2), Some(30))));
    // Before Linux 5.8, the device alone.
    assert!(folder.holds(place((8, 1), None)));
    assert!(place((8, 1), Some(31)).mounted_on(folder));
}

#[test]
fn a_btrfs_subvolume_is_not_a_mount_point() {
    let home = place((0, 40), Some(30));
    let subvolume = place((0, 41), Some(30));
    assert!(!subvolume.mounted_on(home));
    assert!(!home.holds(subvolume));
    assert!(place((0, 41), None).mounted_on(place((0, 40), None)));
}

#[test]
fn a_folder_and_its_parent_share_a_place() {
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir(root.path().join("folder")).unwrap();
    let parent = std::fs::File::open(root.path()).unwrap();
    let folder = status(&parent, "folder").unwrap();
    assert_eq!(folder.kind, FileType::Directory);
    assert!(!folder.place.mounted_on(status_of(&parent).unwrap().place));
    assert!(status_of(&parent).unwrap().place.holds(folder.place));
}
