use crate::ui::date_groups::DateGroup;
use crate::ui::workspace::{LayoutAxis, LayoutNode, SavedTab, SavedWorkspace, SortColumn, SortDirection, TabState, WorkspaceState};
use oxyde_lib::models::FileEntry;

#[test]
fn date_filter_keeps_previous_calendar_day_after_midnight() {
    use chrono::{Local, TimeZone};
    let now = Local
        .with_ymd_and_hms(2026, 10, 7, 0, 10, 0)
        .single()
        .unwrap();
    let modified = Local
        .with_ymd_and_hms(2026, 10, 6, 23, 50, 0)
        .single()
        .unwrap();
    let mut tab = TabState::new("date-filter".into(), "C:\\".into());
    tab.files = std::sync::Arc::new(vec![oxyde_lib::models::FileEntry {
        name: "previous-day.txt".into(),
        modified: modified.timestamp_millis() as u64,
        ..Default::default()
    }]);
    tab.filter_dates.insert(DateGroup::Today);
    tab.update_cache_on(now.date_naive());
    assert!(tab.filtered_files().is_empty());
    tab.filter_dates = std::collections::HashSet::from([DateGroup::Yesterday]);
    tab.update_cache_on(now.date_naive());
    assert_eq!(tab.filtered_files()[0].name, "previous-day.txt");
}

#[test]
fn date_filters_counts_and_select_all_share_the_six_calendar_groups() {
    use chrono::{Local, TimeZone};
    let today = chrono::NaiveDate::from_ymd_opt(2026, 10, 28).unwrap();
    let mut tab = TabState::new("date-filter".into(), "C:\\".into());
    tab.files = std::sync::Arc::new([
        (10, 29),
        (10, 28),
        (10, 27),
        (10, 26),
        (10, 25),
        (10, 19),
        (10, 18),
        (10, 1),
        (9, 30),
    ]
    .into_iter()
    .map(|(month, day)| oxyde_lib::models::FileEntry {
        name: format!("{month}-{day}.txt"),
        modified: Local
            .with_ymd_and_hms(2026, month, day, 12, 0, 0)
            .single()
            .unwrap()
            .timestamp_millis() as u64,
        ..Default::default()
    })
    .collect());
    std::sync::Arc::make_mut(&mut tab.files).push(oxyde_lib::models::FileEntry {
        name: "invalid.txt".into(),
        modified: u64::MAX,
        ..Default::default()
    });
    let counts = crate::ui::date_groups::counts_by_date(&tab.files, today);
    assert_eq!(counts.len(), DateGroup::ALL.len());
    for (group, count) in [
        (DateGroup::Today, 2),
        (DateGroup::Yesterday, 1),
        (DateGroup::ThisWeek, 1),
        (DateGroup::LastWeek, 2),
        (DateGroup::ThisMonth, 2),
        (DateGroup::Older, 2),
    ] {
        assert_eq!(counts[&group], count);
        tab.filter_dates = std::collections::HashSet::from([group]);
        tab.update_cache_on(today);
        assert_eq!(tab.filtered_files().len() as u64, count);
        assert!(tab
            .filtered_files()
            .iter()
            .all(|file| DateGroup::for_timestamp(file.modified, today) == group));
    }
    tab.filter_dates.extend(DateGroup::ALL);
    tab.update_cache_on(today);
    assert_eq!(
        tab.filtered_files()
            .iter()
            .map(|file| &file.name)
            .collect::<Vec<_>>(),
        tab.files.iter().map(|file| &file.name).collect::<Vec<_>>()
    );
    tab.filter_dates =
        std::collections::HashSet::from([DateGroup::Today, DateGroup::ThisMonth]);
    tab.filter_name = "10-1.".into();
    tab.update_cache_on(today);
    assert_eq!(tab.filtered_files().len(), 1);
    assert_eq!(tab.filtered_files()[0].name, "10-1.txt");
}

#[test]
fn monday_filter_uses_calendar_week_and_yesterday_precedence() {
    use chrono::{Local, TimeZone};
    let today = chrono::NaiveDate::from_ymd_opt(2026, 10, 5).unwrap();
    let mut tab = TabState::new("date-filter".into(), "C:\\".into());
    tab.files = std::sync::Arc::new([(10, 5), (10, 4), (10, 3), (9, 28), (9, 27)]
        .into_iter()
        .map(|(month, day)| oxyde_lib::models::FileEntry {
            name: format!("{month}-{day}.txt"),
            modified: Local
                .with_ymd_and_hms(2026, month, day, 12, 0, 0)
                .single()
                .unwrap()
                .timestamp_millis() as u64,
            ..Default::default()
        })
        .collect());
    tab.filter_dates.insert(DateGroup::ThisWeek);
    tab.update_cache_on(today);
    assert!(tab.filtered_files().is_empty());
    tab.filter_dates = std::collections::HashSet::from([DateGroup::LastWeek]);
    tab.update_cache_on(today);
    assert_eq!(
        tab.filtered_files()
            .iter()
            .map(|file| file.name.as_str())
            .collect::<Vec<_>>(),
        vec!["10-3.txt", "9-28.txt"]
    );
    tab.filter_dates = std::collections::HashSet::from([DateGroup::Yesterday]);
    tab.update_cache_on(today);
    assert_eq!(tab.filtered_files()[0].name, "10-4.txt");
}

#[test]
fn trash_location_filter_matches_displayed_parent_and_combines_with_name() {
    let mut tab = TabState::new("trash".into(), "trash://".into());
    tab.files = std::sync::Arc::new(vec![
        oxyde_lib::models::FileEntry { name: "report.txt".into(), original_path: Some(std::path::Path::new("Documents").join("report.txt").to_string_lossy().into_owned()), ..Default::default() },
        oxyde_lib::models::FileEntry { name: "Documents.txt".into(), original_path: Some(std::path::Path::new("Downloads").join("Documents.txt").to_string_lossy().into_owned()), ..Default::default() },
        oxyde_lib::models::FileEntry { name: "missing".into(), ..Default::default() },
    ]);
    tab.filter_original_location = "DOCUMENTS".into();
    tab.update_cache();
    assert_eq!(tab.cached_filtered_files.len(), 1);
    assert_eq!(tab.cached_filtered_files[0].name, "report.txt");
    tab.filter_name = "other".into();
    tab.update_cache();
    assert!(tab.cached_filtered_files.is_empty());
    tab.filter_name.clear();
    tab.filter_original_location.clear();
    tab.update_cache();
    assert_eq!(tab.cached_filtered_files.len(), 3);
}
#[test]
fn trash_sort_uses_original_location_and_deletion_time() {
    let mut tab = TabState::new("trash".into(), "trash://".into());
    tab.files = std::sync::Arc::new(vec![
        oxyde_lib::models::FileEntry { name: "A".into(), path: "id-a".into(),
            original_path: Some(std::path::Path::new("z-folder").join("A").to_string_lossy().into_owned()),
            deleted_time: Some(100), ..Default::default() },
        oxyde_lib::models::FileEntry { name: "Z".into(), path: "id-z".into(),
            original_path: Some(std::path::Path::new("a-folder").join("Z").to_string_lossy().into_owned()),
            deleted_time: Some(200), ..Default::default() },
    ]);
    tab.sort_column = SortColumn::OriginalLocation;
    tab.sort_direction = SortDirection::Ascending;
    tab.sort_files();
    assert_eq!(tab.files[0].name, "Z");
    tab.sort_column = SortColumn::DeletedAt;
    tab.sort_files();
    assert_eq!(tab.files[0].name, "A");
    tab.sort_direction = SortDirection::Descending;
    tab.sort_files();
    assert_eq!(tab.files[0].name, "Z");
}

#[test]
fn new_panel_is_full_height_after_stacked_panes_and_preserves_tabs() {
    let mut state = WorkspaceState::new();
    let original = state.active_pane_id.clone();
    state.add_tab(&original, "stacked-path".into(), true);
    state.split_tab_to_axis(&original, 1, &original, LayoutAxis::Vertical, true);
    let stacked = state.active_pane_id.clone();
    let existing_tab = state.find_pane(&stacked).unwrap().active_tab().id.clone();
    let added = state.add_panel_right().unwrap();
    assert_eq!(state.find_pane(&added).unwrap().active_tab().path,
        if cfg!(target_os = "windows") { "C:\\" } else { "/" });
    assert_eq!(state.find_pane(&stacked).unwrap().active_tab().id, existing_tab);
    let LayoutNode::Split { axis: LayoutAxis::Horizontal, children, .. } = &state.layout else {
        panic!("Expected full-height columns");
    };
    assert!(matches!(&children[0], LayoutNode::Split { axis: LayoutAxis::Vertical, .. }));
    assert!(matches!(&children[1], LayoutNode::Pane(pane) if pane.id == added));
    // Adding from the left stack inserts beside it, before the existing right column.
    state.active_pane_id = original;
    let middle = state.add_panel_right().unwrap();
    let LayoutNode::Split { children, weights, .. } = &state.layout else { panic!() };
    assert_eq!(children.len(), 3);
    assert_eq!(weights.len(), 3);
    assert!(matches!(&children[1], LayoutNode::Pane(pane) if pane.id == middle));
    assert!(matches!(&children[2], LayoutNode::Pane(pane) if pane.id == added));
}



#[test]
fn new_tabs_open_after_current_with_or_without_activation() {
    for activate in [false, true] {
        let mut state = WorkspaceState::new();
        let pane_id = state.active_pane_id.clone();
        state.add_tab(&pane_id, "B".to_string(), true);
        state.add_tab(&pane_id, "C".to_string(), true);
        let pane = state.find_pane_mut(&pane_id).unwrap();
        pane.active_tab_index = 1;
        let current_id = pane.active_tab().id.clone();

        state.add_tab(&pane_id, "new".to_string(), activate);

        let pane = state.find_pane(&pane_id).unwrap();
        assert_eq!(pane.tabs[1].id, current_id);
        assert_eq!(pane.tabs[2].path, "new");
        assert_eq!(pane.tabs[3].path, "C");
        assert_eq!(pane.active_tab_index, if activate { 2 } else { 1 });
    }
}

#[test]
fn closing_tab_before_active_preserves_active_tab_identity() {
    let mut state = WorkspaceState::new();
    let pane_id = state.active_pane_id.clone();
    state.add_tab(&pane_id, "B".to_string(), true);
    state.add_tab(&pane_id, "C".to_string(), false);
    let active_id = state.find_pane(&pane_id).unwrap().active_tab().id.clone();

    state.close_tab_id(&pane_id, 0);

    let pane = state.find_pane(&pane_id).unwrap();
    assert_eq!(pane.active_tab().id, active_id);
}

#[test]
fn invalid_tab_close_is_ignored() {
    let mut state = WorkspaceState::new();
    let pane_id = state.active_pane_id.clone();
    let initial_len = state.find_pane(&pane_id).unwrap().tabs.len();

    state.close_tab_id(&pane_id, usize::MAX);

    assert_eq!(state.find_pane(&pane_id).unwrap().tabs.len(), initial_len);
}

#[test]
fn tab_ids_are_not_reused_after_close() {
    let mut state = WorkspaceState::new();
    let pane_id = state.active_pane_id.clone();
    state.add_tab(&pane_id, "B".to_string(), true);
    let removed_id = state.find_pane(&pane_id).unwrap().active_tab().id.clone();
    state.close_tab_id(&pane_id, 1);
    state.add_tab(&pane_id, "C".to_string(), true);
    let new_id = state.find_pane(&pane_id).unwrap().active_tab().id.clone();

    assert_ne!(removed_id, new_id);
}

#[test]
fn reordering_an_inactive_tab_preserves_the_active_tab() {
    let mut state = WorkspaceState::new();
    let pane_id = state.active_pane_id.clone();
    state.add_tab(&pane_id, "B".to_string(), true);
    state.add_tab(&pane_id, "C".to_string(), false);
    let active_id = state.find_pane(&pane_id).unwrap().active_tab().id.clone();

    state.reorder_tabs_id(&pane_id, 2, 0);

    assert_eq!(
        state.find_pane(&pane_id).unwrap().active_tab().id,
        active_id
    );
}

#[test]
fn moving_to_an_unknown_pane_keeps_the_tab_and_active_selection() {
    let mut state = WorkspaceState::new();
    let pane_id = state.active_pane_id.clone();
    state.add_tab(&pane_id, "B".to_string(), true);
    let tabs_before = state.find_pane(&pane_id).unwrap().tabs.clone();
    let active_id = state.find_pane(&pane_id).unwrap().active_tab().id.clone();

    state.move_tab_to_pane_id(&pane_id, 0, "missing-pane");

    let pane = state.find_pane(&pane_id).unwrap();
    assert_eq!(pane.tabs.len(), tabs_before.len());
    assert_eq!(pane.active_tab().id, active_id);
}

#[test]
fn splitting_a_tab_preserves_both_tab_identities_and_source_selection() {
    let mut state = WorkspaceState::new();
    let source_pane_id = state.active_pane_id.clone();
    state.add_tab(&source_pane_id, "second".to_string(), true);
    let moved_id = state.find_pane(&source_pane_id).unwrap().tabs[0].id.clone();
    let active_id = state
        .find_pane(&source_pane_id)
        .unwrap()
        .active_tab()
        .id
        .clone();

    state.split_tab_to_axis(
        &source_pane_id,
        0,
        &source_pane_id,
        LayoutAxis::Horizontal,
        true,
    );

    assert_eq!(
        state.find_pane(&source_pane_id).unwrap().active_tab().id,
        active_id
    );
    assert!(state.find_tab_mut(&moved_id).is_some());
    let mut pane_count = 0;
    state.layout.visit_panes(&mut |_| pane_count += 1);
    assert_eq!(pane_count, 2);
}

#[test]
fn saved_workspace_restores_split_panels_and_open_tab_paths() {
    let mut state = WorkspaceState::new();
    let source_pane_id = state.active_pane_id.clone();
    state.add_tab(&source_pane_id, "D:/first".to_string(), true);
    state.add_tab(&source_pane_id, "E:/active".to_string(), true);
    state.split_tab_to_axis(
        &source_pane_id,
        2,
        &source_pane_id,
        LayoutAxis::Horizontal,
        true,
    );

    let active_pane_id = state.active_pane_id.clone();
    state.add_tab(&active_pane_id, "F:/second".to_string(), true);
    state.find_pane_mut(&active_pane_id).unwrap().active_tab_mut().group_by_date = true;
    if let LayoutNode::Split { weights, .. } = &mut state.layout {
        *weights = vec![1.4, 0.6];
    } else {
        panic!("expected a split layout");
    }

    let value = serde_json::to_value(state.saved_snapshot(320.0)).unwrap();
    let mut legacy_value = value.clone();
    legacy_value
        .as_object_mut()
        .unwrap()
        .remove("sidebar_collapsed");
    let legacy: SavedWorkspace = serde_json::from_value(legacy_value).unwrap();
    assert!(!legacy.sidebar_collapsed);
    let mut collapsed_value = value.clone();
    collapsed_value["sidebar_collapsed"] = serde_json::json!(true);
    let collapsed: SavedWorkspace = serde_json::from_value(collapsed_value).unwrap();
    assert!(collapsed.sidebar_collapsed);
    assert_eq!(collapsed.sidebar_width, 320.0);
    let snapshot: SavedWorkspace = serde_json::from_value(value).unwrap();
    let (restored, sidebar_width) = WorkspaceState::from_saved_snapshot(snapshot).unwrap();

    assert_eq!(sidebar_width, 320.0);
    assert_eq!(restored.active_pane_id, active_pane_id);
    let pane = restored.find_pane(&active_pane_id).unwrap();
    assert_eq!(pane.tabs.len(), 2);
    assert_eq!(pane.tabs[0].path, "E:/active");
    assert_eq!(pane.active_tab().path, "F:/second");
    assert!(pane.active_tab().group_by_date);
    assert!(!pane.tabs[0].group_by_date);
    match &restored.layout {
        LayoutNode::Split { weights, .. } => assert_eq!(weights, &[1.4, 0.6]),
        LayoutNode::Pane(_) => panic!("expected split layout after restore"),
    }
}

#[test]
fn date_grouping_is_optional_in_legacy_tabs_and_independent_in_duplicates() {
    let old: SavedTab = serde_json::from_str(r#"{"id":"old","path":"C:/"}"#).unwrap();
    assert!(!old.group_by_date);
    let mut state = WorkspaceState::new();
    let pane_id = state.active_pane_id.clone();
    state.find_pane_mut(&pane_id).unwrap().active_tab_mut().group_by_date = true;
    let index = state.duplicate_tab(&pane_id, 0).unwrap();
    let pane = state.find_pane_mut(&pane_id).unwrap();
    assert!(pane.tabs[index].group_by_date);
    pane.tabs[index].group_by_date = false;
    assert!(pane.tabs[0].group_by_date);
}

#[test]
fn selecting_every_present_type_remains_an_explicit_filter_state() {
    let mut tab = TabState::new("tab".to_string(), String::new());
    tab.files = std::sync::Arc::new(vec![FileEntry {
        name: "notes.txt".to_string(),
        ..FileEntry::default()
    }]);
    tab.filter_types.insert("TXT".to_string());

    tab.update_cache();

    assert!(tab.filter_types.contains("TXT"));
    assert_eq!(tab.cached_filtered_files.len(), 1);
}
