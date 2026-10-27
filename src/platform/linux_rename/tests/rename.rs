use super::*;
use std::os::fd::AsFd;

/// Volumes to try: a temporary folder, plus `OXYDE_TEST_NTFS` when set to a folder on
/// an ntfs-3g mount, whose refusal of `RENAME_NOREPLACE` exercises the fallback.
fn roots() -> Vec<tempfile::TempDir> {
    let mut roots = vec![tempfile::tempdir().unwrap()];
    if let Some(ntfs) = std::env::var_os("OXYDE_TEST_NTFS") {
        roots.push(tempfile::tempdir_in(ntfs).unwrap());
    }
    roots
}

#[test]
fn a_folder_moves_between_parents_but_never_over_an_existing_name() {
    for root in roots() {
        let (from, to) = (root.path().join("from"), root.path().join("to"));
        std::fs::create_dir_all(from.join("folder/inner")).unwrap();
        std::fs::create_dir(&to).unwrap();
        std::fs::write(to.join("taken"), b"kept").unwrap();
        let (from_dir, to_dir) = (std::fs::File::open(&from).unwrap(), std::fs::File::open(&to).unwrap());

        let error = exclusive(from_dir.as_fd(), Path::new("folder"), to_dir.as_fd(), Path::new("taken")).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::AlreadyExists, "{:?}", root.path());
        assert_eq!(std::fs::read(to.join("taken")).unwrap(), b"kept");

        exclusive(from_dir.as_fd(), Path::new("folder"), to_dir.as_fd(), Path::new("folder")).unwrap();
        assert!(to.join("folder/inner").is_dir());
        assert!(!from.join("folder").exists());
    }
}

#[test]
fn absolute_paths_rename_from_the_working_directory() {
    for root in roots() {
        let (from, to) = (root.path().join("a"), root.path().join("b"));
        std::fs::write(&from, b"a").unwrap();
        std::fs::write(&to, b"b").unwrap();
        assert!(exclusive(rustix::fs::CWD, &from, rustix::fs::CWD, &to).is_err());
        std::fs::remove_file(&to).unwrap();
        exclusive(rustix::fs::CWD, &from, rustix::fs::CWD, &to).unwrap();
        assert_eq!(std::fs::read(&to).unwrap(), b"a");
    }
}

#[test]
fn folders_share_a_mount_only_on_the_same_file_system_and_mount() {
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(root.path().join("a/b")).unwrap();
    let open = |path: &Path| std::fs::File::open(path).unwrap();
    assert!(same_mount(&open(root.path()), &open(&root.path().join("a/b"))).unwrap());
    assert!(!same_mount(&open(root.path()), &open(Path::new("/proc"))).unwrap());
}
