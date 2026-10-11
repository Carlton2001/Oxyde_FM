use super::{default_sidebar_width, LayoutAxis, LayoutNode, PaneState, SavedLayoutAxis, SavedLayoutNode, SavedTab, SavedWorkspace, TabState, TabStripState, WorkspaceState};

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
                        group_by_date: tab.group_by_date,
                        back_stack: tab.back_stack.clone(),
                        forward_stack: tab.forward_stack.clone(),
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

    fn into_layout(self) -> Option<LayoutNode> {
        match self {
            Self::Pane {
                id,
                tabs,
                active_tab_index,
            } => {
                let tabs: Vec<TabState> = tabs
                    .into_iter()
                    .filter(|tab| !tab.path.is_empty())
                    .map(|tab| {
                        let mut state = TabState::new(tab.id, tab.path);
                        state.view_mode = tab.view_mode;
                        state.group_by_date = tab.group_by_date;
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
                let children: Option<Vec<LayoutNode>> =
                    children.into_iter().map(Self::into_layout).collect();
                let children = children?;
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
        let layout = snapshot.layout.into_layout()?;
        let mut state = Self::new();
        state.layout = layout;

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
        state.active_pane_id = if state.find_pane(&snapshot.active_pane_id).is_some() {
            snapshot.active_pane_id
        } else {
            first_pane_id?
        };
        state.next_tab_id = next_tab_id;
        state.refresh_sidebar();

        let sidebar_width = if snapshot.sidebar_width.is_finite() {
            snapshot.sidebar_width.clamp(150.0, 600.0)
        } else {
            default_sidebar_width()
        };
        Some((state, sidebar_width))
    }

}
