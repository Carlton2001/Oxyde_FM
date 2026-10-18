use oxyde_lib::models::FileEntry;
use crate::ui::date_groups::{CalendarReference, DateGroup};

use crate::ui::workspace::{LayoutAxis, LayoutNode, PaneState, TabState};
use chrono::{Duration, Local, NaiveDate, TimeZone};

fn reference(day: NaiveDate) -> CalendarReference {
    CalendarReference {
        today: day,
        utc_offset: 7200,
        timezone: Some("fixture-zone".into()),
    }
}
fn tab(id: &str, day: NaiveDate, group: DateGroup) -> TabState {
    let timestamp = Local
        .from_local_datetime(&day.and_hms_opt(12, 0, 0).unwrap())
        .single()
        .unwrap();
    let mut tab = TabState::new(id.into(), "fixture".into());
    tab.files = std::sync::Arc::new(vec![FileEntry {
        name: "Document école.txt".into(),
        path: format!("fixture/{id}/Document école.txt"),
        modified: timestamp.timestamp_millis() as u64,
        ..Default::default()
    }]);
    tab.filter_dates.insert(group);
    tab.update_cache_on(day);
    tab.calendar_reference = Some(reference(day));
    tab
}
fn layout(day: NaiveDate) -> LayoutNode {
    let mut first = PaneState::new(tab("active", day, DateGroup::Today));
    first.tabs.push(tab("inactive", day, DateGroup::Yesterday));
    let second = PaneState::new(tab("other-pane", day, DateGroup::Yesterday));
    LayoutNode::Split {
        id: "split".into(),
        axis: LayoutAxis::Horizontal,
        weights: vec![0.5, 0.5],
        children: vec![
            LayoutNode::Pane(Box::new(first)),
            LayoutNode::Pane(Box::new(second)),
        ],
    }
}

#[test]
fn midnight_refresh_updates_active_and_inactive_tabs_without_reloading_files() {
    let day = NaiveDate::from_ymd_opt(2026, 10, 7).unwrap();
    let mut layout = layout(day);
    let next = reference(day + Duration::days(1));
    let requests = layout.calendar_filter_requests(&next);
    assert_eq!(requests.len(), 3);
    // Exercise the worker boundary: no GPUI entities or scroll handles travel to it.
    let results =
        std::thread::spawn(move || requests.into_iter().map(|r| r.calculate()).collect())
            .join()
            .unwrap();
    assert!(layout.apply_calendar_filters(results));
    layout.visit_panes(&mut |pane| {
        for tab in &pane.tabs {
            assert_eq!(tab.files.len(), 1);
            assert_eq!(
                tab.filtered_files().len(),
                if tab.id == "active" { 0 } else { 1 }
            );
            assert_eq!(tab.calendar_reference.as_ref(), Some(&next));
        }
    });
    assert!(layout.calendar_filter_requests(&next).is_empty());
}

#[test]
fn clock_jumps_and_timezone_changes_invalidate_once() {
    let day = NaiveDate::from_ymd_opt(2026, 10, 7).unwrap();
    let mut tab = tab("clock", day, DateGroup::Today);
    let mut reference = reference(day + Duration::days(4));
    for key in [
        reference.clone(),
        {
            reference.today = day;
            reference.clone()
        },
        {
            reference.utc_offset = 3600;
            reference.clone()
        },
        {
            reference.timezone = Some("another-zone-with-same-offset".into());
            reference.clone()
        },
    ] {
        let result = tab.calendar_filter_request(&key).unwrap().calculate();
        assert!(tab.apply_calendar_filter(result));
        assert!(tab.calendar_filter_request(&key).is_none());
    }
    assert_eq!(tab.filtered_files().len(), 1);
}

#[test]
fn obsolete_results_cannot_override_a_filter_edit_navigation_or_reload() {
    let day = NaiveDate::from_ymd_opt(2026, 10, 7).unwrap();
    let next = reference(day + Duration::days(1));
    for change in 0..3 {
        let mut tab = tab("revision", day, DateGroup::Today);
        let result = tab.calendar_filter_request(&next).unwrap().calculate();
        match change {
            0 => {
                tab.filter_dates.clear();
            }
            1 => {
                tab.path = "another-folder".into();
                tab.files = Default::default();
            }
            _ => {
                std::sync::Arc::make_mut(&mut tab.files)[0].name = "new result.txt".into();
            }
        }
        tab.update_cache_on(day);
        let current: Vec<_> = tab
            .filtered_files()
            .iter()
            .map(|file| file.name.clone())
            .collect();
        assert!(!tab.apply_calendar_filter(result));
        assert_eq!(
            tab.filtered_files()
                .iter()
                .map(|file| file.name.clone())
                .collect::<Vec<_>>(),
            current
        );
    }
}

#[test]
fn closed_tabs_are_ignored_and_tabs_moved_between_panes_receive_their_results() {
    let day = NaiveDate::from_ymd_opt(2026, 10, 7).unwrap();
    let mut layout = layout(day);
    let next = reference(day + Duration::days(1));
    let results = layout
        .calendar_filter_requests(&next)
        .into_iter()
        .map(|r| r.calculate())
        .collect();
    let LayoutNode::Split { children, .. } = &mut layout else {
        panic!();
    };
    let mut moved = None;
    if let LayoutNode::Pane(pane) = &mut children[0] {
        moved = Some(pane.tabs.remove(1));
    }
    if let LayoutNode::Pane(pane) = &mut children[1] {
        pane.tabs.clear(); // The old tab is closed and replaced by the moved one.
        pane.tabs.push(moved.unwrap());
    }
    assert!(layout.apply_calendar_filters(results));
    layout.visit_panes(&mut |pane| {
        for tab in &pane.tabs {
            assert_eq!(tab.calendar_reference.as_ref(), Some(&next));
            assert_eq!(
                tab.filtered_files().len(),
                if tab.id == "active" { 0 } else { 1 }
            );
        }
    });
}

#[test]
fn calendar_refresh_preserves_filters_selection_scroll_and_column_measurements() {
    let day = NaiveDate::from_ymd_opt(2026, 10, 7).unwrap();
    let mut tab = tab("preserved", day, DateGroup::Yesterday);
    tab.filter_name = "école".into();
    tab.filter_types.insert("TXT".into());
    tab.update_cache_on(day);
    tab.calendar_reference = Some(reference(day));
    let selected = tab.files[0].path.clone();
    tab.selected_paths.insert(selected.clone());
    tab.selection_anchor = Some(selected.clone());
    tab.type_col_width = 123.0;
    tab.column_type_labels.insert("measurement-marker".into());
    tab.list_scroll
        .0
        .borrow()
        .base_handle
        .set_offset(gpui::point(gpui::px(0.0), gpui::px(-100.0)));
    let offset = tab.list_scroll.0.borrow().base_handle.offset();
    let result = tab
        .calendar_filter_request(&reference(day + Duration::days(1)))
        .unwrap()
        .calculate();
    assert!(tab.apply_calendar_filter(result));
    assert_eq!(tab.filtered_files().len(), 1);
    assert_eq!(tab.filter_name, "école");
    assert!(tab.filter_types.contains("TXT"));
    assert!(tab.selected_paths.contains(&selected));
    assert_eq!(tab.selection_anchor, Some(selected));
    assert_eq!(tab.list_scroll.0.borrow().base_handle.offset(), offset);
    assert_eq!(tab.type_col_width, 123.0);
    assert!(tab.column_type_labels.contains("measurement-marker"));
}

#[test]
fn tabs_without_date_filters_never_schedule_a_cache_rebuild() {
    let day = NaiveDate::from_ymd_opt(2026, 10, 7).unwrap();
    let mut tab = tab("grouped", day, DateGroup::Today);
    tab.filter_dates.clear();
    tab.group_mode = crate::ui::date_groups::GroupMode::Date;
    tab.update_cache_on(day);
    let layout = LayoutNode::Pane(Box::new(PaneState::new(tab)));
    assert!(layout
        .calendar_filter_requests(&reference(day + Duration::days(1)))
        .is_empty());
    assert!(layout.has_date_groups());
}
