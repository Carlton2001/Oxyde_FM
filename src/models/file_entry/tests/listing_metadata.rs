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
