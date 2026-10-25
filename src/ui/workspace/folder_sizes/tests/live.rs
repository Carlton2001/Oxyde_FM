use super::*;
use crate::ui::workspace::FileUpdate;

fn size(value: u64) -> FolderSizeResult {
    FolderSizeResult {
        size: value,
        files_count: 1,
        folders_count: 0,
    }
}

#[test]
fn initial_watcher_installation_does_not_dirty_the_first_two_measurements() {
    for root in ["C:/", "D:/", "C:/fixture/nested"] {
        let mut tab = TabState::new("tab".into(), root.into());
        tab.show_size_histogram = true;
        tab.files = ["first", "second", "third"]
            .into_iter()
            .map(|name| FileEntry {
                name: name.into(),
                path: format!("{root}/{name}"),
                is_dir: true,
                ..Default::default()
            })
            .collect::<Vec<_>>()
            .into();
        tab.queue_missing_folder_sizes();
        let revisions = tab
            .folder_sizes
            .pending
            .iter()
            .map(|(path, job)| (path.clone(), job.revision))
            .collect::<HashMap<_, _>>();
        // Initial requests have not started until installation completes.
        tab.reconcile_folder_size_watcher();
        assert!(tab.folder_sizes.provisional_paths().is_empty());
        for (path, revision) in revisions {
            assert_eq!(tab.folder_sizes.pending[&path].revision, revision);
        }
        let first = tab.folder_sizes.take_available(2);
        assert_eq!(first.len(), 2);
        for (path, revision) in &first {
            tab.folder_sizes
                .complete(path.clone(), *revision, size(100));
        }
        assert!(tab.folder_sizes.provisional_paths().is_empty());
        tab.folder_sizes.published(&first);
        assert_eq!(tab.folder_sizes.pending.len(), 1);
        assert_eq!(tab.folder_sizes.queue.len(), 1);
        assert!(tab.folder_sizes.provisional_paths().is_empty());
        assert_eq!(tab.folder_sizes.take_available(2).len(), 1);
    }
}

#[test]
fn changes_before_first_scan_are_included_without_a_provisional_marker() {
    let mut sizes = FolderSizes::default();
    let now = Instant::now();
    for _ in 0..1000 {
        sizes.enqueue_at("waiting".into(), now);
    }
    assert!(sizes.provisional_paths().is_empty());
    let job = sizes.take_available_at(1, now);
    sizes.complete_at("waiting".into(), job[0].1, size(100), now);
    assert!(!sizes.has_pending());
    assert!(sizes.provisional_paths().is_empty());
}

#[test]
fn replacement_watchers_only_refresh_measurements_that_could_have_crossed_the_gap() {
    let mut tab = TabState::new("tab".into(), "C:/".into());
    tab.show_size_histogram = true;
    tab.files = ["started", "queued"]
        .into_iter()
        .map(|name| FileEntry {
            name: name.into(),
            path: format!("C:/{name}"),
            is_dir: true,
            ..Default::default()
        })
        .collect::<Vec<_>>()
        .into();
    tab.queue_missing_folder_sizes();
    let started = tab.folder_sizes.take_available(1);
    let queued_revision = tab.folder_sizes.pending["C:/queued"].revision;
    tab.reconcile_folder_size_watcher();
    assert!(tab.folder_sizes.pending.contains_key(&started[0].0));
    assert!(tab.folder_sizes.provisional_paths().contains(&started[0].0));
    assert_eq!(
        tab.folder_sizes.pending["C:/queued"].revision,
        queued_revision
    );
    assert!(!tab.folder_sizes.provisional_paths().contains("C:/queued"));
    tab.folder_sizes
        .complete(started[0].0.clone(), started[0].1, size(100));
    assert!(tab
        .folder_sizes
        .publication_current(0, &started[0].0, started[0].1));
}

#[test]
fn continuous_changes_publish_first_measurement_but_suspend_automatic_refreshes_until_quiet() {
    let mut sizes = FolderSizes::default();
    let start = Instant::now();
    let path = "C:/Windows".to_owned();
    sizes.enqueue_at(path.clone(), start);
    let first = sizes.take_available_at(1, start);
    for index in 0..10_000 {
        sizes.enqueue_at(path.clone(), start + Duration::from_millis(index));
    }
    let completed = start + Duration::from_secs(10);
    sizes.complete_at(path.clone(), first[0].1, size(100), completed);
    assert!(sizes.publication_current(0, &path, first[0].1));
    assert_eq!(sizes.ready_batch()[0].2.size, 100);
    sizes.published(&first);
    assert!(sizes.provisional_paths().contains(&path));
    assert_eq!(sizes.pending.len(), 1);
    assert_eq!(sizes.queue.len(), 1);
    for index in 0..5_000 {
        let now = completed + Duration::from_millis(index);
        sizes.enqueue_at(path.clone(), now);
        assert!(sizes.take_available_at(1, now).is_empty());
    }
    assert!(sizes
        .take_available_at(1, completed + RESCAN_INTERVAL)
        .is_empty());
    let last_change = completed + Duration::from_millis(4_999);
    assert!(sizes
        .take_available_at(1, last_change + RESCAN_INTERVAL - Duration::from_millis(1))
        .is_empty());
    assert_eq!(
        sizes.take_available_at(1, last_change + RESCAN_INTERVAL)[0].0,
        path
    );
}

#[test]
fn first_measurements_take_priority_over_due_repeat_scans() {
    let mut sizes = FolderSizes::default();
    let now = Instant::now();
    sizes.enqueue_at("hot".into(), now);
    let first = sizes.take_available_at(1, now);
    sizes.complete_at("hot".into(), first[0].1, size(1), now);
    sizes.published(&first);
    sizes.enqueue_at("hot".into(), now);
    sizes.enqueue_at("unmeasured".into(), now);
    let next = sizes.take_available_at(1, now + RESCAN_INTERVAL);
    assert_eq!(next[0].0, "unmeasured");
    assert_eq!(
        sizes.take_available_at(1, now + RESCAN_INTERVAL)[0].0,
        "hot"
    );
}

#[test]
fn cooldown_starts_at_completion_and_prevents_immediate_retry_of_a_long_scan() {
    let mut sizes = FolderSizes::default();
    let now = Instant::now();
    sizes.enqueue_at("hot".into(), now);
    let first = sizes.take_available_at(1, now);
    sizes.enqueue_at("hot".into(), now + Duration::from_secs(1));
    let completed = now + Duration::from_secs(30);
    sizes.complete_at("hot".into(), first[0].1, size(1), completed);
    // Preserve a completed value until publication even if its retry is due.
    assert!(sizes
        .take_available_at(1, completed + RESCAN_INTERVAL)
        .is_empty());
    sizes.published(&first);
    assert!(sizes.has_pending());
    assert!(sizes.take_available_at(1, completed).is_empty());
    assert_eq!(sizes.running.len(), 0);
    assert_eq!(
        sizes
            .take_available_at(1, completed + RESCAN_INTERVAL)
            .len(),
        1
    );
}

#[test]
fn repeated_dirty_scans_keep_publishing_and_last_clean_scan_clears_the_marker() {
    let mut sizes = FolderSizes::default();
    let mut now = Instant::now();
    sizes.enqueue_at("hot".into(), now);
    for value in 1..=4 {
        let job = sizes.take_available_at(1, now);
        assert_eq!(job.len(), 1);
        if value < 4 {
            sizes.enqueue_at("hot".into(), now);
        }
        sizes.complete_at("hot".into(), job[0].1, size(value), now);
        assert_eq!(sizes.ready_batch()[0].2.size, value);
        sizes.published(&job);
        assert_eq!(sizes.provisional_paths().contains("hot"), value < 4);
        now += RESCAN_INTERVAL;
    }
    assert!(!sizes.has_pending());
    assert!(!sizes.has_ready());
}

#[test]
fn changes_between_preparation_and_publication_keep_the_measurement_visible_as_provisional() {
    let mut tab = TabState::new("tab".into(), "C:/".into());
    tab.show_size_histogram = true;
    tab.files = vec![FileEntry {
        name: "Users".into(),
        path: "C:/Users".into(),
        is_dir: true,
        ..Default::default()
    }]
    .into();
    tab.queue_missing_folder_sizes();
    let job = tab.folder_sizes.take_available(1);
    tab.folder_sizes
        .complete(job[0].0.clone(), job[0].1, size(100));
    let prepared = tab
        .preparation(FileUpdate::FolderSizes(vec![(job[0].0.clone(), size(100))]))
        .calculate();
    tab.invalidate_folder_sizes(Some(&HashSet::from([job[0].0.clone()])));
    assert!(tab.folder_sizes.publication_current(0, &job[0].0, job[0].1));
    assert!(tab.apply_preparation(prepared));
    tab.folder_sizes.published(&job);
    assert!(tab.files[0].is_calculated);
    assert_eq!(tab.files[0].size, 100);
    assert!(tab.folder_sizes.provisional_paths().contains(&job[0].0));
}

#[test]
fn removal_and_recreation_still_reject_dirty_running_and_ready_measurements() {
    for complete_first in [false, true] {
        let mut tab = TabState::new("tab".into(), "C:/".into());
        tab.show_size_histogram = true;
        let entry = FileEntry {
            name: "Users".into(),
            path: "C:/Users".into(),
            is_dir: true,
            ..Default::default()
        };
        tab.files = vec![entry.clone()].into();
        tab.queue_missing_folder_sizes();
        let job = tab.folder_sizes.take_available(1);
        tab.invalidate_folder_sizes(None);
        if complete_first {
            tab.folder_sizes
                .complete(job[0].0.clone(), job[0].1, size(100));
        }
        tab.files = Default::default();
        tab.queue_missing_folder_sizes();
        tab.files = vec![entry].into();
        tab.queue_missing_folder_sizes();
        tab.folder_sizes
            .complete(job[0].0.clone(), job[0].1, size(100));
        assert!(!tab.folder_sizes.publication_current(0, &job[0].0, job[0].1));
        assert!(!tab.folder_sizes.has_ready());
    }
}

#[test]
fn real_folder_changes_produce_a_first_estimate_then_a_fresh_exact_total() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("file"), [0; 10]).unwrap();
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let path = root.path().to_string_lossy().into_owned();
    let mut sizes = FolderSizes::default();
    let now = Instant::now();
    sizes.enqueue_at(path.clone(), now);
    let first = sizes.take_available_at(1, now);
    let initial = runtime
        .block_on(oxyde_lib::commands::io::calculate_folder_size(path.clone()))
        .unwrap();
    std::fs::write(root.path().join("file"), [0; 70]).unwrap();
    sizes.enqueue_at(path.clone(), now);
    sizes.complete_at(path.clone(), first[0].1, initial, now);
    assert_eq!(sizes.ready_batch()[0].2.size, 10);
    sizes.published(&first);
    assert!(sizes.provisional_paths().contains(&path));
    let next = sizes.take_available_at(1, now + RESCAN_INTERVAL);
    let latest = runtime
        .block_on(oxyde_lib::commands::io::calculate_folder_size(path.clone()))
        .unwrap();
    sizes.complete_at(path.clone(), next[0].1, latest, now + RESCAN_INTERVAL);
    assert_eq!(sizes.ready_batch()[0].2.size, 70);
    sizes.published(&next);
    assert!(!sizes.provisional_paths().contains(&path));
}

#[test]
fn a_hot_folder_does_not_restart_while_a_quiet_neighbor_updates() {
    let mut sizes = FolderSizes::default();
    let now = Instant::now();
    for path in ["hot", "quiet"] {
        sizes.enqueue_at(path.into(), now);
    }
    let first = sizes.take_available_at(2, now);
    for (path, revision) in &first {
        sizes.complete_at(path.clone(), *revision, size(100), now);
    }
    sizes.published(&first);
    sizes.enqueue_at("quiet".into(), now);
    let mut quiet_refreshes = 0;
    for second in 1..=60 {
        let time = now + Duration::from_secs(second);
        sizes.enqueue_at("hot".into(), time);
        for (path, revision) in sizes.take_available_at(2, time) {
            assert_eq!(path, "quiet", "a busy folder started a disk traversal");
            quiet_refreshes += 1;
            sizes.complete_at(path.clone(), revision, size(200), time);
            sizes.published(&[(path, revision)]);
        }
    }
    assert_eq!(quiet_refreshes, 1);
    assert_eq!(sizes.pending.len(), 1);
    assert!(sizes.provisional_paths().contains("hot"));
    assert!(!sizes.provisional_paths().contains("quiet"));
    let after_quiet = sizes.take_available_at(1, now + Duration::from_secs(65));
    assert_eq!(after_quiet.len(), 1);
    assert_eq!(after_quiet[0].0, "hot");
}

#[test]
fn manual_refresh_survives_watcher_events_without_enabling_continuous_automatic_scans() {
    let mut sizes = FolderSizes::default();
    let now = Instant::now();
    sizes.enqueue_at("hot".into(), now);
    let first = sizes.take_available_at(1, now);
    sizes.complete_at("hot".into(), first[0].1, size(100), now);
    sizes.published(&first);
    sizes.enqueue_at("hot".into(), now + Duration::from_secs(1));
    sizes.force_at("hot".into(), now + Duration::from_secs(2));
    sizes.enqueue_at("hot".into(), now + Duration::from_secs(3));
    let manual = sizes.take_available_at(1, now + Duration::from_secs(3));
    assert_eq!(manual.len(), 1);
    sizes.enqueue_at("hot".into(), now + Duration::from_secs(4));
    sizes.complete_at(
        "hot".into(),
        manual[0].1,
        size(200),
        now + Duration::from_secs(5),
    );
    sizes.published(&manual);
    for second in 6..=60 {
        let time = now + Duration::from_secs(second);
        sizes.enqueue_at("hot".into(), time);
        assert!(sizes.take_available_at(1, time).is_empty());
    }
    assert_eq!(sizes.pending.len(), 1);
    assert_eq!(
        sizes
            .take_available_at(1, now + Duration::from_secs(65))
            .len(),
        1
    );
}

#[test]
fn manual_refresh_during_a_scan_keeps_one_immediate_followup_after_publication() {
    let mut sizes = FolderSizes::default();
    let now = Instant::now();
    sizes.enqueue_at("hot".into(), now);
    let first = sizes.take_available_at(1, now);
    for _ in 0..100 {
        sizes.force_at("hot".into(), now);
    }
    sizes.enqueue_at("hot".into(), now);
    sizes.complete_at("hot".into(), first[0].1, size(100), now);
    assert!(sizes.take_available_at(1, now).is_empty());
    sizes.published(&first);
    assert_eq!(sizes.take_available_at(1, now).len(), 1);
    assert!(!sizes.has_pending());
}

#[test]
fn tab_refresh_explicitly_recalculates_existing_values_without_waiting_for_quiet() {
    let mut tab = TabState::new("tab".into(), "C:/".into());
    tab.show_size_histogram = true;
    tab.files = vec![FileEntry {
        name: "Users".into(),
        path: "C:/Users".into(),
        is_dir: true,
        ..Default::default()
    }]
    .into();
    tab.queue_missing_folder_sizes();
    let first = tab.folder_sizes.take_available(1);
    tab.folder_sizes
        .complete(first[0].0.clone(), first[0].1, size(100));
    assert!(tab.apply_preparation(
        tab.preparation(FileUpdate::FolderSizes(vec![(
            first[0].0.clone(),
            size(100)
        )]))
        .calculate()
    ));
    tab.folder_sizes.published(&first);
    tab.invalidate_folder_sizes(None);
    assert!(tab.folder_sizes.take_available(1).is_empty());
    tab.refresh_folder_sizes();
    tab.invalidate_folder_sizes(None);
    assert_eq!(tab.folder_sizes.take_available(1).len(), 1);
    assert_eq!(tab.files[0].size, 100);
}

#[test]
fn refresh_preserves_clean_sizes_and_forces_only_dirty_or_unmeasured_folders() {
    let mut tab = TabState::new("tab".into(), "C:/".into());
    tab.show_size_histogram = true;
    let folder = |name: &str, calculated| FileEntry {
        name: name.into(),
        path: format!("C:/{name}"),
        is_dir: true,
        is_calculated: calculated,
        size: 100,
        ..Default::default()
    };
    let mut link = folder("link", false);
    link.is_symlink = true;
    let mut junction = folder("junction", false);
    junction.is_junction = true;
    tab.files = vec![
        folder("clean", true),
        folder("dirty", true),
        folder("new", false),
        link,
        junction,
    ]
    .into();
    let previous = tab.files.clone();
    tab.invalidate_folder_sizes(Some(&HashSet::from(["C:/dirty".into()])));
    tab.refresh_folder_sizes();
    let jobs = tab.folder_sizes.take_available(2);
    assert_eq!(
        jobs.iter()
            .map(|(path, _)| path.as_str())
            .collect::<HashSet<_>>(),
        HashSet::from(["C:/dirty", "C:/new"])
    );
    assert!(Arc::ptr_eq(&previous, &tab.files));
    assert!(!tab.folder_sizes.pending.contains_key("C:/clean"));
}

#[test]
fn refresh_does_not_duplicate_clean_running_scans_or_completed_results_waiting_for_publication() {
    let mut tab = TabState::new("tab".into(), "C:/".into());
    tab.show_size_histogram = true;
    tab.files = ["first", "second"]
        .into_iter()
        .map(|name| FileEntry {
            name: name.into(),
            path: format!("C:/{name}"),
            is_dir: true,
            ..Default::default()
        })
        .collect::<Vec<_>>()
        .into();
    tab.queue_missing_folder_sizes();
    let first = tab.folder_sizes.take_available(2);
    tab.refresh_folder_sizes();
    assert!(!tab.folder_sizes.has_pending());
    assert_eq!(tab.folder_sizes.running.len(), 2);
    tab.folder_sizes
        .complete(first[0].0.clone(), first[0].1, size(100));
    tab.refresh_folder_sizes();
    assert!(!tab.folder_sizes.has_pending());
    assert!(tab
        .folder_sizes
        .publication_current(0, &first[0].0, first[0].1));
}

#[test]
fn refresh_retries_a_failed_dirty_scan_and_keeps_clean_neighbors() {
    let mut tab = TabState::new("tab".into(), "C:/".into());
    tab.show_size_histogram = true;
    tab.files = ["clean", "dirty"]
        .into_iter()
        .map(|name| FileEntry {
            name: name.into(),
            path: format!("C:/{name}"),
            is_dir: true,
            ..Default::default()
        })
        .collect::<Vec<_>>()
        .into();
    tab.queue_missing_folder_sizes();
    let first = tab.folder_sizes.take_available(2);
    for (path, revision) in &first {
        tab.folder_sizes
            .complete(path.clone(), *revision, size(100));
    }
    let update = FileUpdate::FolderSizes(
        first
            .iter()
            .map(|(path, _)| (path.clone(), size(100)))
            .collect(),
    );
    assert!(tab.apply_preparation(tab.preparation(update).calculate()));
    tab.folder_sizes.published(&first);
    tab.invalidate_folder_sizes(Some(&HashSet::from(["C:/dirty".into()])));
    tab.refresh_folder_sizes();
    let failed = tab.folder_sizes.take_available(1);
    assert_eq!(failed[0].0, "C:/dirty");
    tab.folder_sizes.finish(&failed[0].0, failed[0].1);
    assert!(!tab.folder_sizes.has_pending());
    tab.refresh_folder_sizes();
    let retry = tab.folder_sizes.take_available(2);
    assert_eq!(retry.len(), 1);
    assert_eq!(retry[0].0, "C:/dirty");
    assert!(tab.files.iter().all(|file| file.size == 100));
}

#[test]
fn refreshed_listing_preserves_clean_totals_and_only_measures_new_folders() {
    let mut tab = TabState::new("tab".into(), "C:/".into());
    tab.show_size_histogram = true;
    let folder = |name: &str, calculated| FileEntry {
        name: name.into(),
        path: format!("C:/{name}"),
        is_dir: true,
        is_calculated: calculated,
        size: if calculated { 100 } else { 0 },
        ..Default::default()
    };
    tab.files = vec![folder("clean", true)].into();
    tab.refresh_folder_sizes();
    assert!(!tab.folder_sizes.has_pending());
    let listing = FileUpdate::Listing(vec![folder("clean", false), folder("new", false)].into());
    assert!(tab.apply_preparation(tab.preparation(listing).calculate()));
    tab.queue_missing_folder_sizes();
    assert_eq!(
        tab.files
            .iter()
            .find(|file| file.name == "clean")
            .unwrap()
            .size,
        100
    );
    let jobs = tab.folder_sizes.take_available(2);
    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs[0].0, "C:/new");
}

#[test]
fn a_full_watcher_invalidation_still_forces_all_sizes_on_refresh() {
    let mut tab = TabState::new("tab".into(), "C:/".into());
    tab.show_size_histogram = true;
    tab.files = ["first", "second"]
        .into_iter()
        .map(|name| FileEntry {
            name: name.into(),
            path: format!("C:/{name}"),
            is_dir: true,
            is_calculated: true,
            size: 100,
            ..Default::default()
        })
        .collect::<Vec<_>>()
        .into();
    tab.invalidate_folder_sizes(None);
    tab.refresh_folder_sizes();
    assert_eq!(tab.folder_sizes.take_available(2).len(), 2);
}
