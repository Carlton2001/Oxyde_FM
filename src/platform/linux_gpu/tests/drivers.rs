use super::*;

#[test]
fn a_hybrid_laptop_disables_the_idle_discrete_gpu() {
    assert_eq!(idle_drivers(&[(0x10de, false), (0x8086, true)]).as_deref(), Some("*nvidia*,*nouveau*"));
    assert_eq!(idle_drivers(&[(0x8086, true), (0x1002, false)]).as_deref(), Some("*radeon*,*amd*"));
}

#[test]
fn every_needed_gpu_is_kept() {
    assert_eq!(idle_drivers(&[(0x10de, true), (0x8086, true)]), None);
    assert_eq!(idle_drivers(&[(0x10de, true)]), None);
    assert_eq!(idle_drivers(&[(0x10de, true), (0x8086, false)]).as_deref(), Some("*intel*"));
    // One needed card keeps the drivers its vendor's other cards share.
    assert_eq!(idle_drivers(&[(0x8086, true), (0x8086, false)]), None);
}

#[test]
fn unknown_use_changes_nothing() {
    assert_eq!(idle_drivers(&[]), None);
    assert_eq!(idle_drivers(&[(0x10de, false), (0x8086, false)]), None);
}

/// A sysfs tree with the given cards (name, vendor, runtime state), each on its own PCI
/// device, and their render nodes under `dev/char`.
fn sysfs(cards: &[(&str, &str, &str)]) -> tempfile::TempDir {
    let root = tempfile::tempdir().unwrap();
    for (index, (card, vendor, status)) in cards.iter().enumerate() {
        let device = root.path().join(format!("devices/0000:0{index}:00.0"));
        fs::create_dir_all(device.join("power")).unwrap();
        fs::write(device.join("vendor"), format!("{vendor}\n")).unwrap();
        fs::write(device.join("power/runtime_status"), format!("{status}\n")).unwrap();
        fs::create_dir_all(root.path().join("drm").join(card)).unwrap();
        std::os::unix::fs::symlink(&device, root.path().join("drm").join(card).join("device")).unwrap();
        let node = root.path().join(format!("char/226:{}", 128 + index));
        fs::create_dir_all(&node).unwrap();
        std::os::unix::fs::symlink(&device, node.join("device")).unwrap();
    }
    root
}

#[test]
fn cards_and_their_power_state_are_read_from_sysfs() {
    let root = sysfs(&[("card1", "0x8086", "active"), ("card0", "0x10de", "suspended")]);
    for other in ["card0-eDP-1", "renderD128", "version"] {
        fs::create_dir(root.path().join("drm").join(other)).unwrap();
    }
    let cards = read_cards(&root.path().join("drm"));
    let read: Vec<(&str, u16, bool)> = cards.iter().map(|card| (card.name.as_str(), card.vendor, card.asleep)).collect();
    assert_eq!(read, [("card0", 0x10de, true), ("card1", 0x8086, false)]);
}

#[test]
fn without_the_compositor_only_sleeping_gpus_lose_their_drivers() {
    let root = sysfs(&[("card0", "0x10de", "suspended"), ("card1", "0x8086", "active")]);
    let cards = read_cards(&root.path().join("drm"));
    assert_eq!(idle_drivers(&needed(&cards, None)).as_deref(), Some("*nvidia*,*nouveau*"));
    // Awake, as when something else uses it: kept, since the desktop may render with it.
    let root = sysfs(&[("card0", "0x10de", "active"), ("card1", "0x8086", "active")]);
    let cards = read_cards(&root.path().join("drm"));
    assert_eq!(idle_drivers(&needed(&cards, None)), None);
}

#[test]
fn the_gpu_the_compositor_names_is_the_only_one_kept() {
    // A laptop whose desktop renders with the discrete GPU while the panel stays wired to
    // the integrated one: the integrated GPU is awake, yet its drivers go.
    let root = sysfs(&[("card0", "0x10de", "active"), ("card1", "0x8086", "active")]);
    let cards = read_cards(&root.path().join("drm"));
    let rendering = card_of_node(&cards, &root.path().join("char"), "226:128");
    assert_eq!(rendering.map(|card| card.name.as_str()), Some("card0"));
    assert_eq!(idle_drivers(&needed(&cards, rendering)).as_deref(), Some("*intel*"));
    // The usual case: the integrated GPU renders, the discrete one goes even when awake.
    let rendering = card_of_node(&cards, &root.path().join("char"), "226:129");
    assert_eq!(idle_drivers(&needed(&cards, rendering)).as_deref(), Some("*nvidia*,*nouveau*"));
    // A node of no known card leaves the choice to the power state.
    assert_eq!(card_of_node(&cards, &root.path().join("char"), "226:200"), None);
}
