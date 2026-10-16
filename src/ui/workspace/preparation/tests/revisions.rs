use super::*;

#[test]
fn profile_background_search_preparation_and_small_ui_installation() {
    for count in [20_000, 100_000] {
        let mut tab = TabState::new("tab".into(), "fixture".into());
        tab.files = (0..count)
            .rev()
            .map(|index| entry(&format!("file-{index}.txt")))
            .collect::<Vec<_>>()
            .into();
        tab.update_cache();
        let frame = tab.cached_filtered_files.clone();
        let started = std::time::Instant::now();
        let request = tab.preparation(FileUpdate::Append(vec![entry("extra.txt")].into()));
        let snapshot_time = started.elapsed();
        let started = std::time::Instant::now();
        let prepared = std::thread::spawn(move || request.calculate())
            .join()
            .unwrap();
        let background_time = started.elapsed();
        let started = std::time::Instant::now();
        assert!(tab.apply_preparation(prepared));
        let install_time = started.elapsed();
        eprintln!("file preparation: entries={count}; ui_snapshot={snapshot_time:?}; background={background_time:?}; ui_install={install_time:?}");
        assert_eq!(frame.len(), count);
        assert_eq!(tab.files.len(), count + 1);
        assert_eq!(tab.files[0].name, "extra.txt");
        assert!(tab
            .files
            .windows(2)
            .all(|pair| pair[0].name <= pair[1].name));
    }
}

fn entry(name: &str) -> FileEntry {
    FileEntry {
        name: name.into(),
        path: format!("C:/fixture/{name}"),
        ..Default::default()
    }
}

#[test]
fn listing_preserves_legacy_natural_ties_and_search_preserves_arrival_order() {
    for direction in [SortDirection::Ascending, SortDirection::Descending] {
        for listed in [true, false] {
            let mut tab = TabState::new("tab".into(), "fixture".into());
            tab.sort_direction = direction;
            let files = vec![entry("é.txt"), entry("É.txt")].into();
            let update = if listed {
                FileUpdate::Listing(files)
            } else {
                FileUpdate::Append(files)
            };
            assert!(tab.apply_preparation(tab.preparation(update).calculate()));
            let names = tab
                .files
                .iter()
                .map(|file| file.name.as_str())
                .collect::<Vec<_>>();
            assert_eq!(
                names,
                if listed {
                    ["É.txt", "é.txt"]
                } else {
                    ["é.txt", "É.txt"]
                }
            );
        }
    }
}

#[test]
fn background_preparation_keeps_lexical_order_stable_ties_filters_and_snapshot_ownership() {
    let mut tab = TabState::new("tab".into(), "C:/fixture".into());
    tab.files = vec![entry("file2"), entry("File10"), entry("file10")].into();
    tab.update_cache();
    let before = tab.files.clone();
    let prepared = tab
        .preparation(FileUpdate::Append(vec![entry("file1")].into()))
        .calculate();
    assert_eq!(before[0].name, "file2");
    assert!(tab.apply_preparation(prepared));
    assert_eq!(
        tab.files
            .iter()
            .map(|file| file.name.as_str())
            .collect::<Vec<_>>(),
        ["file1", "File10", "file10", "file2"]
    );
    assert!(Arc::ptr_eq(&tab.files, &tab.cached_filtered_files));
    tab.filter_name = "2".into();
    tab.invalidate_preparation();
    let prepared = tab.preparation(FileUpdate::Current).calculate();
    assert!(tab.apply_preparation(prepared));
    assert_eq!(
        tab.filtered_files()
            .iter()
            .map(|file| file.name.as_str())
            .collect::<Vec<_>>(),
        ["file2"]
    );
    assert_eq!(before.len(), 3);
}

#[test]
fn stale_filter_listing_source_path_and_tab_results_never_replace_current_files() {
    for change in 0..5 {
        let mut tab = TabState::new("tab".into(), "old".into());
        tab.files = vec![entry("old")].into();
        tab.update_cache();
        let prepared = tab
            .preparation(FileUpdate::Replace(vec![entry("incoming")].into()))
            .calculate();
        match change {
            0 => {
                tab.filter_name = "new".into();
                tab.invalidate_preparation();
            }
            1 => tab.clear_files(),
            2 => {
                Arc::make_mut(&mut tab.files).push(entry("newer"));
            }
            3 => tab.path = "new path".into(),
            _ => tab.id = "another tab".into(),
        }
        let expected = tab.files.clone();
        assert!(!tab.apply_preparation(prepared));
        assert!(Arc::ptr_eq(&expected, &tab.files));
    }
}
