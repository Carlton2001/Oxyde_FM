use super::*;
use oxyde_lib::platform::WriteProtection;

fn protection(mounts: &[(&str, bool)], links: &[(&str, &str)]) -> WriteProtection {
    WriteProtection {
        mounts: mounts.iter().map(|(point, read_only)| (point.to_string(), *read_only)).collect(),
        links: links.iter().map(|(link, target)| (link.to_string(), target.to_string())).collect(),
    }
}

#[::core::prelude::v1::test]
fn a_read_only_root_leaves_volumes_mounted_on_it_writable() {
    // SteamOS: `/` refuses writes, `/home` is a writable volume the drive list hides.
    let protection = protection(&[("/", true), ("/home", false), ("/run/media/user/DVD", true)], &[]);
    assert!(on_read_only_volume("/usr/share", &protection));
    assert!(!on_read_only_volume("/home/user/Documents", &protection));
    assert!(on_read_only_volume("/run/media/user/DVD/setup", &protection));
}

#[::core::prelude::v1::test]
fn a_link_of_a_read_only_root_leads_to_its_writable_target() {
    // Fedora Atomic: `/home` is a link of the read-only `/` to the writable `/var/home`.
    let protection = protection(&[("/", true), ("/var", false)], &[("/home", "/var/home"), ("/media", "/run/media")]);
    assert!(!on_read_only_volume("/var/home/user", &protection));
    assert!(!on_read_only_volume("/home/user/Documents", &protection));
    assert!(!on_read_only_volume("/home", &protection));
    assert!(on_read_only_volume("/homes", &protection));
    assert!(on_read_only_volume("/usr", &protection));
}

#[::core::prelude::v1::test]
fn a_read_only_volume_mounted_in_a_writable_one_stays_read_only() {
    let protection = protection(&[("/", false), ("/mnt/iso", true)], &[]);
    assert!(on_read_only_volume("/mnt/iso/boot", &protection));
    assert!(!on_read_only_volume("/mnt/isos", &protection));
    assert!(!on_read_only_volume("/home", &protection));
}

#[::core::prelude::v1::test]
fn a_path_off_every_known_mount_is_writable() {
    assert!(!on_read_only_volume("/data", &WriteProtection::default()));
}

#[::core::prelude::v1::test]
fn a_folder_the_user_cannot_write_refuses_its_entries_but_not_its_subfolders_contents() {
    let denied = Some("/etc");
    assert!(in_denied_folder(denied, "/etc"));
    assert!(in_denied_folder(denied, "/etc/hosts"));
    assert!(!in_denied_folder(denied, "/etc/ssh/sshd_config"));
    assert!(!in_denied_folder(denied, "/etcetera"));
    assert!(!in_denied_folder(None, "/etc/hosts"));
}
