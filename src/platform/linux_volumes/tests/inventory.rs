use super::*;

fn drive(removable: bool, optical: bool, usb: bool) -> Drive {
    Drive { object: String::new(), removable, optical, usb, ejectable: removable, can_power_off: usb }
}

fn internal() -> Drive {
    drive(false, false, false)
}

fn usb_key() -> Drive {
    drive(true, false, true)
}

fn block(device: &str, label: &str, hint_ignore: bool, mounts: Option<&[&str]>, drive: Drive) -> Block {
    Block {
        object: format!("/org/freedesktop/UDisks2/block_devices/{}", device.trim_start_matches("/dev/")),
        device: device.into(),
        label: label.into(),
        id_type: String::new(),
        id_version: String::new(),
        hint_ignore,
        mount_points: mounts.map(|points| points.iter().map(|point| point.to_string()).collect()),
        drive: Some(drive),
    }
}

fn session() -> Session {
    Session { home: "/home/user".into(), user: "user".into() }
}

/// A Fedora machine with a Windows disk and a live USB key, as UDisks2 reports it.
fn machine() -> Vec<Block> {
    vec![
        block("/dev/nvme0n1p1", "BigSpace", false, Some(&[]), internal()),
        block("/dev/nvme1n1p1", "", true, Some(&["/boot/efi"]), internal()),
        block("/dev/nvme1n1p4", "", true, Some(&[]), internal()),
        block("/dev/nvme1n1p5", "Data", false, Some(&["/run/media/user/Data"]), internal()),
        block("/dev/nvme1n1p6", "boot", false, Some(&["/boot"]), internal()),
        block("/dev/nvme1n1p7", "fedora", false, Some(&["/"]), internal()),
        block("/dev/sda", "Fedora-Live", false, None, usb_key()),
        block("/dev/sda1", "Fedora-Live", false, Some(&["/run/media/user/Fedora-Live"]), usb_key()),
        block("/dev/sda2", "BOOT", true, Some(&[]), usb_key()),
    ]
}

#[test]
fn mounted_volumes_follow_nautilus() {
    let drives = mounted(&machine(), &session());
    let shown: Vec<_> = drives
        .iter()
        .map(|drive| (drive.path.as_str(), drive.label.as_str(), drive.drive_type.as_str()))
        .collect();
    assert_eq!(
        shown,
        [
            ("/", "Root", "fixed"),
            ("/run/media/user/Data", "Data", "fixed"),
            ("/run/media/user/Fedora-Live", "Fedora-Live", "removable"),
        ]
    );
    assert!(drives[2].is_usb());
}

#[test]
fn only_mounts_the_user_owns_are_shown() {
    let session = session();
    for point in ["/", "/media/disk", "/run/media/user/Key", "/home/user/mnt/sshfs"] {
        assert!(session.shows(point), "{point}");
    }
    for point in [
        "/boot",
        "/mnt/data",
        "/run/media/other/Key",
        "/run/media/user/",
        "/home/user/.cache/mount",
        "/home/username/disk",
        "/media/.hidden",
    ] {
        assert!(!session.shows(point), "{point}");
    }
}

#[test]
fn root_stays_first_even_when_udisks2_does_not_know_it() {
    let blocks = vec![block("/dev/sdb1", "", false, Some(&["/run/media/user/4A2B"]), usb_key())];
    let drives = mounted(&blocks, &session());
    assert_eq!(drives[0].path, "/");
    assert_eq!(drives[0].label, "Root");
    assert_eq!(drives[1].label, "4A2B");
}

#[test]
fn optical_media_are_cd_drives() {
    let disc = drive(true, true, false);
    let blocks = vec![block("/dev/sr0", "DVD", false, Some(&["/run/media/user/DVD"]), disc)];
    assert_eq!(mounted(&blocks, &session())[1].drive_type, "cdrom");
}

#[test]
fn volumes_name_their_file_system_as_file_managers_do() {
    let mut data = block("/dev/nvme1n1p5", "Data", false, Some(&["/run/media/user/Data"]), internal());
    data.id_type = "ntfs".into();
    assert_eq!(mounted(&[data], &session())[1].file_system.as_deref(), Some("NTFS"));
    for (kind, version, name) in [
        ("vfat", "FAT32", Some("FAT32")),
        ("vfat", "", Some("FAT")),
        ("exfat", "1.0", Some("exFAT")),
        ("ext4", "1.0", Some("ext4")),
        ("btrfs", "1", Some("Btrfs")),
        ("iso9660", "Joliet Extension", Some("ISO 9660")),
        ("fuseblk", "", None),
        ("fuse.sshfs", "", None),
        ("", "", None),
    ] {
        assert_eq!(file_system(kind, version).as_deref(), name, "{kind}");
    }
}
