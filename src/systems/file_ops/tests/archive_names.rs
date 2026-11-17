use super::*;

/// A link to nothing; `None` where the system refuses to create one (Windows without
/// Developer Mode).
fn broken_link(path: &Path) -> Option<()> {
    let missing = path.with_file_name("missing-target");
    #[cfg(unix)]
    let created = std::os::unix::fs::symlink(&missing, path);
    #[cfg(windows)]
    let created = std::os::windows::fs::symlink_file(&missing, path);
    created.map_err(|error| eprintln!("symbolic links unavailable: {error}")).ok()
}

#[test]
fn names_derive_from_the_original_and_keep_compound_suffixes() {
    let directory = tempfile::tempdir().unwrap();
    let archive = directory.path().join("a.tar.gz");
    assert_eq!(FileOperationManager::free_archive_path(&archive, 0), (archive.clone(), 0));
    for name in ["a.tar.gz", "a (1).tar.gz"] {
        std::fs::write(directory.path().join(name), b"taken").unwrap();
    }
    // A refused « a (1) » moves on to « a (2) », never to « a (1) (1) ».
    assert_eq!(
        FileOperationManager::free_archive_path(&archive, 0),
        (directory.path().join("a (2).tar.gz"), 2)
    );
    assert_eq!(
        FileOperationManager::free_archive_path(&archive, 3),
        (directory.path().join("a (3).tar.gz"), 3)
    );
}

#[test]
fn a_broken_link_occupies_its_name() {
    let directory = tempfile::tempdir().unwrap();
    let archive = directory.path().join("photos.zip");
    if broken_link(&archive).is_none() {
        return;
    }
    assert!(!archive.exists());
    assert_eq!(
        FileOperationManager::free_archive_path(&archive, 0),
        (directory.path().join("photos (1).zip"), 1)
    );
}

#[test]
fn compressing_beside_a_broken_link_publishes_under_a_free_name() {
    use crate::systems::archive_workspace::mutation_operation;
    let directory = tempfile::tempdir().unwrap();
    let requested = directory.path().join("photos.zip");
    if broken_link(&requested).is_none() {
        return;
    }
    let sources = tempfile::tempdir().unwrap();
    let source = sources.path().join("photo.txt");
    std::fs::write(&source, b"payload").unwrap();
    let (sender, receiver) = std::sync::mpsc::channel();
    let target = requested.clone();
    std::thread::spawn(move || {
        let operation = mutation_operation("fast");
        let result = FileOperationManager::perform_compress(&operation, vec![source], Some(target));
        let _ = sender.send((result, operation.lock().unwrap().destination.clone()));
    });
    // The former retry loop never ended: fail instead of hanging the suite.
    let (result, published) = receiver
        .recv_timeout(std::time::Duration::from_secs(30))
        .expect("compression must not retry the same name forever");
    result.unwrap();
    assert_eq!(published, Some(directory.path().join("photos (1).zip")));
    assert!(std::fs::symlink_metadata(&requested).unwrap().file_type().is_symlink());
}
