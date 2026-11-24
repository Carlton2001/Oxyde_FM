use super::*;

#[cfg(not(target_os = "windows"))]
fn points() -> Vec<String> {
    ["/", "/home", "/run/media/user/Key"].map(String::from).to_vec()
}

#[cfg(not(target_os = "windows"))]
#[::core::prelude::v1::test]
fn the_deepest_mount_point_owns_a_path() {
    let points = points();
    assert_eq!(mount_of(&points, "/"), Some("/"));
    assert_eq!(mount_of(&points, "/home"), Some("/home"));
    assert_eq!(mount_of(&points, "/home/user/file"), Some("/home"));
    assert_eq!(mount_of(&points, "/homework/file"), Some("/"));
    assert_eq!(mount_of(&points, "/run/media/user/Key/a"), Some("/run/media/user/Key"));
    assert_eq!(mount_of(&points, "relative"), None);
}

#[cfg(not(target_os = "windows"))]
#[::core::prelude::v1::test]
fn drops_across_mount_points_differ() {
    let points = points();
    assert!(!volumes_differ(&points, "/home/user/a", "/home/user/b"));
    assert!(volumes_differ(&points, "/home/user/a", "/run/media/user/Key"));
    assert!(volumes_differ(&points, "/etc/a", "/home/user"));
    assert!(!volumes_differ(&points, "/etc/a", "/homework"));
}

#[cfg(target_os = "windows")]
#[::core::prelude::v1::test]
fn windows_folder_mounts_and_spellings_resolve_to_one_volume() {
    let points = [r"C:\", r"C:\Data\", r"D:\"].map(String::from).to_vec();
    assert_eq!(mount_of(&points, r"c:\data\file"), Some(r"C:\Data\"));
    assert_eq!(mount_of(&points, r"\\?\C:\Data\file"), Some(r"C:\Data\"));
    assert_eq!(mount_of(&points, r"C:\Database\file"), Some(r"C:\"));
    assert_eq!(mount_of(&points, r"\\server\share\file"), None);
    assert!(volumes_differ(&points, r"C:\Users\a", r"C:\Data\b"));
    assert!(!volumes_differ(&points, r"C:\Data\a", r"c:/data/b"));
    assert!(!volumes_differ(&points, r"C:\Users\a", r"C:\Database"));
}

#[cfg(target_os = "windows")]
#[::core::prelude::v1::test]
fn windows_roots_ignore_the_extended_prefix_and_case() {
    assert!(!roots_differ(r"\\?\C:\a", r"c:\b"));
    assert!(!roots_differ(r"\\?\UNC\Server\Share\a", r"\\server\share\b"));
    assert!(roots_differ(r"\\server\share\a", r"\\server\other\b"));
    assert!(roots_differ(r"C:\a", r"D:\a"));
}
