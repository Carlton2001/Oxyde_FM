use super::*;
use crate::ui::workspace::{FileUpdate, SortColumn, SortDirection};
use oxyde_lib::models::FolderSizeResult;
use std::{
    collections::HashSet,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

fn folder(name: &str, size: Option<u64>) -> FileEntry {
    FileEntry {
        name: name.into(),
        path: format!("C:/root/{name}"),
        is_dir: true,
        size: size.unwrap_or(0),
        is_calculated: size.is_some(),
        ..Default::default()
    }
}

fn tab() -> TabState {
    let mut tab = TabState::new("tab".into(), "C:/root".into());
    tab.files = vec![
        folder("a", Some(10)),
        folder("b", Some(20)),
        folder("c", Some(30)),
    ]
    .into();
    tab.show_size_histogram = true;
    tab
}

#[test]
fn bursts_coalesce_and_changes_during_scan_queue_one_fair_followup() {
    let mut tab = tab();
    tab.invalidate_folder_sizes(None);
    let batch = tab.folder_sizes.take_batch();
    assert_eq!(batch.len(), 2);
    let (path, revision) = &batch[0];
    let changes = HashSet::from([path.clone()]);
    for _ in 0..10_000 {
        tab.invalidate_folder_sizes(Some(&changes));
    }
    assert!(tab.folder_sizes.current(0, path, *revision));
    assert_eq!(tab.folder_sizes.pending.len(), 2);
    assert_eq!(tab.folder_sizes.queue.len(), 2);
    for (path, revision) in batch {
        tab.folder_sizes.finish(&path, revision);
    }
    let next = tab
        .folder_sizes
        .take_available_at(2, Instant::now() + RESCAN_INTERVAL);
    assert_eq!(
        next.iter()
            .map(|(path, _)| path.as_str())
            .collect::<Vec<_>>(),
        ["C:/root/c", "C:/root/a"]
    );
    for (path, revision) in next {
        assert!(tab.folder_sizes.current(0, &path, revision));
        tab.folder_sizes.finish(&path, revision);
    }
    assert!(tab.folder_sizes.take_batch().is_empty());
}

#[test]
fn a_slow_scan_does_not_block_free_slots_or_run_concurrently_with_itself() {
    let mut tab = tab();
    tab.invalidate_folder_sizes(None);
    let first = tab.folder_sizes.take_available(2);
    tab.invalidate_folder_sizes(Some(&HashSet::from([first[0].0.clone()])));
    assert!(tab.folder_sizes.take_available(2).is_empty());
    tab.folder_sizes.finish(&first[1].0, first[1].1);
    let next = tab.folder_sizes.take_available(2);
    assert_eq!(next.len(), 1);
    assert_eq!(next[0].0, "C:/root/c");
    tab.folder_sizes.finish(&next[0].0, next[0].1);
    assert!(tab.folder_sizes.take_available(2).is_empty());
    tab.folder_sizes.finish(&first[0].0, first[0].1);
    assert!(tab.folder_sizes.take_available(2).is_empty());
    assert_eq!(
        tab.folder_sizes
            .take_available_at(2, Instant::now() + RESCAN_INTERVAL)[0]
            .0,
        "C:/root/a"
    );
}

#[test]
fn render_snapshots_share_the_queue_without_copying_or_owning_future_changes() {
    let mut tab = tab();
    tab.invalidate_folder_sizes(None);
    let snapshot = tab.clone();
    assert!(Arc::ptr_eq(
        &snapshot.folder_sizes.queue,
        &tab.folder_sizes.queue
    ));
    assert!(Arc::ptr_eq(
        &snapshot.folder_sizes.pending,
        &tab.folder_sizes.pending
    ));
    tab.folder_sizes.take_available(2);
    assert_eq!(snapshot.folder_sizes.pending.len(), 3);
    assert_eq!(tab.folder_sizes.pending.len(), 1);
}

#[test]
fn only_published_changes_to_previously_calculated_sizes_flash() {
    let mut tab = tab();
    Arc::make_mut(&mut tab.files).push(folder("new", None));
    let update = |path: &str, size| {
        FileUpdate::FolderSizes(vec![(
            format!("C:/root/{path}"),
            FolderSizeResult {
                size,
                files_count: 1,
                folders_count: 0,
            },
        )])
    };
    for change in [update("new", 10), update("a", 10)] {
        assert!(tab.apply_preparation(tab.preparation(change).calculate()));
    }
    assert!(tab.folder_sizes.size_flashes().is_empty());
    assert!(tab.apply_preparation(tab.preparation(update("a", 50)).calculate()));
    let started = tab.folder_sizes.size_flashes()["C:/root/a"];
    assert!(tab.apply_preparation(tab.preparation(update("a", 50)).calculate()));
    assert_eq!(tab.folder_sizes.size_flashes()["C:/root/a"], started);
    let stale = tab.preparation(update("b", 100)).calculate();
    tab.invalidate_preparation();
    assert!(!tab.apply_preparation(stale));
    assert!(!tab.folder_sizes.size_flashes().contains_key("C:/root/b"));
    tab.reset_folder_sizes();
    assert!(tab.folder_sizes.size_flashes().is_empty());
}

#[test]
fn refresh_keeps_values_and_recomputes_all_rows_with_background_sort_and_filters() {
    let mut tab = tab();
    tab.invalidate_folder_sizes(None);
    let prepared = tab
        .preparation(FileUpdate::Listing(
            vec![folder("a", None), folder("b", None), folder("d", None)].into(),
        ))
        .calculate();
    assert!(tab.apply_preparation(prepared));
    tab.queue_missing_folder_sizes();
    assert!(tab.show_size_histogram);
    assert_eq!(
        tab.files
            .iter()
            .map(|file| (file.name.as_str(), file.size, file.is_calculated))
            .collect::<Vec<_>>(),
        [("a", 10, true), ("b", 20, true), ("d", 0, false)]
    );
    assert_eq!(tab.folder_sizes.pending.len(), 3);
    assert!(!tab.folder_sizes.pending.contains_key("C:/root/c"));
    tab.sort_column = SortColumn::Size;
    tab.sort_direction = SortDirection::Descending;
    let prepared = tab
        .preparation(FileUpdate::FolderSizes(vec![(
            "C:/root/a".into(),
            FolderSizeResult {
                size: 100,
                files_count: 5,
                folders_count: 2,
            },
        )]))
        .calculate();
    assert!(tab.apply_preparation(prepared));
    assert_eq!(tab.filtered_files()[0].name, "a");
    assert_eq!(tab.files[0].files_count, Some(5));
    assert_eq!(tab.files[0].folders_count, Some(2));
    assert!(tab.column_size_values.contains(&(100, true, true)));
}

#[test]
fn targeted_updates_skip_other_folders_links_and_junctions() {
    let mut tab = tab();
    let mut link = folder("link", None);
    link.is_symlink = true;
    let mut junction = folder("junction", None);
    junction.is_junction = true;
    Arc::make_mut(&mut tab.files).extend([link, junction]);
    tab.queue_missing_folder_sizes();
    assert!(tab.folder_sizes.take_batch().is_empty());
    tab.invalidate_folder_sizes(Some(&HashSet::from(["C:/root/b".into()])));
    let batch = tab.folder_sizes.take_batch();
    assert_eq!(batch.len(), 1);
    assert_eq!(batch[0].0, "C:/root/b");
}

#[test]
fn navigation_cancels_and_rejects_old_results_even_after_returning_to_same_path() {
    let mut tab = tab();
    tab.invalidate_folder_sizes(None);
    let old = tab.folder_sizes.take_batch();
    let token = Arc::new(AtomicBool::new(false));
    tab.folder_sizes.cancellation = Some(token.clone());
    tab.reset_folder_sizes();
    assert!(token.load(Ordering::Relaxed));
    assert!(!tab.show_size_histogram);
    tab.show_size_histogram = true;
    tab.invalidate_folder_sizes(None);
    tab.folder_sizes.take_batch();
    for (path, revision) in old {
        assert!(!tab.folder_sizes.current(0, &path, revision));
    }
}

#[test]
fn removed_then_recreated_path_cannot_receive_the_old_scan_result() {
    let mut tab = tab();
    tab.invalidate_folder_sizes(None);
    let old = tab.folder_sizes.take_batch();
    tab.files = vec![folder("c", Some(30))].into();
    tab.queue_missing_folder_sizes();
    tab.files = vec![folder("a", None), folder("c", Some(30))].into();
    tab.queue_missing_folder_sizes();
    assert!(!tab.folder_sizes.current(0, &old[0].0, old[0].1));
    assert!(tab.folder_sizes.pending.contains_key("C:/root/a"));
    assert!(tab.folder_sizes.take_available(2).is_empty());
}

#[test]
fn nested_filesystem_changes_recompute_the_target_and_refresh_keeps_other_totals() {
    let root = tempfile::tempdir().unwrap();
    let a = root.path().join("a");
    let b = root.path().join("b");
    std::fs::create_dir_all(a.join("inner")).unwrap();
    std::fs::create_dir(&b).unwrap();
    std::fs::write(a.join("inner").join("data"), [0; 10]).unwrap();
    std::fs::write(b.join("data"), [0; 20]).unwrap();
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let path = root.path().to_string_lossy().into_owned();
    let mut tab = TabState::new("real".into(), path.clone());
    tab.show_size_histogram = true;
    let list = || {
        runtime
            .block_on(oxyde_lib::commands::io::list_dir_unsorted(
                path.clone(),
                Some(true),
                Some(true),
            ))
            .unwrap()
    };
    assert!(tab.apply_preparation(
        tab.preparation(FileUpdate::Listing(list().into()))
            .calculate()
    ));
    tab.queue_missing_folder_sizes();
    let recalculate = |tab: &mut TabState| {
        let batch = tab.folder_sizes.take_available(2);
        let count = batch.len();
        for (path, revision) in batch {
            let size = runtime
                .block_on(oxyde_lib::commands::io::calculate_folder_size(path.clone()))
                .unwrap();
            assert!(tab.folder_sizes.current(0, &path, revision));
            assert!(tab.apply_preparation(
                tab.preparation(FileUpdate::FolderSizes(vec![(path.clone(), size)]))
                    .calculate()
            ));
            tab.folder_sizes.finish(&path, revision);
        }
        count
    };
    assert_eq!(recalculate(&mut tab), 2);
    std::fs::write(a.join("inner").join("data"), [0; 70]).unwrap();
    tab.invalidate_folder_sizes(Some(&HashSet::from([a.to_string_lossy().into_owned()])));
    assert_eq!(recalculate(&mut tab), 1);
    assert_eq!(tab.files.iter().map(|file| file.size).sum::<u64>(), 90);
    assert!(tab.apply_preparation(
        tab.preparation(FileUpdate::Listing(list().into()))
            .calculate()
    ));
    assert!(tab.files.iter().all(|file| file.is_calculated));
    assert_eq!(tab.files.iter().map(|file| file.size).sum::<u64>(), 90);
    std::fs::remove_file(a.join("inner").join("data")).unwrap();
    tab.invalidate_folder_sizes(None);
    assert_eq!(recalculate(&mut tab), 2);
    assert_eq!(
        tab.files.iter().find(|file| file.name == "a").unwrap().size,
        0
    );
    assert_eq!(
        tab.files.iter().find(|file| file.name == "b").unwrap().size,
        20
    );
}

#[cfg(target_os = "windows")]
#[test]
fn watcher_path_case_and_separators_match_listing_paths() {
    let mut tab = tab();
    tab.invalidate_folder_sizes(Some(&HashSet::from(["c:\\ROOT\\B".into()])));
    assert_eq!(tab.folder_sizes.take_batch()[0].0, "C:/root/b");
}

fn result(size: u64) -> FolderSizeResult {
    FolderSizeResult {
        size,
        files_count: 1,
        folders_count: 0,
    }
}

#[test]
fn completed_scans_release_slots_before_publication_and_survive_listing_reconciliation() {
    let mut tab = tab();
    Arc::make_mut(&mut tab.files)[0].is_calculated = false;
    tab.invalidate_folder_sizes(None);
    let first = tab.folder_sizes.take_available(2);
    let (path, revision) = &first[0];
    tab.folder_sizes
        .complete(path.clone(), *revision, result(100));
    assert!(tab.folder_sizes.publication_current(0, path, *revision));
    assert!(!tab.folder_sizes.current(0, path, *revision));
    tab.queue_missing_folder_sizes();
    assert!(!tab.folder_sizes.pending.contains_key(path));
    assert_eq!(tab.folder_sizes.take_available(2)[0].0, "C:/root/c");
    assert_eq!(tab.folder_sizes.ready_batch().len(), 1);
}

#[test]
fn invalidation_during_publication_keeps_one_followup_and_rejects_old_acknowledgements() {
    let mut tab = tab();
    tab.invalidate_folder_sizes(None);
    let first = tab.folder_sizes.take_available(2);
    let now = Instant::now();
    let (path, revision) = &first[0];
    tab.folder_sizes
        .complete_at(path.clone(), *revision, result(100), now);
    tab.invalidate_folder_sizes(Some(&HashSet::from([path.clone()])));
    assert!(tab.folder_sizes.publication_current(0, path, *revision));
    assert!(tab.folder_sizes.has_ready());
    let next = tab.folder_sizes.take_available_at(2, now);
    // The fair queue runs c before the invalidated a.
    tab.folder_sizes.finish(&next[0].0, next[0].1);
    tab.folder_sizes.published(&first);
    assert!(tab.folder_sizes.provisional_paths().contains(path));
    assert!(tab.folder_sizes.take_available_at(2, now).is_empty());
    let next = tab
        .folder_sizes
        .take_available_at(2, now + RESCAN_INTERVAL * 2);
    assert_eq!(next[0].0, *path);
    tab.folder_sizes
        .complete(path.clone(), next[0].1, result(200));
    tab.folder_sizes.published(&first);
    assert!(tab.folder_sizes.publication_current(0, path, next[0].1));
    tab.folder_sizes.published(&next);
    assert!(!tab.folder_sizes.has_ready());
}

#[test]
fn removed_recreated_or_navigated_folders_cannot_receive_waiting_publications() {
    for navigate in [false, true] {
        let mut tab = tab();
        tab.invalidate_folder_sizes(None);
        let first = tab.folder_sizes.take_available(2);
        let (path, revision) = &first[0];
        tab.folder_sizes
            .complete(path.clone(), *revision, result(100));
        if navigate {
            tab.reset_folder_sizes();
            tab.show_size_histogram = true;
        } else {
            tab.files = vec![folder("c", Some(30))].into();
            tab.queue_missing_folder_sizes();
            tab.files = vec![folder("a", None), folder("c", Some(30))].into();
            tab.queue_missing_folder_sizes();
        }
        assert!(!tab.folder_sizes.publication_current(0, path, *revision));
        assert!(!tab.folder_sizes.has_ready());
    }
}

#[test]
fn a_batch_updates_all_totals_and_flashes_only_changed_previously_calculated_folders() {
    let mut tab = tab();
    Arc::make_mut(&mut tab.files).push(folder("new", None));
    tab.sort_column = SortColumn::Size;
    tab.sort_direction = SortDirection::Descending;
    let before = tab.files.clone();
    let update = FileUpdate::FolderSizes(vec![
        ("C:/root/a".into(), result(100)),
        ("C:/root/b".into(), result(20)),
        ("C:/root/new".into(), result(200)),
    ]);
    assert!(tab.apply_preparation(tab.preparation(update).calculate()));
    assert_eq!(tab.filtered_files()[0].name, "new");
    assert_eq!(tab.files.iter().map(|file| file.size).sum::<u64>(), 350);
    assert_eq!(before[0].size, 10);
    assert_eq!(tab.folder_sizes.size_flashes().len(), 1);
    assert!(tab.folder_sizes.size_flashes().contains_key("C:/root/a"));
}

#[test]
fn publication_batches_are_bounded_and_preserve_the_backlog() {
    let mut sizes = FolderSizes::default();
    for index in 0..300 {
        let path = format!("folder-{index}");
        sizes.enqueue(path.clone());
        let job = sizes.take_available(1).pop().unwrap();
        sizes.complete(path, job.1, result(index));
    }
    let snapshot = sizes.clone();
    assert!(Arc::ptr_eq(&snapshot.ready, &sizes.ready));
    let mut published = 0;
    while sizes.has_ready() {
        let batch = sizes.ready_batch();
        assert!(batch.len() <= 128);
        published += batch.len();
        sizes.published(
            &batch
                .into_iter()
                .map(|(path, revision, _)| (path, revision))
                .collect::<Vec<_>>(),
        );
    }
    assert_eq!(published, 300);
    assert_eq!(snapshot.ready.len(), 300);
}

#[test]
#[ignore = "manual comparison of individual vs grouped list preparations"]
fn profile_grouped_folder_size_publication() {
    let mut source = tab();
    source.files = (0..2000)
        .map(|index| folder(&index.to_string(), Some(0)))
        .collect::<Vec<_>>()
        .into();
    let updates = (0..128)
        .map(|index| (format!("C:/root/{index}"), result(index + 1)))
        .collect::<Vec<_>>();
    let mut single = source.clone();
    let started = std::time::Instant::now();
    for update in &updates {
        assert!(single.apply_preparation(
            single
                .preparation(FileUpdate::FolderSizes(vec![update.clone()]))
                .calculate()
        ));
    }
    let individual = started.elapsed();
    let started = std::time::Instant::now();
    assert!(source.apply_preparation(
        source
            .preparation(FileUpdate::FolderSizes(updates))
            .calculate()
    ));
    let grouped = started.elapsed();
    assert_eq!(
        single.files.iter().map(|f| f.size).sum::<u64>(),
        source.files.iter().map(|f| f.size).sum::<u64>()
    );
    eprintln!(
        "publication: 2000 rows / 128 results; individual={individual:?}; grouped={grouped:?}"
    );
}
