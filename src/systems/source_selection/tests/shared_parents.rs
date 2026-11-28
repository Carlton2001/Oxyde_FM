use super::*;

#[test]
fn sources_of_one_folder_share_its_held_parents() {
    let root = tempfile::tempdir().unwrap();
    let other = root.path().join("other");
    std::fs::create_dir(&other).unwrap();
    for path in [root.path().join("a.txt"), root.path().join("b.txt"), other.join("c.txt")] {
        std::fs::write(path, b"x").unwrap();
    }
    let mut parents = SelectionParents::default();
    let a = SelectedEntry::select_in(&root.path().join("a.txt"), &mut parents).unwrap();
    let b = SelectedEntry::select_in(&root.path().join("b.txt"), &mut parents).unwrap();
    let c = SelectedEntry::select_in(&other.join("c.txt"), &mut parents).unwrap();
    assert!(Arc::ptr_eq(&a.parent, &b.parent));
    assert!(!Arc::ptr_eq(&a.parent, &c.parent));
    assert_eq!(parents.held.len(), 2);
    // Each stays bound to its own name under the shared folder.
    assert_eq!(b.resolved_path().file_name().unwrap(), "b.txt");
    assert_eq!(b.into_file().unwrap().size, 1);
}

#[test]
fn a_source_gone_from_a_held_folder_is_still_refused() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("a.txt"), b"x").unwrap();
    let mut parents = SelectionParents::default();
    SelectedEntry::select_in(&root.path().join("a.txt"), &mut parents).unwrap();
    let error = SelectedEntry::select_in(&root.path().join("gone.txt"), &mut parents).err().unwrap();
    assert_eq!(error.kind(), io::ErrorKind::NotFound);
}
