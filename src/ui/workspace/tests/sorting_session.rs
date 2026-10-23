use super::*;
use crate::ui::workspace::{SortColumn, SortDirection};

#[test]
fn sort_preferences_round_trip_independently_for_every_tab_in_split_panes() {
    let mut state = WorkspaceState::new();
    let first = state.active_pane_id.clone();
    state.add_tab(&first, "C:/other".into(), true);
    let second = state.add_panel_right().unwrap();
    let cases = [
        (&first, 0, SortColumn::Modified, SortDirection::Descending),
        (&first, 1, SortColumn::Size, SortDirection::Ascending),
        (&second, 0, SortColumn::Type, SortDirection::Descending),
    ];
    for (pane, index, column, direction) in cases {
        let tab = &mut state.find_pane_mut(pane).unwrap().tabs[index];
        tab.sort_column = column;
        tab.sort_direction = direction;
    }
    let json = serde_json::to_string(&state.saved_snapshot(260.0)).unwrap();
    let saved = serde_json::from_str(&json).unwrap();
    let (restored, _) = WorkspaceState::from_saved_snapshot(saved).unwrap();
    for (pane, index, column, direction) in cases {
        let tab = &restored.find_pane(pane).unwrap().tabs[index];
        assert_eq!(tab.sort_column, column);
        assert_eq!(tab.sort_direction, direction);
    }
}

#[test]
fn legacy_sessions_keep_name_ascending_sorting() {
    let saved: SavedTab = serde_json::from_str(r#"{"id":"tab-1","path":"C:/"}"#).unwrap();
    assert_eq!(saved.sort_column, SortColumn::Name);
    assert_eq!(saved.sort_direction, SortDirection::Ascending);
}
