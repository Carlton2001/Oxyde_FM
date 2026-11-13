use super::deepest_root;

#[cfg(not(windows))]
#[test]
fn the_deepest_mount_owns_the_folder() {
    let roots = ["/", "/home/biluf", "/run/media/biluf/Data"];
    assert_eq!(deepest_root("/run/media/biluf/Data/TEST1", roots), Some("/run/media/biluf/Data"));
    assert_eq!(deepest_root("/run/media/biluf/Data", roots), Some("/run/media/biluf/Data"));
    assert_eq!(deepest_root("/home/biluf/Documents", roots), Some("/home/biluf"));
    assert_eq!(deepest_root("/etc", roots), Some("/"));
    // Whole components only: a sibling sharing a prefix stays on the root.
    assert_eq!(deepest_root("/run/media/biluf/Data2", roots), Some("/"));
}

#[test]
fn places_outside_every_root_light_nothing() {
    assert_eq!(deepest_root("trash://", ["/home/biluf"]), None);
}
