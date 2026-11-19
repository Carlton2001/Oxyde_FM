use super::*;
use oxyde_lib::models::DriveInfo;

fn drive(path: &str) -> DriveInfo {
    DriveInfo {
        path: path.into(),
        label: String::new(),
        drive_type: "fixed".into(),
        total_bytes: 0,
        free_bytes: 0,
        media_type: None,
        physical_id: None,
        remote_path: None,
        file_system: None,
        volume_bytes: 0,
    }
}

#[::core::prelude::v1::test]
fn a_read_only_root_leaves_volumes_mounted_on_it_writable() {
    // SteamOS: `/` refuses writes, `/home` is a writable volume of its own.
    let drives = [drive("/"), drive("/home"), drive("/run/media/user/DVD")];
    let read_only = ["/".to_string(), "/run/media/user/DVD".to_string()];
    assert!(on_read_only_volume("/usr/share", &drives, &read_only));
    assert!(!on_read_only_volume("/home/user/Documents", &drives, &read_only));
    assert!(on_read_only_volume("/run/media/user/DVD/setup", &drives, &read_only));
}

#[::core::prelude::v1::test]
fn a_read_only_volume_mounted_in_a_writable_one_stays_read_only() {
    let drives = [drive("/"), drive("/mnt/iso")];
    let read_only = ["/mnt/iso".to_string()];
    assert!(on_read_only_volume("/mnt/iso/boot", &drives, &read_only));
    assert!(!on_read_only_volume("/mnt/isos", &drives, &read_only));
    assert!(!on_read_only_volume("/home", &drives, &read_only));
}

#[::core::prelude::v1::test]
fn a_path_off_every_known_drive_is_writable() {
    assert!(!on_read_only_volume("/data", &[], &["/".to_string()]));
}
