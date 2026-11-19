use super::*;

#[test]
fn expand_all_walk_stops_once_cancelled() {
    let root = tempfile::tempdir().unwrap();
    fs::create_dir_all(root.path().join("a").join("b")).unwrap();
    let walk = |cancel: bool| {
        get_subtree_nodes_blocking(root.path().to_path_buf(), true, true, &AtomicBool::new(cancel))
    };
    let tree = walk(false).unwrap();
    assert_eq!(tree.len(), 2);
    assert!(walk(true).is_err());
}
