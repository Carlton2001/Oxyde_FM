use super::*;

#[test]
fn profile_listing_metadata_reuse_against_reopening_each_entry() {
    let root = tempfile::tempdir().unwrap();
    for index in 0..1000 {
        std::fs::write(root.path().join(format!("file-{index}.txt")), b"payload").unwrap();
    }
    let listed: Vec<_> = std::fs::read_dir(root.path())
        .unwrap()
        .map(|entry| {
            let entry = entry.unwrap();
            (entry.path(), entry.metadata().unwrap())
        })
        .collect();
    let started = std::time::Instant::now();
    for _ in 0..3 {
        for (path, _) in &listed {
            std::hint::black_box(get_file_entry_from_path(path).unwrap());
        }
    }
    let reopened = started.elapsed();
    let started = std::time::Instant::now();
    for _ in 0..3 {
        for (path, metadata) in &listed {
            std::hint::black_box(file_entry_from_metadata(path, metadata));
        }
    }
    eprintln!("listing construction: entries=1000, iterations=3; reopened_3000={reopened:?}; metadata_reuse={:?}", started.elapsed());
}

#[test]
fn ordinary_listing_entry_consumes_existing_metadata_without_reopening_the_name() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("selected.txt");
    std::fs::write(&path, b"selected").unwrap();
    let metadata = std::fs::symlink_metadata(&path).unwrap();
    let expected = get_file_entry_from_path(&path).unwrap();
    std::fs::remove_file(&path).unwrap();
    let actual = file_entry_from_metadata(&path, &metadata);
    assert_eq!(format!("{actual:?}"), format!("{expected:?}"));
    assert!(get_file_entry_from_path(&path).is_err());
}

#[test]
fn listing_creation_time_comes_from_the_supplied_metadata() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("created.txt");
    std::fs::write(&path, b"created").unwrap();
    let metadata = std::fs::symlink_metadata(&path).unwrap();
    let expected = metadata.created().ok().and_then(|time| time.duration_since(SystemTime::UNIX_EPOCH).ok()).and_then(|duration| duration.as_millis().try_into().ok());
    let actual = file_entry_from_metadata(&path, &metadata);
    assert_eq!(actual.created, expected);
}

#[cfg(unix)]
#[test]
fn links_behave_like_their_target_and_broken_links_stay_links() {
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir(root.path().join("folder")).unwrap();
    std::fs::write(root.path().join("file"), b"payload").unwrap();
    for (link, target) in [("to-folder", "folder"), ("to-file", "file"), ("broken", "missing")] {
        std::os::unix::fs::symlink(target, root.path().join(link)).unwrap();
    }
    let entry = |name: &str| get_file_entry_from_path(&root.path().join(name)).unwrap();
    let folder = entry("to-folder");
    assert!(folder.is_symlink && folder.is_dir && !folder.is_junction && !folder.is_broken_link);
    assert_eq!(folder.size, 0);
    let file = entry("to-file");
    assert!(file.is_symlink && !file.is_dir && !file.is_broken_link);
    assert_eq!(file.size, 7);
    let broken = entry("broken");
    assert!(broken.is_symlink && !broken.is_dir && broken.is_broken_link);
    assert!(!entry("file").is_broken_link);
    assert_eq!(broken.size, "missing".len() as u64);
}
