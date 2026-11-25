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

#[test]
fn batches_appended_to_sorted_files_end_as_one_listing_would() {
    let mut batched = TabState::new("tab".into(), "fixture".into());
    let names: Vec<String> = (0..300).map(|index| format!("File-{}.{}", (index * 37) % 120, ["txt", "log", "md"][index % 3])).collect();
    for batch in names.chunks(70) {
        let additions: Vec<_> = batch.iter().map(|name| entry(name)).collect();
        prepare(&mut batched, FileUpdate::Append(additions.into()));
    }
    let mut whole = TabState::new("tab".into(), "fixture".into());
    prepare(&mut whole, FileUpdate::Replace(names.iter().map(|name| entry(name)).collect::<Vec<_>>().into()));
    let paths = |tab: &TabState| tab.files.iter().map(|file| file.path.clone()).collect::<Vec<_>>();
    assert_eq!(paths(&batched), paths(&whole));
    assert_eq!(batched.column_type_labels, whole.column_type_labels);
    assert_eq!(batched.column_size_values, whole.column_size_values);
}
