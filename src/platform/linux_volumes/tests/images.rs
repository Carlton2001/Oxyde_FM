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
        MountedDiskImage { image_path: "/home/me/disc.iso".into(), roots: vec!["/run/media/me/DISC".into()] },
        MountedDiskImage { image_path: "/home/me/card.img".into(), roots: vec!["/run/media/me/boot".into()] },
    ]);
    assert_eq!(image_at(&blocks, "/run/media/me/boot").as_deref(), Some("/home/me/card.img"));
    assert_eq!(image_at(&blocks, "/run/media/me/DISC").as_deref(), Some("/home/me/disc.iso"));
    assert_eq!(image_at(&blocks, "/"), None);
    let objects: Vec<&str> = volumes(&blocks, LOOP1).iter().map(|block| block.object.as_str()).collect();
    assert_eq!(objects, ["/org/freedesktop/UDisks2/block_devices/loop1p1", "/org/freedesktop/UDisks2/block_devices/loop1p2"]);
}
