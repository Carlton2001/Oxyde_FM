use super::*;
use std::process::{Command, Stdio};
use std::time::Duration;

/// A child forked by a concurrent test briefly shares this process's descriptors,
/// and a lease lasts until its last reference closes: retry an expected success.
fn eventually(path: &std::path::Path) -> File {
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    loop {
        match open_cleanup(path, false) {
            Err(error) if error.kind() == io::ErrorKind::WouldBlock && std::time::Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(10))
            }
            result => return result.unwrap(),
        }
    }
}

fn cached() -> (tempfile::TempDir, std::path::PathBuf) {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("document.txt");
    std::fs::write(&path, b"cached").unwrap();
    (root, path)
}

#[test]
fn a_closed_file_is_leased_until_its_handle_closes() {
    let (_root, path) = cached();
    let file = eventually(&path);
    still_exclusive(&file).unwrap();
    assert_eq!(open_cleanup(&path, false).unwrap_err().kind(), io::ErrorKind::WouldBlock);
    drop(file);
    drop(eventually(&path));
}

#[test]
fn a_reader_in_this_or_another_process_prevents_cleanup() {
    let (_root, path) = cached();
    let reader = File::open(&path).unwrap();
    assert_eq!(open_cleanup(&path, false).unwrap_err().kind(), io::ErrorKind::WouldBlock);
    drop(reader);
    let mut child = Command::new("sleep").arg("5").stdin(File::open(&path).unwrap()).spawn().unwrap();
    assert_eq!(open_cleanup(&path, false).unwrap_err().kind(), io::ErrorKind::WouldBlock);
    child.kill().unwrap();
    child.wait().unwrap();
    drop(eventually(&path));
}

#[test]
fn an_opener_during_cleanup_is_noticed_and_waits_without_a_signal() {
    let (_root, path) = cached();
    let file = eventually(&path);
    // The child blocks in open() until the lease is released.
    let mut child = Command::new("cat").arg(&path).stdout(Stdio::piped()).spawn().unwrap();
    std::thread::sleep(Duration::from_millis(200));
    assert_eq!(still_exclusive(&file).unwrap_err().kind(), io::ErrorKind::WouldBlock);
    assert!(child.try_wait().unwrap().is_none());
    drop(file);
    let output = child.wait_with_output().unwrap();
    assert_eq!(output.stdout, b"cached");
}

#[test]
fn links_and_special_files_are_refused_and_folders_open_unleased() {
    let (root, path) = cached();
    let link = root.path().join("link");
    std::os::unix::fs::symlink(&path, &link).unwrap();
    assert!(open_cleanup(&link, false).is_err());
    assert!(open_cleanup(&link, true).is_err());
    assert!(open_cleanup(&path, true).is_err());
    let folder = open_cleanup(root.path(), true).unwrap();
    still_exclusive(&folder).unwrap();
    assert!(open_cleanup(root.path(), false).is_err());
    let fifo = root.path().join("fifo");
    assert!(Command::new("mkfifo").arg(&fifo).status().unwrap().success());
    assert!(open_cleanup(&fifo, false).is_err());
}
