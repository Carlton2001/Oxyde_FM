use super::*;

#[test]
fn a_hybrid_laptop_disables_the_idle_discrete_gpu() {
    assert_eq!(idle_drivers(&[(0x10de, false), (0x8086, true)]).as_deref(), Some("*nvidia*,*nouveau*"));
    assert_eq!(idle_drivers(&[(0x8086, true), (0x1002, false)]).as_deref(), Some("*radeon*,*amd*"));
}

#[test]
fn every_gpu_with_a_screen_is_kept() {
    assert_eq!(idle_drivers(&[(0x10de, true), (0x8086, true)]), None);
    assert_eq!(idle_drivers(&[(0x10de, true)]), None);
    assert_eq!(idle_drivers(&[(0x10de, true), (0x8086, false)]).as_deref(), Some("*intel*"));
}

#[test]
fn unknown_screens_change_nothing() {
    assert_eq!(idle_drivers(&[]), None);
    assert_eq!(idle_drivers(&[(0x10de, false), (0x8086, false)]), None);
}

#[test]
fn cards_and_their_connectors_are_read_from_sysfs() {
    let drm = tempfile::tempdir().unwrap();
    for (card, vendor) in [("card0", "0x10de\n"), ("card1", "0x8086\n")] {
        fs::create_dir_all(drm.path().join(card).join("device")).unwrap();
        fs::write(drm.path().join(card).join("device/vendor"), vendor).unwrap();
    }
    for (connector, status) in [("card0-DP-1", "disconnected\n"), ("card1-eDP-1", "connected\n")] {
        fs::create_dir(drm.path().join(connector)).unwrap();
        fs::write(drm.path().join(connector).join("status"), status).unwrap();
    }
    fs::create_dir(drm.path().join("renderD128")).unwrap();
    let mut gpus = read_gpus(drm.path());
    gpus.sort_unstable();
    assert_eq!(gpus, [(0x10de, false), (0x8086, true)]);
}
