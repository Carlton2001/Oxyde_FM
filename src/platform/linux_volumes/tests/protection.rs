use super::*;

fn entry(point: &str, read_only: bool) -> MountEntry {
    MountEntry { point: point.into(), fs_type: "btrfs".into(), source: "/dev/vda3".into(), read_only }
}

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

#[test]
fn hidden_mounts_keep_their_own_flag_and_the_last_mount_on_a_point_wins() {
    // Fedora Atomic: a read-only `/`, writable `/var` and `/var/home` the list hides.
    let table = [entry("/", true), entry("/var", false), entry("/var/home", false), entry("/mnt/iso", false),
        entry("/mnt/iso", true)];
    let protection = write_protection(&table, &[]);
    assert_eq!(protection.mounts, [
        ("/".to_string(), true),
        ("/var".to_string(), false),
        ("/var/home".to_string(), false),
        ("/mnt/iso".to_string(), true),
    ]);
    assert!(protection.links.is_empty());
}

#[test]
fn links_are_read_only_at_the_top_of_a_read_only_drive() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().to_string_lossy().into_owned();
    std::fs::create_dir(root.path().join("var")).unwrap();
    std::os::unix::fs::symlink("var/home", root.path().join("home")).unwrap();
    std::os::unix::fs::symlink("/run/media", root.path().join("media")).unwrap();
    std::os::unix::fs::symlink("../../outside", root.path().join("up")).unwrap();

    let writable = write_protection(&[entry(&path, false)], &[drive(&path, "fixed")]);
    assert!(writable.links.is_empty());
    let remote = write_protection(&[entry(&path, true)], &[drive(&path, "remote")]);
    assert!(remote.links.is_empty());

    let mut links = write_protection(&[entry(&path, true)], &[drive(&path, "fixed")]).links;
    links.sort();
    let outside = lexical(&root.path().join("../../outside")).unwrap();
    let mut expected = vec![
        (format!("{path}/home"), format!("{path}/var/home")),
        (format!("{path}/media"), "/run/media".to_string()),
        (format!("{path}/up"), outside.to_string_lossy().into_owned()),
    ];
    expected.sort();
    assert_eq!(links, expected);
}

#[test]
fn lexical_targets_stop_at_the_root() {
    assert_eq!(lexical(Path::new("/usr/../var/./usrlocal")), Some(PathBuf::from("/var/usrlocal")));
    assert_eq!(lexical(Path::new("/../../etc")), Some(PathBuf::from("/etc")));
    assert_eq!(lexical(Path::new("relative")), None);
}
