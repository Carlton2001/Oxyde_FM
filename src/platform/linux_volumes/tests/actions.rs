use super::*;
use crate::platform::linux_volumes::udisks::Drive;

fn drive(object: &str, optical: bool, ejectable: bool, can_power_off: bool) -> Drive {
    Drive { object: object.into(), removable: true, optical, usb: !optical, ejectable, can_power_off }
}

fn block(object: &str, mounts: Option<&[&str]>, drive: Option<Drive>) -> Block {
    Block {
        object: object.into(),
        device: String::new(),
        label: String::new(),
        id_type: String::new(),
        id_version: String::new(),
        id_usage: "filesystem".into(),
        size: 0,
        partitioned: false,
        fstab: None,
        hint_ignore: false,
        mount_points: mounts.map(|points| points.iter().map(|point| point.to_string()).collect()),
        drive,
    }
}

#[test]
fn ejecting_a_key_unmounts_all_its_partitions_then_powers_it_off() {
    let key = drive("/drives/key", false, true, true);
    let blocks = vec![
        block("/block/sdb1", Some(&["/run/media/user/KEY"]), Some(key.clone())),
        block("/block/sdb2", Some(&["/run/media/user/DATA"]), Some(key.clone())),
        block("/block/sdb3", Some(&[]), Some(key.clone())),
        block("/block/sdb", None, Some(key)),
        block("/block/nvme0n1p3", Some(&["/"]), Some(drive("/drives/nvme", false, false, false))),
    ];
    assert_eq!(release(&blocks, "/run/media/user/DATA").unwrap(), Release {
        unmount: vec!["/block/sdb1".into(), "/block/sdb2".into()],
        drive: Some(("/drives/key".into(), "PowerOff")),
    });
}

#[test]
fn optical_media_are_ejected_and_unknown_paths_rejected() {
    let blocks = vec![block("/block/sr0", Some(&["/run/media/user/DVD"]), Some(drive("/drives/dvd", true, true, false)))];
    assert_eq!(release(&blocks, "/run/media/user/DVD").unwrap().drive, Some(("/drives/dvd".into(), "Eject")));
    assert!(matches!(release(&blocks, "/run/media/user/Other"), Err(VolumeError::Other(_))));
}

#[test]
fn dismissed_authentication_and_busy_volumes_are_recognised() {
    let error = |name: &str| {
        let message = zbus::Message::method_call("/", "Unmount").unwrap().build(&()).unwrap();
        let name = zbus::names::OwnedErrorName::try_from(name.to_string()).unwrap();
        volume_error(zbus::Error::MethodError(name, Some("detail".into()), message))
    };
    assert_eq!(error("org.freedesktop.UDisks2.Error.NotAuthorizedDismissed"), VolumeError::Cancelled);
    assert_eq!(error("org.freedesktop.UDisks2.Error.DeviceBusy"), VolumeError::Busy(Vec::new()));
    assert_eq!(error("org.freedesktop.UDisks2.Error.NotAuthorizedCanObtain"), VolumeError::NotAuthorized);
    assert_eq!(error("org.freedesktop.UDisks2.Error.Failed"), VolumeError::Other("detail".into()));
}

#[test]
fn a_hibernated_windows_volume_is_recognised() {
    assert!(hibernated("Error mounting /dev/nvme1n1p3: Windows is hibernated, refused to mount."));
    assert!(hibernated("The NTFS partition is in an unsafe state. Please resume and shutdown Windows fully (no hibernation or fast restarting.)"));
    assert!(!hibernated("wrong fs type, bad option, bad superblock"));
    assert!(matches!(mount("/org/freedesktop/UDisks2/drives/key"), Err(VolumeError::Other(_))));
}
