use super::*;
use std::time::Instant;

#[test]
fn repeated_large_pane_snapshots_reuse_files_rows_and_geometry() {
    for count in [20_000, 100_000] {
        let mut tab = TabState::new("pane-1".into(), "fixture".into());
        tab.files = (0..count)
            .map(|index| FileEntry {
                name: format!("file-{index}"),
                path: format!("C:/fixture/file-{index}"),
                size: index as u64,
                ..Default::default()
            })
            .collect::<Vec<_>>()
            .into();
        tab.update_cache();
        let initial = tab.presentation(false, 1);
        let second = tab.clone();
        let started = Instant::now();
        for _ in 0..1000 {
            assert!(Arc::ptr_eq(&initial, &tab.presentation(false, 1)));
            assert!(Arc::ptr_eq(&initial, &second.presentation(false, 1)));
        }
        let reuse = started.elapsed();
        let started = Instant::now();
        for _ in 0..10 {
            let files = tab.filtered_files().to_vec();
            let rows = crate::ui::date_groups::display_rows(&files, crate::ui::date_groups::GroupMode::None, &Default::default(), 1);
            std::hint::black_box((files, rows));
        }
        eprintln!(
            "pane snapshot: entries={count}; shared_2000={reuse:?}; legacy_clones_and_rows_10={:?}",
            started.elapsed()
        );
        assert!(Arc::ptr_eq(&initial.files, &tab.cached_filtered_files));
        tab.selected_paths.insert("C:/fixture/file-0".into());
        assert!(Arc::ptr_eq(&initial, &tab.presentation(false, 1)));
        let grid = tab.presentation(true, 4);
        let resized = tab.presentation(true, 7);
        assert!(!Arc::ptr_eq(&grid.rows, &resized.rows));
        assert!(Arc::ptr_eq(&grid.files, &resized.files));
        tab.group_mode = crate::ui::date_groups::GroupMode::Date;
        let grouped = tab.presentation(true, 7);
        tab.collapsed_groups.insert(GroupKey::Date(crate::ui::date_groups::DateGroup::Older));
        let collapsed = tab.presentation(true, 7);
        assert!(collapsed.rows.len() < grouped.rows.len());
        let old = Arc::downgrade(&collapsed);
        drop(collapsed);
        tab.clear_files();
        assert!(old.upgrade().is_none());
        assert_eq!(initial.files.len(), count);
        assert!(tab.presentation(false, 1).rows.is_empty());
    }
}
