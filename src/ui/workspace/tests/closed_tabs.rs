use super::*;

fn paths(state: &WorkspaceState, pane_id: &str) -> Vec<String> {
    state.find_pane(pane_id).unwrap().tabs.iter().map(|tab| tab.path.clone()).collect()
}

#[test]
fn a_closed_tab_comes_back_at_its_place_with_its_history_and_flash() {
    let mut state = WorkspaceState::new();
    let pane_id = state.active_pane_id.clone();
    state.find_pane_mut(&pane_id).unwrap().active_tab_mut().path = "/a".into();
    state.add_tab(&pane_id, "/b".into(), true);
    state.add_tab(&pane_id, "/c".into(), true);
    state.find_pane_mut(&pane_id).unwrap().tabs[1].back_stack = vec!["/old".into()];
    state.close_tab_id(&pane_id, 1);
    assert_eq!(paths(&state, &pane_id), ["/a", "/c"]);
    assert_eq!(state.reopen_closed_tab(), Some((pane_id.clone(), 1)));
    assert_eq!(paths(&state, &pane_id), ["/a", "/b", "/c"]);
    let pane = state.find_pane(&pane_id).unwrap();
    assert_eq!(pane.active_tab_index, 1);
    assert_eq!(pane.active_tab().back_stack, ["/old"]);
    assert!(pane.active_tab().opened_at.is_some());
    assert!(state.reopen_closed_tab().is_none());
}

#[test]
fn closing_other_tabs_reopens_them_in_their_order() {
    let mut state = WorkspaceState::new();
    let pane_id = state.active_pane_id.clone();
    state.find_pane_mut(&pane_id).unwrap().active_tab_mut().path = "/a".into();
    for path in ["/b", "/c", "/d"] { state.add_tab(&pane_id, path.into(), true); }
    state.close_other_tabs(&pane_id, 2);
    while state.reopen_closed_tab().is_some() {}
    assert_eq!(paths(&state, &pane_id), ["/a", "/b", "/c", "/d"]);
}

#[test]
fn a_tab_of_a_vanished_pane_reopens_in_the_active_pane() {
    let mut state = WorkspaceState::new();
    let first = state.active_pane_id.clone();
    let second = state.add_panel_right().unwrap();
    state.find_pane_mut(&second).unwrap().active_tab_mut().path = "/gone".into();
    state.close_tab_id(&second, 0);
    assert!(state.find_pane(&second).is_none());
    state.active_pane_id = first.clone();
    assert_eq!(state.reopen_closed_tab().map(|(pane, _)| pane), Some(first.clone()));
    assert_eq!(state.find_pane(&first).unwrap().active_tab().path, "/gone");
}

#[test]
fn the_history_keeps_the_most_recent_tabs() {
    let mut state = WorkspaceState::new();
    let pane_id = state.active_pane_id.clone();
    for index in 0..CLOSED_TAB_HISTORY + 5 {
        state.add_tab(&pane_id, format!("/{index}"), true);
        let active = state.find_pane(&pane_id).unwrap().active_tab_index;
        state.close_tab_id(&pane_id, active);
    }
    let mut count = 0;
    while state.reopen_closed_tab().is_some() { count += 1; }
    assert_eq!(count, CLOSED_TAB_HISTORY);
}

#[test]
fn closed_tabs_survive_a_saved_workspace() {
    let mut state = WorkspaceState::new();
    let pane_id = state.active_pane_id.clone();
    state.find_pane_mut(&pane_id).unwrap().active_tab_mut().path = "/a".into();
    state.add_tab(&pane_id, "/b".into(), true);
    state.find_pane_mut(&pane_id).unwrap().tabs[1].back_stack = vec!["/old".into()];
    state.close_tab_id(&pane_id, 1);
    let json = serde_json::to_value(state.saved_snapshot(260.0)).unwrap();
    let (mut restored, _) = WorkspaceState::from_saved_snapshot(serde_json::from_value(json).unwrap()).unwrap();
    assert_eq!(restored.reopen_closed_tab(), Some((pane_id.clone(), 1)));
    assert_eq!(paths(&restored, &pane_id), ["/a", "/b"]);
    assert_eq!(restored.find_pane(&pane_id).unwrap().active_tab().back_stack, ["/old"]);
}

#[test]
fn a_workspace_saved_without_closed_tabs_still_loads() {
    let state = WorkspaceState::new();
    let mut json = serde_json::to_value(state.saved_snapshot(260.0)).unwrap();
    json.as_object_mut().unwrap().remove("closed_tabs");
    let (restored, _) = WorkspaceState::from_saved_snapshot(serde_json::from_value(json).unwrap()).unwrap();
    assert!(restored.closed_tabs.is_empty());
}
