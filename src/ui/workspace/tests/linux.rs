use super::*;
use crate::ui::workspace::{LayoutNode, PaneState, TabState};
use oxyde_lib::models::SidebarNode;
use oxyde_lib::platform::WSL_ROOT;

fn workspace() -> WorkspaceState {
    let pane = PaneState::new(TabState::new("tab".into(), "C:\\".into()));
    WorkspaceState {
        active_pane_id: pane.id.clone(), layout: LayoutNode::Pane(Box::new(pane)),
        pinned_items: Vec::new(), onedrive_items: Vec::new(), drives: Vec::new(), unmounted_volumes: Vec::new(),
        expanded_paths: Default::default(), sidebar_items: Vec::new(),
        sidebar_nodes: Default::default(), next_tab_id: 1,
    }
}

fn distribution(name: &str) -> SidebarNode {
    SidebarNode {
        name: name.into(), path: format!(r"{WSL_ROOT}\{name}"),
        is_hidden: false, is_system: false, has_subdirs: true,
    }
}

#[test]
fn no_row_without_a_distribution() {
    let mut state = workspace();
    state.refresh_sidebar();
    assert!(!state.sidebar_items.iter().any(|item| matches!(item, SidebarItem::Linux { .. })));
    state.sidebar_nodes.insert(WSL_ROOT.into(), Vec::new());
    state.refresh_sidebar();
    assert!(!state.sidebar_items.iter().any(|item| matches!(item, SidebarItem::Linux { .. })));
}

#[test]
fn distributions_appear_under_the_expanded_row_before_the_network() {
    let mut state = workspace();
    state.sidebar_nodes.insert(WSL_ROOT.into(), vec![distribution("FedoraLinux-44"), distribution("Ubuntu")]);
    state.refresh_sidebar();
    let row = state.sidebar_items.iter().position(|item| matches!(item, SidebarItem::Linux { is_expanded: false })).unwrap();
    assert!(matches!(state.sidebar_items[row - 1], SidebarItem::Header(Message::SidebarLocations)));
    assert!(matches!(&state.sidebar_items[row + 1], SidebarItem::Pinned { item } if item.path == "__network_vincinity__"));

    state.toggle_sidebar_path(WSL_ROOT.into());
    assert!(matches!(state.sidebar_items[row], SidebarItem::Linux { is_expanded: true }));
    assert!(matches!(&state.sidebar_items[row + 1], SidebarItem::Directory { node, depth: 1, .. } if node.name == "FedoraLinux-44"));
    assert!(matches!(&state.sidebar_items[row + 2], SidebarItem::Directory { node, depth: 1, .. } if node.name == "Ubuntu"));
    assert!(matches!(&state.sidebar_items[row + 3], SidebarItem::Pinned { item } if item.path == "__network_vincinity__"));
}
