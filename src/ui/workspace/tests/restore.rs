use super::*;

fn tab(id: &str, path: &str) -> SavedTab {
    serde_json::from_value(serde_json::json!({ "id": id, "path": path })).unwrap()
}

fn pane(id: &str, tabs: Vec<SavedTab>) -> SavedLayoutNode {
    SavedLayoutNode::Pane { id: id.into(), tabs, active_tab_index: 0 }
}

fn saved(layout: SavedLayoutNode) -> SavedWorkspace {
    let mut snapshot = WorkspaceState::new().saved_snapshot(260.0);
    snapshot.layout = layout;
    snapshot.active_pane_id = "p1".into();
    snapshot
}

#[test]
fn an_empty_pane_drops_out_alone_and_the_viewer_stays_single_and_alone() {
    let layout = SavedLayoutNode::Split {
        id: "s".into(),
        axis: SavedLayoutAxis::Horizontal,
        weights: vec![1.0; 4],
        children: vec![
            pane("p1", vec![tab("tab-1", "C:/a"), tab("tab-1", "C:/b"), tab("tab-2", "viewer://")]),
            pane("p2", Vec::new()),
            pane("p3", vec![tab("tab-3", "viewer://")]),
            pane("p4", vec![tab("tab-4", "viewer://")]),
        ],
    };
    let (state, _) = WorkspaceState::from_saved_snapshot(saved(layout)).unwrap();
    let mut panes = Vec::new();
    state.layout.visit_panes(&mut |pane| {
        panes.push((pane.id.clone(), pane.tabs.iter().map(|tab| (tab.id.clone(), tab.path.clone())).collect::<Vec<_>>()));
    });
    assert_eq!(panes.len(), 2);
    assert_eq!(panes[0].0, "p1");
    // Two tabs once named alike now have names of their own; the shared Viewer is gone.
    assert_eq!(panes[0].1.len(), 2);
    assert_ne!(panes[0].1[0].0, panes[0].1[1].0);
    assert_eq!(panes[1], ("p3".to_string(), vec![("tab-3".to_string(), "viewer://".to_string())]));
}

#[test]
fn a_layout_without_an_explorer_is_not_restored_and_history_is_bounded() {
    assert!(WorkspaceState::from_saved_snapshot(saved(pane("p1", vec![tab("tab-1", "viewer://")]))).is_none());

    let mut state = WorkspaceState::new();
    let id = state.active_pane_id.clone();
    state.find_pane_mut(&id).unwrap().tabs[0].back_stack = (0..200).map(|index| format!("C:/{index}")).collect();
    let snapshot = state.saved_snapshot(260.0);
    let SavedLayoutNode::Pane { tabs, .. } = &snapshot.layout else { panic!("one pane") };
    assert_eq!(tabs[0].back_stack.len(), 50);
    assert_eq!(tabs[0].back_stack.last().map(String::as_str), Some("C:/199"));
}
