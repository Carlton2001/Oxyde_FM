use super::*;

#[test]
fn raw_listing_keeps_metadata_and_public_listing_keeps_natural_order() {
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir(root.path().join("folder")).unwrap();
    for name in ["file10.txt", "file2.txt", "file1.txt"] {
        std::fs::write(root.path().join(name), name.as_bytes()).unwrap();
    }
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let path = root.path().to_string_lossy().into_owned();
    let ordered = runtime
        .block_on(list_dir(path.clone(), Some(true), Some(true)))
        .unwrap();
    assert_eq!(
        ordered
            .iter()
            .map(|file| file.name.as_str())
            .collect::<Vec<_>>(),
        ["folder", "file1.txt", "file2.txt", "file10.txt"]
    );
    let mut raw = runtime
        .block_on(list_dir_unsorted(path, Some(true), Some(true)))
        .unwrap();
    for file in &raw {
        let expected =
            crate::models::get_file_entry_from_path(std::path::Path::new(&file.path)).unwrap();
        assert_eq!(format!("{file:?}"), format!("{expected:?}"));
    }
    sort_file_entries(&mut raw);
    assert_eq!(format!("{raw:?}"), format!("{ordered:?}"));
}

#[cfg(windows)]
#[test]
fn reused_listing_metadata_preserves_junction_flags_and_target_stamp() {
    let fixture = tempfile::tempdir().unwrap();
    let root = fixture.path().join("listing");
    let target = fixture.path().join("target");
    std::fs::create_dir(&root).unwrap();
    std::fs::create_dir(&target).unwrap();
    std::fs::write(target.join("payload"), b"target").unwrap();
    let link = root.join("junction");
    crate::platform::windows_reparse_tests::junction(&link, &target);
    let expected = crate::models::get_file_entry_from_path(&link).unwrap();
    let files = tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(list_dir_unsorted(
            root.to_string_lossy().into_owned(),
            Some(true),
            Some(true),
        ))
        .unwrap();
    assert_eq!(files.len(), 1);
    assert_eq!(format!("{:?}", files[0]), format!("{expected:?}"));
    std::fs::remove_dir(link).unwrap();
    assert_eq!(std::fs::read(target.join("payload")).unwrap(), b"target");
}
