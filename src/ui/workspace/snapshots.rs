use super::{default_sidebar_width, LayoutAxis, LayoutNode, PaneState, SavedLayoutAxis, SavedLayoutNode, SavedTab, SavedWorkspace, TabState, TabStripState, WorkspaceState};

#[cfg(test)]
#[path = "tests/sorting_session.rs"]
mod sorting_session_tests;

#[cfg(test)]
#[path = "tests/restore.rs"]
mod restore_tests;

/// Places kept per direction of a tab's history: the nearest ones.
const MAX_SAVED_HISTORY: usize = 50;

fn recent(stack: &[String]) -> Vec<String> {
    stack[stack.len().saturating_sub(MAX_SAVED_HISTORY)..].to_vec()
}

/// What restoring a layout checks across its panes: one Viewer at most, alone in
/// its pane, and tab identifiers that name one tab each.
#[derive(Default)]
struct Restore {
    viewer: bool,
    ids: std::collections::HashSet<String>,
}

impl SavedLayoutNode {
    fn from_layout(layout: &LayoutNode) -> Self {
        match layout {
            LayoutNode::Pane(pane) => Self::Pane {
                id: pane.id.clone(),
                tabs: pane
                    .tabs
                    .iter()
                    .map(|tab| SavedTab {
                        id: tab.id.clone(),
                        path: tab.path.clone(),
                        view_mode: tab.view_mode,
                        grouping: tab.group_mode,
                        sort_column: tab.sort_column,
                        sort_direction: tab.sort_direction,
                        back_stack: recent(&tab.back_stack),
                        forward_stack: recent(&tab.forward_stack),
                    })
                    .collect(),
                active_tab_index: pane.active_tab_index,
            },
            LayoutNode::Split {
                id,
                axis,
                children,
                weights,
            } => Self::Split {
                id: id.clone(),
                axis: match axis {
                    LayoutAxis::Horizontal => SavedLayoutAxis::Horizontal,
                    LayoutAxis::Vertical => SavedLayoutAxis::Vertical,
                },
                children: children.iter().map(Self::from_layout).collect(),
                weights: weights.clone(),
            },
        }
    }

    /// A pane left without tabs drops out alone; the rest of the layout stays.
    fn into_layout(self, restore: &mut Restore) -> Option<LayoutNode> {
        match self {
            Self::Pane {
                id,
                tabs,
                active_tab_index,
            } => {
                let shared = tabs.len() > 1;
                let tabs: Vec<TabState> = tabs
                    .into_iter()
                    .filter(|tab| !tab.path.is_empty())
                    // The Viewer has a pane of its own, and there is one at most.
                    .filter(|tab| {
                        tab.path != super::viewer::VIEWER_PATH || (!shared && !std::mem::replace(&mut restore.viewer, true))
                    })
                    .map(|tab| {
                        let id = if restore.ids.insert(tab.id.clone()) {
                            tab.id
                        } else {
                            let id = format!("tab-{}", uuid::Uuid::new_v4());
                            restore.ids.insert(id.clone());
                            id
                        };
                        let mut state = TabState::new(id, tab.path);
                        state.view_mode = tab.view_mode;
                        state.group_mode = tab.grouping;
                        state.sort_column = tab.sort_column;
                        state.sort_direction = tab.sort_direction;
                        state.back_stack = tab.back_stack;
                        state.forward_stack = tab.forward_stack;
                        state
                    })
                    .collect();
                if tabs.is_empty() {
                    return None;
                }
                Some(LayoutNode::Pane(Box::new(PaneState {
                    id,
                    active_tab_index: active_tab_index.min(tabs.len() - 1),
                    tab_strip: TabStripState::default(),
                    tabs,
                })))
            }
            Self::Split {
                id,
                axis,
                children,
                weights,
            } => {
                let (children, weights): (Vec<LayoutNode>, Vec<f32>) = children
                    .into_iter()
                    .enumerate()
                    .filter_map(|(index, child)| {
                        let weight = weights.get(index).copied().unwrap_or(f32::NAN);
                        child.into_layout(restore).map(|child| (child, weight))
                    })
                    .unzip();
                if children.len() < 2 {
                    return children.into_iter().next();
                }
                let weights = if weights.len() == children.len()
                    && weights
                        .iter()
                        .all(|weight| weight.is_finite() && *weight > 0.0)
                {
                    weights
                } else {
                    vec![1.0; children.len()]
                };
                Some(LayoutNode::Split {
                    id,
                    axis: match axis {
                        SavedLayoutAxis::Horizontal => LayoutAxis::Horizontal,
                        SavedLayoutAxis::Vertical => LayoutAxis::Vertical,
                    },
                    children,
                    weights,
                })
            }
        }
    }
}

impl WorkspaceState {
    pub fn saved_snapshot(&self, sidebar_width: f32) -> SavedWorkspace {
        SavedWorkspace {
            sidebar_collapsed: false,
            closed_tabs: self.closed_tabs.clone(),
            layout: SavedLayoutNode::from_layout(&self.layout),
            active_pane_id: self.active_pane_id.clone(),
            sidebar_width: if sidebar_width.is_finite() {
                sidebar_width.clamp(150.0, 600.0)
            } else {
                default_sidebar_width()
            },
        }
    }

    pub fn from_saved_snapshot(snapshot: SavedWorkspace) -> Option<(Self, f32)> {
        let layout = snapshot.layout.into_layout(&mut Restore::default())?;
        let mut state = Self::new();
        state.layout = layout;

        state.layout.visit_panes_mut(&mut |pane| {
            for tab in &mut pane.tabs {
                tab.restored = true;
            }
        });
        let mut first_pane_id = None;
        let mut next_tab_id = 1u64;
        state.layout.visit_panes(&mut |pane| {
            if first_pane_id.is_none() {
                first_pane_id = Some(pane.id.clone());
            }
            for tab in &pane.tabs {
                if let Some(id) = tab
                    .id
                    .strip_prefix("tab-")
                    .and_then(|id| id.parse::<u64>().ok())
                {
                    next_tab_id = next_tab_id.max(id.saturating_add(1));
                }
            }
        });
        // A layout holding only the Viewer has nothing to browse: start afresh.
        let mut explorer = false;
        state.layout.visit_panes(&mut |pane| explorer |= !pane.active_tab().is_viewer());
        if !explorer {
            return None;
        }
        state.active_pane_id = if state.find_pane(&snapshot.active_pane_id).is_some() {
            snapshot.active_pane_id
        } else {
            first_pane_id?
        };
        let active = state.active_pane_id.clone();
        state.keep_explorer_active(&active);
        state.next_tab_id = next_tab_id;
        state.closed_tabs = snapshot.closed_tabs.restored();
        state.refresh_sidebar();

        let sidebar_width = if snapshot.sidebar_width.is_finite() {
            snapshot.sidebar_width.clamp(150.0, 600.0)
        } else {
            default_sidebar_width()
        };
        Some((state, sidebar_width))
    }

}
