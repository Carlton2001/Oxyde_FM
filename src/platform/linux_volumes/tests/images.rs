use super::*;

fn block(object: &str, usage: &str, mounts: Option<&[&str]>, backing: Option<&str>, table: Option<&str>) -> Block {
    Block {
        object: object.into(),
        device: String::new(),
        label: String::new(),
        id_type: String::new(),
        id_version: String::new(),
        id_usage: usage.into(),
        size: 0,
        hint_ignore: false,
        partitioned: table.is_none() && usage.is_empty(),
        fstab: None,
        mount_points: mounts.map(|points| points.iter().map(|point| point.to_string()).collect()),
        drive: None,
        loop_backing: backing.map(str::to_string),
        partition_of: table.map(str::to_string),
    }
}

const LOOP0: &str = "/org/freedesktop/UDisks2/block_devices/loop0";
const LOOP1: &str = "/org/freedesktop/UDisks2/block_devices/loop1";

#[test]
fn an_image_lists_the_mount_points_of_its_filesystems_or_partitions() {
    let blocks = [
        // A CD image: the filesystem is the loop device itself.
        block(LOOP0, "filesystem", Some(&["/run/media/me/DISC"]), Some("/home/me/disc.iso"), None),
        // A partitioned disk image: one partition mounted, one not, swap ignored.
        block(LOOP1, "", None, Some("/home/me/card.img"), None),
        block("/org/freedesktop/UDisks2/block_devices/loop1p1", "filesystem", Some(&["/run/media/me/boot"]), None, Some(LOOP1)),
        block("/org/freedesktop/UDisks2/block_devices/loop1p2", "filesystem", Some(&[]), None, Some(LOOP1)),
        block("/org/freedesktop/UDisks2/block_devices/loop1p3", "other", None, None, Some(LOOP1)),
        block("/org/freedesktop/UDisks2/block_devices/sda1", "filesystem", Some(&["/"]), None, None),
    ];
    let images = mounted(&blocks);
    assert_eq!(images, [
        MountedDiskImage { image_path: "/home/me/disc.iso".into(), roots: vec!["/run/media/me/DISC".into()], aliases: Vec::new() },
        MountedDiskImage { image_path: "/home/me/card.img".into(), roots: vec!["/run/media/me/boot".into()], aliases: Vec::new() },
    ]);
    assert_eq!(image_at(&blocks, "/run/media/me/boot").as_deref(), Some("/home/me/card.img"));
    assert_eq!(image_at(&blocks, "/run/media/me/DISC").as_deref(), Some("/home/me/disc.iso"));
    assert_eq!(image_at(&blocks, "/"), None);
    let objects: Vec<&str> = volumes(&blocks, LOOP1).iter().map(|block| block.object.as_str()).collect();
    assert_eq!(objects, ["/org/freedesktop/UDisks2/block_devices/loop1p1", "/org/freedesktop/UDisks2/block_devices/loop1p2"]);
}

#[test]
fn a_loop_device_is_found_by_its_backing_file_once_the_image_is_gone() {
    let blocks = [block(LOOP0, "filesystem", Some(&["/run/media/me/DISC"]), Some("/home/me/disc.iso"), None)];
    assert!(reads(&blocks[0], &[PathBuf::from("/home/me/disc.iso")]));
    assert!(!reads(&blocks[0], &[PathBuf::from("/home/me/other.iso")]));
    let deleted = [block(LOOP0, "filesystem", Some(&[]), Some("/home/me/disc.iso (deleted)"), None)];
    assert!(reads(&deleted[0], &[image_of(Path::new("/home/me/disc.iso (deleted)")).to_path_buf()]));
    assert!(reads(&deleted[0], &[PathBuf::from("/home/me/disc.iso")]));
}

/// A FAT image mounted, deleted, then unmounted by its mount point as from the drive list:
/// `sudo` with UDisks2 running, `mkfs.vfat` installed.
#[test]
#[ignore = "needs UDisks2, mkfs.vfat and the right to set up loop devices"]
fn a_deleted_image_still_unmounts() {
    let root = tempfile::tempdir().unwrap();
    let image = root.path().join("card.img");
    std::fs::File::create(&image).unwrap().set_len(8 << 20).unwrap();
    assert!(std::process::Command::new("mkfs.vfat").arg(&image).output().unwrap().status.success());
    let points = mount(&image).unwrap();
    assert_eq!(points.len(), 1);
    std::fs::remove_file(&image).unwrap();
    // UDisks2 learns of the deletion in the background, as a user's later click finds it.
    // Its cleanup may also release the image itself meanwhile: nothing is left to unmount.
    let deadline = Instant::now() + Duration::from_secs(5);
    let backing = loop {
        let blocks = udisks::blocks().unwrap();
        let backing = image_at(&blocks, &points[0]);
        if backing.as_ref().is_some_and(|backing| backing.ends_with(" (deleted)")) {
            break backing.unwrap();
        }
        if !blocks.iter().any(|block| block.loop_backing.as_deref().is_some_and(|backing| image_of(Path::new(backing)) == image)) {
            eprintln!("UDisks2 released the deleted image itself");
            return;
        }
        assert!(Instant::now() < deadline, "{backing:?}");
        std::thread::sleep(PROBE_INTERVAL);
    };
    unmount(Path::new(&backing)).unwrap();
    assert!(udisks::blocks().unwrap().iter().all(|block| block.loop_backing.as_deref() != Some(backing.as_str())));
}
