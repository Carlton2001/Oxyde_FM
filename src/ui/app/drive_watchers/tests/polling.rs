use super::*;
use oxyde_lib::models::DriveInfo;

fn drive(path: &str, drive_type: &str) -> DriveInfo {
    DriveInfo {
        path: path.into(),
        label: String::new(),
        drive_type: drive_type.into(),
        total_bytes: 0,
        free_bytes: 0,
        media_type: None,
        physical_id: None,
        remote_path: None,
        file_system: None,
        volume_bytes: 0,
    }
}

fn polls(path: &str, drives: &[DriveInfo]) -> bool {
    drive_of(path, drives).is_some_and(|drive| oxyde_lib::platform::platform().volumes().polls(drive))
}

#[::core::prelude::v1::test]
fn only_network_shares_are_polled_on_linux() {
    let drives = [
        drive("/", "fixed"),
        drive("/run/media/user/Data", "fixed"),
        drive("/run/media/user/Key", "removable"),
        drive("/home/user/mnt/share", "remote"),
    ];
    assert_eq!(drive_of("/run/media/user/Key/photos", &drives).unwrap().path, "/run/media/user/Key");
    assert_eq!(drive_of("/home/user/Documents", &drives).unwrap().path, "/");
    for path in ["/", "/home/user/Documents", "/run/media/user/Data/Tests", "/run/media/user/Key/photos"] {
        assert!(!polls(path, &drives), "{path}");
    }
    assert!(polls("/home/user/mnt/share", &drives));
    assert!(polls("/home/user/mnt/share/docs", &drives));
    assert!(!polls("/home/user/mnt/shared", &drives));
}
