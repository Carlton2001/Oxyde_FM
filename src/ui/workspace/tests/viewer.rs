use super::*;
use crate::ui::workspace::{LayoutAxis, LayoutNode, PaneState};

fn workspace() -> WorkspaceState {
    let pane = PaneState::new(TabState::new("tab-0".into(), "C:/".into()));
    WorkspaceState {
        active_pane_id: pane.id.clone(),
        layout: LayoutNode::Pane(Box::new(pane)),
        pinned_items: Vec::new(),
        onedrive_items: Vec::new(),
        drives: Vec::new(), unmounted_volumes: Vec::new(),
        expanded_paths: Default::default(),
        sidebar_items: Vec::new(),
        sidebar_nodes: Default::default(),
        next_tab_id: 1,
    }
}

#[test]
fn viewer_cannot_join_explorer_and_explorer_cannot_join_viewer() {
    let mut state = workspace();
    let explorer = state.active_pane_id.clone();
    let viewer = state.add_viewer().unwrap();
    assert_eq!(state.active_pane_id, explorer);
    let pane = state.viewer_pane_id(&viewer).unwrap();
    assert!(!state.can_merge_tab(&pane, 0, &explorer));
    assert!(!state.can_merge_tab(&explorer, 0, &pane));
    state.move_tab_to_pane_id(&pane, 0, &explorer);
    state.move_tab_to_pane_id(&explorer, 0, &pane);
    assert_eq!(state.find_pane(&explorer).unwrap().tabs.len(), 1);
    assert_eq!(state.find_pane(&pane).unwrap().tabs[0].id, viewer);
    assert_eq!(state.find_pane(&pane).unwrap().tabs.len(), 1);
}

#[test]
fn adding_or_duplicating_never_puts_another_tab_in_viewer_pane() {
    let mut state = workspace();
    let viewer = state.add_viewer().unwrap();
    let viewer_pane = state.viewer_pane_id(&viewer).unwrap();
    state.add_tab(&viewer_pane, "D:/".into(), true);
    assert_eq!(state.find_pane(&viewer_pane).unwrap().tabs.len(), 1);
    state.duplicate_tab(&viewer_pane, 0);
    assert!(state.add_viewer().is_none());
    assert_ne!(state.active_pane_id, viewer_pane);
    let mut viewers = 0;
    state.layout.visit_panes(&mut |pane| {
        if pane.active_tab().is_viewer() {
            viewers += 1;
            assert_eq!(pane.tabs.len(), 1);
        }
    });
    assert_eq!(viewers, 1);
}

#[test]
fn viewer_moves_as_its_own_split_and_keeps_its_tab_identity() {
    let mut state = workspace();
    let explorer = state.active_pane_id.clone();
    let viewer = state.add_viewer().unwrap();
    let old_pane = state.viewer_pane_id(&viewer).unwrap();
    state.split_tab_to_axis(&old_pane, 0, &explorer, LayoutAxis::Vertical, false);
    assert!(state.find_pane(&old_pane).is_none());
    assert_eq!(state.active_pane_id, explorer);
    let pane = state.find_pane(&state.viewer_pane_id(&viewer).unwrap()).unwrap();
    assert_eq!(pane.tabs.len(), 1);
    assert_eq!(pane.tabs[0].id, viewer);
    let own_pane = pane.id.clone();
    state.split_tab_to_axis(&own_pane, 0, &own_pane, LayoutAxis::Horizontal, true);
    assert_eq!(state.find_pane(&own_pane).unwrap().tabs.len(), 1);
}

#[test]
fn viewer_survives_session_restore_without_becoming_a_directory_load() {
    let mut state = workspace();
    let viewer = state.add_viewer().unwrap();
    let saved = state.saved_snapshot(260.0);
    let (restored, _) = WorkspaceState::from_saved_snapshot(saved).unwrap();
    assert!(!restored.is_viewer_pane(&restored.active_pane_id));
    let tab = restored
        .find_pane(&restored.viewer_pane_id(&viewer).unwrap())
        .unwrap()
        .active_tab();
    assert_eq!(tab.id, viewer);
    assert!(tab.is_viewer());
    assert!(matches!(tab.load_status, TabLoadStatus::Ready));
}
