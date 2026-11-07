use super::{crumbs, typed_path, Crumb, CrumbKind};
use std::path::Path;

fn paths(crumbs: &[Crumb]) -> Vec<(CrumbKind, &str, &str)> {
    crumbs.iter().map(|crumb| (crumb.kind, crumb.path.as_str(), crumb.name.as_str())).collect()
}

#[test]
fn crumbs_start_at_home_inside_it_and_at_the_root_elsewhere() {
    assert_eq!(paths(&crumbs("/home/a/Documents/Notes", Some("/home/a"))), [
        (CrumbKind::Home, "/home/a", ""),
        (CrumbKind::Folder, "/home/a/Documents", "Documents"),
        (CrumbKind::Folder, "/home/a/Documents/Notes", "Notes"),
    ]);
    assert_eq!(paths(&crumbs("/home/a", Some("/home/a/"))), [(CrumbKind::Home, "/home/a", "")]);
    assert_eq!(paths(&crumbs("/home/ab", Some("/home/a"))), [
        (CrumbKind::Root, "/", ""),
        (CrumbKind::Folder, "/home", "home"),
        (CrumbKind::Folder, "/home/ab", "ab"),
    ]);
    assert_eq!(paths(&crumbs("/", Some("/home/a"))), [(CrumbKind::Root, "/", "")]);
    assert_eq!(paths(&crumbs("/mnt/usb", None)), [
        (CrumbKind::Root, "/", ""),
        (CrumbKind::Folder, "/mnt", "mnt"),
        (CrumbKind::Folder, "/mnt/usb", "usb"),
    ]);
}

#[test]
fn typed_paths_resolve_home_relative_names_and_dots() {
    let (folder, home) = (Path::new("/data/projects"), Some(Path::new("/home/a")));
    assert_eq!(typed_path("~", folder, home), Path::new("/home/a"));
    assert_eq!(typed_path(" ~/Music/ ", folder, home), Path::new("/home/a/Music"));
    assert_eq!(typed_path("~other", folder, home), Path::new("/data/projects/~other"));
    assert_eq!(typed_path("notes.txt", folder, home), Path::new("/data/projects/notes.txt"));
    assert_eq!(typed_path("../old/./a", folder, home), Path::new("/data/old/a"));
    assert_eq!(typed_path("/etc/../../usr", folder, home), Path::new("/usr"));
    assert_eq!(typed_path("~", folder, None), Path::new("/data/projects/~"));
}
