use super::*;

#[test]
fn show_only_replaces_the_layout_and_keeps_the_given_order() {
    let pane = PaneState::new(TabState::new("tab-0".into(), "/saved".into()));
    let saved = pane.id.clone();
    let mut state = WorkspaceState {
        active_pane_id: saved.clone(),
        layout: LayoutNode::Pane(Box::new(pane)),
        pinned_items: Vec::new(),
        onedrive_items: Vec::new(),
        drives: Vec::new(), unmounted_volumes: Vec::new(),
        expanded_paths: Default::default(),
        sidebar_items: Vec::new(),
        sidebar_nodes: Default::default(),
        next_tab_id: 1,
    };
    state.show_only(&["/a".into(), "/b".into(), "smb://server/share".into()]);
    let LayoutNode::Pane(pane) = &state.layout else { panic!("one pane expected") };
    assert_ne!(pane.id, saved);
    assert_eq!(state.active_pane_id, pane.id);
    assert_eq!(pane.active_tab_index, 0);
    let paths: Vec<_> = pane.tabs.iter().map(|tab| tab.path.as_str()).collect();
    assert_eq!(paths, ["/a", "/b", "smb://server/share"]);
    let ids: std::collections::HashSet<_> = pane.tabs.iter().map(|tab| tab.id.as_str()).collect();
    assert_eq!(ids.len(), 3);
}

#[test]
fn a_new_tab_takes_the_sort_of_the_tab_it_opens_from() {
    use crate::ui::workspace::{SortColumn, SortDirection};
    let open_from = |path: &str, column: SortColumn| {
        let mut state = WorkspaceState::new();
        let pane_id = state.active_pane_id.clone();
        let pane = state.find_pane_mut(&pane_id).unwrap();
        pane.active_tab_mut().path = "trash://".into();
        pane.active_tab_mut().sort_column = column;
        pane.active_tab_mut().sort_direction = SortDirection::Descending;
        state.add_tab(&pane_id, path.into(), true);
        let tab = state.find_pane(&pane_id).unwrap().active_tab();
        (tab.sort_column, tab.sort_direction)
    };
    assert_eq!(open_from("/home", SortColumn::Modified), (SortColumn::Modified, SortDirection::Descending));
    assert_eq!(open_from("trash://", SortColumn::DeletedAt), (SortColumn::DeletedAt, SortDirection::Descending));
    assert_eq!(open_from("/home", SortColumn::DeletedAt), (SortColumn::Name, SortDirection::Ascending));
    assert_eq!(open_from("/home", SortColumn::OriginalLocation), (SortColumn::Name, SortDirection::Ascending));
    assert_eq!(open_from("/home", SortColumn::Location), (SortColumn::Name, SortDirection::Ascending));
}
