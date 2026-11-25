use super::*;

#[test]
fn a_hard_link_in_another_folder_is_not_the_same_entry() {
    let root = tempfile::tempdir().unwrap();
    let (first, second) = (root.path().join("A"), root.path().join("B"));
    std::fs::create_dir(&first).unwrap();
    std::fs::create_dir(&second).unwrap();
    std::fs::write(first.join("a.txt"), b"data").unwrap();
    std::fs::hard_link(first.join("a.txt"), second.join("a.txt")).unwrap();
    assert!(same_entry(&first.join("a.txt"), &first.join("a.txt")));
    assert!(!same_entry(&first.join("a.txt"), &second.join("a.txt")));
}
