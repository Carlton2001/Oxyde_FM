use super::*;
use crate::ui::workspace::{LayoutNode, PaneState, TabState};
fn workspace() -> WorkspaceState {
    let pane = PaneState::new(TabState::new("tab".into(), "C:\\".into()));
    WorkspaceState {
        active_pane_id: pane.id.clone(), layout: LayoutNode::Pane(Box::new(pane)),
        pinned_items: vec![QuickAccessItem { name: "Pinned".into(), path: "C:\\Pinned".into() }],
        cloud_root_items: Vec::new(), drives: Vec::new(), unmounted_volumes: Vec::new(),
        expanded_paths: Default::default(), sidebar_items: Vec::new(),
        sidebar_nodes: Default::default(), next_tab_id: 1, closed_tabs: Default::default(),
    }
}
#[test]
fn cloud_roots_follow_pins_before_drive_separator() {
    let mut state = workspace();
    state.cloud_root_items.push(QuickAccessItem { name: "Cloud Drive".into(), path: "C:\\Cloud".into() });
    state.refresh_sidebar();
    // Linux heads the pins with the home folder.
    let pinned = state.sidebar_items.iter()
        .position(|i| matches!(i, SidebarItem::Pinned { item } if item.path == "C:\\Pinned")).unwrap();
    assert!(matches!(&state.sidebar_items[pinned + 1], SidebarItem::CloudRoot { item } if item.path == "C:\\Cloud"));
    assert!(matches!(state.sidebar_items[pinned + 2], SidebarItem::Header(Message::SidebarDrives)));
    state.refresh_sidebar();
    assert_eq!(state.sidebar_items.iter().filter(|i| matches!(i, SidebarItem::CloudRoot { .. })).count(), 1);
}
#[test]
fn no_entry_without_available_roots() {
    let mut state = workspace();
    state.refresh_sidebar();
    assert!(!state.sidebar_items.iter().any(|i| matches!(i, SidebarItem::CloudRoot { .. })));
    state.cloud_root_items.push(QuickAccessItem { name: "Cloud Drive".into(), path: "C:\\Cloud".into() });
    state.refresh_sidebar();
    state.cloud_root_items.clear();
    state.refresh_sidebar();
    assert!(!state.sidebar_items.iter().any(|i| matches!(i, SidebarItem::CloudRoot { .. })));
}
