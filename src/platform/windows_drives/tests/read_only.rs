use super::*;

#[test]
fn the_system_drive_accepts_writes() {
    let system = std::env::var("SystemDrive").unwrap_or_else(|_| "C:".into());
    assert!(!volume_is_read_only(std::path::Path::new(&system)));
}

#[test]
fn a_mounted_disk_image_refuses_writes() {
    // Set up by hand: mount an ISO and set OXYDE_TEST_ISO_ROOT to its root or a SUBST of it.
    if let Ok(root) = std::env::var("OXYDE_TEST_ISO_ROOT") {
        assert!(volume_is_read_only(std::path::Path::new(&root)));
    }
}
