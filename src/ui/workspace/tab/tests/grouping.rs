use super::*;
use crate::ui::date_groups::{FileFamily, GroupKey, GroupMode};
use crate::ui::workspace::WorkspaceState;
use std::sync::Arc;

#[test]
fn type_grouping_survives_session_and_duplicate() {
    let mut state = WorkspaceState::new();
    let pane_id = state.active_pane_id.clone();
    state
        .find_pane_mut(&pane_id)
        .unwrap()
        .active_tab_mut()
        .group_mode = crate::ui::date_groups::GroupMode::Type;
    let index = state.duplicate_tab(&pane_id, 0).unwrap();
    assert_eq!(
        state.find_pane(&pane_id).unwrap().tabs[index].grouping(),
        GroupMode::Type
    );
    let snapshot =
        serde_json::from_value(serde_json::to_value(state.saved_snapshot(260.0)).unwrap()).unwrap();
    let (restored, _) = WorkspaceState::from_saved_snapshot(snapshot).unwrap();
    assert!(restored
        .find_pane(&pane_id)
        .unwrap()
        .tabs
        .iter()
        .all(|tab| tab.grouping() == GroupMode::Type));
    state.find_pane_mut(&pane_id).unwrap().tabs[index].group_mode =
        crate::ui::date_groups::GroupMode::None;
    assert_eq!(
        state.find_pane(&pane_id).unwrap().tabs[0].grouping(),
        GroupMode::Type
    );
}

#[test]
fn type_mode_invalidates_rows_and_reveal_expands_the_right_family() {
    let mut tab = TabState::new("tab".into(), "C:/".into());
    tab.files = Arc::new(vec![
        FileEntry {
            name: "photo.png".into(),
            path: "C:/photo.png".into(),
            ..Default::default()
        },
        FileEntry {
            name: "backup.zip".into(),
            path: "C:/backup.zip".into(),
            ..Default::default()
        },
    ]);
    tab.update_cache();
    let ungrouped = tab.presentation(false, 1);
    tab.group_mode = crate::ui::date_groups::GroupMode::Type;
    let grouped = tab.presentation(false, 1);
    assert!(!Arc::ptr_eq(&ungrouped.rows, &grouped.rows));
    assert!(Arc::ptr_eq(&grouped, &tab.presentation(false, 1)));
    let archives = GroupKey::Family(FileFamily::Archives);
    tab.collapsed_groups.insert(archives.clone());
    assert!(tab.presentation(false, 1).rows.len() < grouped.rows.len());
    tab.pending_reveal = Some("C:/backup.zip".into());
    tab.reveal_pending_item(800.0);
    assert!(!tab.collapsed_groups.contains(&archives));
}

#[test]
fn modes_follow_regular_archive_search_trash_and_network_panes() {
    let mut tab = TabState::new("tab".into(), "C:/".into());
    for mode in [GroupMode::Size, GroupMode::Extension, GroupMode::Created] {
        assert!(tab.supports_grouping(mode));
    }
    assert!(!tab.supports_grouping(GroupMode::Origin));
    assert!(!tab.supports_grouping(GroupMode::Deleted));
    tab.archive_root = Some("C:/backup.zip".into());
    assert!(!tab.supports_grouping(GroupMode::Created));
    assert!(tab.supports_grouping(GroupMode::Extension));
    tab.archive_root = None;
    tab.quick_search = Some(crate::ui::app::search::QuickSearchState {
        root: "C:/".into(),
        query: "file".into(),
        running: false,
        limited: false,
        archive_warnings: Default::default(),
        cancellation: Arc::new(crate::ui::app::search::SearchCancellation(Arc::new(
            std::sync::atomic::AtomicBool::new(false),
        ))),
    });
    assert!(tab.supports_grouping(GroupMode::Origin));
    tab.quick_search = None;
    tab.path = "trash://".into();
    assert!(tab.supports_grouping(GroupMode::Deleted));
    assert!(tab.supports_grouping(GroupMode::Origin));
    assert!(!tab.supports_grouping(GroupMode::Created));
    tab.group_mode = GroupMode::Deleted;
    assert_eq!(tab.grouping(), GroupMode::Deleted);
    tab.path = "C:/".into();
    assert_eq!(tab.grouping(), GroupMode::None);
    tab.path = "__network_vincinity__".into();
    assert_eq!(tab.available_groupings(), [GroupMode::None]);
}

#[test]
fn new_modes_survive_sessions_and_duplicates() {
    for mode in [
        GroupMode::Size,
        GroupMode::Extension,
        GroupMode::Created,
        GroupMode::Origin,
        GroupMode::Deleted,
    ] {
        let mut state = WorkspaceState::new();
        let pane_id = state.active_pane_id.clone();
        let tab = state.find_pane_mut(&pane_id).unwrap().active_tab_mut();
        tab.path = if matches!(mode, GroupMode::Deleted | GroupMode::Origin) {
            "trash://"
        } else {
            "C:/"
        }
        .into();
        tab.group_mode = mode;
        let index = state.duplicate_tab(&pane_id, 0).unwrap();
        assert_eq!(
            state.find_pane(&pane_id).unwrap().tabs[index].grouping(),
            mode
        );
        let snapshot =
            serde_json::from_value(serde_json::to_value(state.saved_snapshot(260.0)).unwrap())
                .unwrap();
        let (restored, _) = WorkspaceState::from_saved_snapshot(snapshot).unwrap();
        assert_eq!(
            restored.find_pane(&pane_id).unwrap().tabs[0].grouping(),
            mode
        );
    }
}

#[test]
fn manual_folder_size_changes_its_section_and_invalidates_presentation() {
    let mut tab = TabState::new("tab".into(), "C:/".into());
    tab.group_mode = GroupMode::Size;
    tab.files = Arc::new(vec![FileEntry {
        name: "folder".into(),
        path: "C:/folder".into(),
        is_dir: true,
        ..Default::default()
    }]);
    tab.update_cache();
    let before = tab.presentation(false, 1);
    assert!(matches!(
        &before.rows[0],
        crate::ui::date_groups::DisplayRow::Header(GroupKey::Family(FileFamily::Folders), 1)
    ));
    let folder = &mut Arc::make_mut(&mut tab.files)[0];
    folder.size = 2000000;
    folder.is_calculated = true;
    tab.update_cache();
    let after = tab.presentation(false, 1);
    assert!(!Arc::ptr_eq(&before.rows, &after.rows));
    assert!(matches!(
        &after.rows[0],
        crate::ui::date_groups::DisplayRow::Header(GroupKey::Size(_), 1)
    ));
}
