use super::*;

fn tree() -> tempfile::TempDir {
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(root.path().join("a/inner")).unwrap();
    std::fs::write(root.path().join("note.txt"), b"text").unwrap();
    std::os::unix::fs::symlink(root.path().join("a"), root.path().join("docs")).unwrap();
    std::os::unix::fs::symlink(root.path().join("note.txt"), root.path().join("note")).unwrap();
    std::os::unix::fs::symlink(root.path().join("gone"), root.path().join("dangling")).unwrap();
    root
}

#[test]
fn a_link_to_a_folder_is_a_node_with_its_arrow() {
    let root = tree();
    let nodes = get_sidebar_nodes_blocking(root.path().to_string_lossy().into_owned(), &Default::default()).unwrap();
    let names: Vec<_> = nodes.iter().map(|node| (node.name.as_str(), node.has_subdirs)).collect();
    assert_eq!(names, [("a", true), ("docs", true)]);
}

#[test]
fn expand_all_does_not_walk_into_a_link() {
    let root = tree();
    let found = get_subtree_nodes_blocking(root.path().to_path_buf(), true, true, &AtomicBool::new(false)).unwrap();
    let top = &found[&root.path().to_string_lossy().into_owned()];
    let names: Vec<_> = top.iter().map(|node| (node.name.as_str(), node.has_subdirs)).collect();
    assert_eq!(names, [("a", true), ("docs", true)]);
    assert!(found.contains_key(&root.path().join("a").to_string_lossy().into_owned()));
    assert!(!found.contains_key(&root.path().join("docs").to_string_lossy().into_owned()));
}

#[test]
fn a_known_folder_keeps_its_arrow_without_being_opened() {
    let root = tree();
    let a = root.path().join("a").to_string_lossy().into_owned();
    let known = std::collections::HashMap::from([(a, false)]);
    let nodes = get_sidebar_nodes_blocking(root.path().to_string_lossy().into_owned(), &known).unwrap();
    let names: Vec<_> = nodes.iter().map(|node| (node.name.as_str(), node.has_subdirs)).collect();
    // « a » holds a folder, but its known arrow is kept; « docs » is opened.
    assert_eq!(names, [("a", false), ("docs", true)]);
}
