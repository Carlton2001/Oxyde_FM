use super::*;

fn entry(name: &str) -> FileEntry {
    FileEntry { name: name.into(), path: format!("C:/fixture/{name}"), ..Default::default() }
}

fn prepare(tab: &mut TabState, update: FileUpdate) -> std::time::Duration {
    let request = tab.preparation(update);
    let started = std::time::Instant::now();
    let prepared = request.calculate();
    let elapsed = started.elapsed();
    assert!(tab.apply_preparation(prepared));
    elapsed
}

#[test]
fn a_filter_change_keeps_the_sorted_files_without_copying_them() {
    let mut tab = TabState::new("tab".into(), "fixture".into());
    let listed: Vec<_> = (0..100_000).rev().map(|index| entry(&format!("file-{index}.txt"))).collect();
    let first = prepare(&mut tab, FileUpdate::Listing(listed.into()));
    let sorted = tab.files.clone();
    let labels = tab.column_type_labels.clone();
    let again = prepare(&mut tab, FileUpdate::Current);
    eprintln!("preparation: listing={first:?}; filter={again:?}");
    assert!(Arc::ptr_eq(&tab.files, &sorted));
    assert_eq!(tab.column_type_labels, labels);
    // Another order sorts again.
    tab.sort_direction = SortDirection::Descending;
    prepare(&mut tab, FileUpdate::Current);
    assert!(!Arc::ptr_eq(&tab.files, &sorted));
    assert_eq!(tab.files[0].name, "file-99999.txt");
}
