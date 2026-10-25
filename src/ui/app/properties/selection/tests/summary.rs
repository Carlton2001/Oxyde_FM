use super::inspect_selection;
use std::sync::atomic::AtomicBool;

#[test]
fn files_share_one_summary_with_exact_total_and_location() {
    let root = tempfile::tempdir().unwrap();
    let paths: Vec<_> = [
        root.path().join("first.txt"),
        root.path().join("second.zip"),
    ]
    .into_iter()
    .enumerate()
    .map(|(index, path)| {
        std::fs::write(&path, vec![0; index + 3]).unwrap();
        path.to_string_lossy().into_owned()
    })
    .collect();
    let (summary, directories) = inspect_selection(&paths, &AtomicBool::new(false)).unwrap();
    assert_eq!(summary.files, 2);
    assert_eq!(summary.folders, 0);
    assert_eq!(summary.size, 7);
    assert_eq!(
        summary.location.as_deref(),
        Some(root.path().to_str().unwrap())
    );
    assert!(directories.is_empty());
}

#[test]
fn folders_are_scanned_and_selected_descendants_are_not_counted_twice() {
    let root = tempfile::tempdir().unwrap();
    let folder = root.path().join("folder");
    std::fs::create_dir(&folder).unwrap();
    let nested = folder.join("nested.txt");
    let outside = root.path().join("outside.txt");
    std::fs::write(&nested, b"12345").unwrap();
    std::fs::write(&outside, b"12").unwrap();
    let paths: Vec<_> = [&folder, &nested, &outside]
        .into_iter()
        .map(|path| path.to_string_lossy().into_owned())
        .collect();
    let (summary, directories) = inspect_selection(&paths, &AtomicBool::new(false)).unwrap();
    assert_eq!(summary.files, 1);
    assert_eq!(summary.folders, 1);
    assert_eq!(summary.size, 2);
    assert!(summary.location.is_none());
    assert_eq!(directories, vec![folder.to_string_lossy().into_owned()]);
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let scan = runtime
        .block_on(oxyde_lib::commands::io::calculate_folder_size(
            directories[0].clone(),
        ))
        .unwrap();
    assert_eq!(summary.size + scan.size, 7);
    assert_eq!(summary.files + scan.files_count, 2);
}

#[test]
fn cancellation_and_missing_files_do_not_publish_an_incomplete_total() {
    let root = tempfile::tempdir().unwrap();
    let missing = root
        .path()
        .join("missing.txt")
        .to_string_lossy()
        .into_owned();
    assert!(inspect_selection(std::slice::from_ref(&missing), &AtomicBool::new(true)).is_err());
    assert!(inspect_selection(&[missing], &AtomicBool::new(false)).is_err());
}
