use super::*;

/// As Nautilus (GIO fails the same way), a rename that changes only the case is
/// refused where the file system ignores it, and the item stays as it was:
/// `OXYDE_TEST_CASELESS=<mount>[:<mount>…]`.
#[test]
fn a_case_only_rename_on_a_caseless_system_is_refused_without_effect() {
    let Some(mounts) = std::env::var_os("OXYDE_TEST_CASELESS") else { return };
    for mount in std::env::split_paths(&mounts) {
        let root = tempfile::tempdir_in(&mount).unwrap();
        let source = root.path().join("notes.txt");
        std::fs::write(&source, b"kept").unwrap();
        let error = rename(&source, &root.path().join("Notes.txt")).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::AlreadyExists, "{}: {error}", mount.display());
        let names: Vec<_> = std::fs::read_dir(root.path()).unwrap().map(|entry| entry.unwrap().file_name()).collect();
        assert_eq!(names, ["notes.txt"]);
        assert_eq!(std::fs::read(&source).unwrap(), b"kept");
    }
}

#[test]
fn names_differing_by_case_stay_two_items_on_an_ordinary_folder() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("notes.txt");
    let other = root.path().join("Notes.txt");
    std::fs::write(&source, b"lower").unwrap();
    std::fs::write(&other, b"upper").unwrap();
    assert_eq!(rename(&source, &other).unwrap_err().kind(), io::ErrorKind::AlreadyExists);
    std::fs::remove_file(&other).unwrap();
    rename(&source, &other).unwrap();
    assert_eq!(std::fs::read(&other).unwrap(), b"lower");
}
