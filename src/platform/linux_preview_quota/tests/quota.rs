use super::*;
use std::fs;
use std::path::Path;

fn name(index: u32) -> String {
    format!("{index:064x}.jpg")
}

fn folder(root: &Path) -> OwnedFd {
    rustix::fs::open(root, OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC, Mode::empty()).unwrap()
}

/// An entry written `age` seconds ago.
fn seed(root: &Path, file: &str, bytes: &[u8], age: u64) {
    let path = root.join(file);
    fs::write(&path, bytes).unwrap();
    let time = SystemTime::now() - Duration::from_secs(age);
    fs::File::options().write(true).open(&path).unwrap().set_modified(time).unwrap();
}

fn admitted(root: &Path, index: u32, incoming: u64, policy: Policy) -> io::Result<OwnedFd> {
    admit(&folder(root), OsStr::new(&name(index)), OsStr::new("incoming.tmp"), incoming, policy)
}

#[test]
fn the_oldest_entries_go_until_the_new_one_fits() {
    let root = tempfile::tempdir().unwrap();
    for (index, age) in [(1, 300), (2, 100), (3, 200)] {
        seed(root.path(), &name(index), b"1234", age);
    }
    fs::write(root.path().join("notes.txt"), b"other").unwrap();
    admitted(root.path(), 4, 4, Policy { bytes: 13, entries: 10, entry_bytes: 10 }).unwrap();
    assert!(!root.path().join(name(1)).exists());
    assert!(!root.path().join(name(3)).exists());
    assert!(root.path().join(name(2)).exists());
    assert!(root.path().join("notes.txt").exists());
}

#[test]
fn the_entry_replaced_and_files_of_others_are_never_evicted() {
    let root = tempfile::tempdir().unwrap();
    seed(root.path(), &name(1), b"1234", 300);
    seed(root.path(), "photo.jpg", b"1234", 400);
    admitted(root.path(), 1, 4, Policy { bytes: 100, entries: 2, entry_bytes: 10 }).unwrap();
    assert!(root.path().join(name(1)).exists());
    let error = admitted(root.path(), 2, 4, Policy { bytes: 100, entries: 1, entry_bytes: 10 }).unwrap_err();
    assert!(error.to_string().contains("other files"), "{error}");
    assert!(root.path().join("photo.jpg").exists());
}

#[test]
fn only_abandoned_staging_files_go_first() {
    let root = tempfile::tempdir().unwrap();
    seed(root.path(), &name(1), b"1234", 300);
    let abandoned = format!("{:064x}.123.7.tmp", 2);
    let active = format!("{:064x}.123.8.tmp", 3);
    seed(root.path(), &abandoned, b"1234", 2 * 60 * 60);
    seed(root.path(), &active, b"1234", 0);
    admitted(root.path(), 4, 4, Policy { bytes: 100, entries: 3, entry_bytes: 10 }).unwrap();
    assert!(!root.path().join(&abandoned).exists());
    assert!(root.path().join(&active).exists());
    assert!(root.path().join(name(1)).exists());
}

#[test]
fn an_oversized_entry_or_a_held_lease_is_refused() {
    let root = tempfile::tempdir().unwrap();
    assert!(admitted(root.path(), 1, 11, Policy { bytes: 100, entries: 10, entry_bytes: 10 }).is_err());
    let lease = admitted(root.path(), 1, 4, Policy::default()).unwrap();
    let error = admitted(root.path(), 2, 4, Policy::default()).unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::WouldBlock);
    drop(lease);
    admitted(root.path(), 2, 4, Policy::default()).unwrap();
}

#[test]
fn publication_through_the_platform_keeps_the_cache_bounded() {
    use std::io::Write;
    let root = tempfile::tempdir().unwrap();
    seed(root.path(), &name(1), &[0; 1024], 300);
    let path = root.path().join(name(2));
    let temporary = root.path().join(format!("{:064x}.1.1.tmp", 2));
    let lease = admit_beside(&path, &temporary, 4).unwrap();
    assert!(lease.is_some());
    drop(lease);
    assert!(admit_beside(&root.path().join("other.png"), &root.path().join("other.tmp"), 4).unwrap().is_none());
    crate::platform::platform().write_preview_cache(&path, &temporary, &mut |file| file.write_all(b"new")).unwrap();
    assert_eq!(fs::read(&path).unwrap(), b"new");
}
