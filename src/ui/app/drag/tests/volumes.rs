use super::*;

fn points() -> Vec<String> {
    ["/", "/home", "/run/media/user/Key"].map(String::from).to_vec()
}

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

#[::core::prelude::v1::test]
fn drops_across_mount_points_differ() {
    let points = points();
    assert!(!volumes_differ(&points, "/home/user/a", "/home/user/b"));
    assert!(volumes_differ(&points, "/home/user/a", "/run/media/user/Key"));
    assert!(volumes_differ(&points, "/etc/a", "/home/user"));
    assert!(!volumes_differ(&points, "/etc/a", "/homework"));
}
