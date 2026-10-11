use super::{LayoutAxis, LayoutNode, PaneState, TabState, TabStripState};
use gpui::ScrollHandle;

impl Default for TabStripState {
    fn default() -> Self {
        Self {
            scroll: ScrollHandle::new(),
            overflow: Default::default(),
            active_id: Default::default(),
            active_index: Default::default(),
        }
    }
}

impl PaneState {
    pub fn new(tab: TabState) -> Self {
        Self {
            id: format!("pane-{}", uuid::Uuid::new_v4()), // We need a way to ID panes in the tree
            tabs: vec![tab],
            active_tab_index: 0,
            tab_strip: TabStripState::default(),
        }
    }

    pub fn active_tab(&self) -> &TabState {
        let idx = self.active_tab_index.min(self.tabs.len().saturating_sub(1));
        &self.tabs[idx]
    }

    pub fn active_tab_mut(&mut self) -> &mut TabState {
        let idx = self.active_tab_index.min(self.tabs.len().saturating_sub(1));
        &mut self.tabs[idx]
    }
}

impl LayoutNode {
    pub fn find_pane_mut(&mut self, id: &str) -> Option<&mut PaneState> {
        match self {
            LayoutNode::Pane(p) if p.id == id => Some(p),
            LayoutNode::Split { children, .. } => {
                for child in children {
                    if let Some(p) = child.find_pane_mut(id) {
                        return Some(p);
                    }
                }
                None
            }
            _ => None,
        }
    }

    pub fn find_pane(&self, id: &str) -> Option<&PaneState> {
        match self {
            LayoutNode::Pane(p) if p.id == id => Some(p),
            LayoutNode::Split { children, .. } => {
                for child in children {
                    if let Some(p) = child.find_pane(id) {
                        return Some(p);
                    }
                }
                None
            }
            _ => None,
        }
    }

    pub fn visit_panes<F>(&self, f: &mut F)
    where
        F: FnMut(&PaneState),
    {
        match self {
            LayoutNode::Pane(p) => f(p),
            LayoutNode::Split { children, .. } => {
                for child in children {
                    child.visit_panes(f);
                }
            }
        }
    }

    pub fn visit_panes_mut<F>(&mut self, f: &mut F)
    where
        F: FnMut(&mut PaneState),
    {
        match self {
            LayoutNode::Pane(p) => f(p),
            LayoutNode::Split { children, .. } => {
                for child in children {
                    child.visit_panes_mut(f);
                }
            }
        }
    }

    pub fn find_split_mut(&mut self, id: &str) -> Option<(&LayoutAxis, &mut Vec<f32>)> {
        match self {
            LayoutNode::Pane(_) => None,
            LayoutNode::Split {
                id: s_id,
                axis,
                weights,
                children,
                ..
            } => {
                if s_id == id {
                    return Some((axis, weights));
                }
                for child in children {
                    if let Some(res) = child.find_split_mut(id) {
                        return Some(res);
                    }
                }
                None
            }
        }
    }

    // Returns true if the pane was found and removed from this subtree
    pub fn remove_pane(&mut self, id: &str) -> Option<PaneState> {
        match self {
            LayoutNode::Pane(_) => None, // Handled by parent
            LayoutNode::Split {
                axis: _,
                children,
                weights,
                ..
            } => {
                let mut found_idx = None;
                for (i, child) in children.iter_mut().enumerate() {
                    if let LayoutNode::Pane(p) = child {
                        if p.id == id {
                            found_idx = Some(i);
                            break;
                        }
                    } else {
                        if let Some(p) = child.remove_pane(id) {
                            return Some(p);
                        }
                    }
                }

                if let Some(idx) = found_idx {
                    weights.remove(idx);
                    if let LayoutNode::Pane(p) = children.remove(idx) {
                        return Some(*p);
                    }
                }
                None
            }
        }
    }

    pub fn split_pane(
        &mut self,
        target_id: &str,
        new_pane: PaneState,
        split_axis: LayoutAxis,
        after: bool,
    ) -> bool {
        match self {
            LayoutNode::Pane(p) if p.id == target_id => {
                // We found the target pane, but we need to replace it with a split
                // This is tricky without access to the parent.
                // We'll handle this in the parent Split or special case the root.
                false
            }
            LayoutNode::Split {
                axis,
                children,
                weights,
                ..
            } => {
                let mut split_idx = None;
                for (i, child) in children.iter_mut().enumerate() {
                    if let LayoutNode::Pane(p) = child {
                        if p.id == target_id {
                            split_idx = Some(i);
                            break;
                        }
                    } else {
                        if child.split_pane(target_id, new_pane.clone(), split_axis.clone(), after)
                        {
                            return true;
                        }
                    }
                }

                if let Some(idx) = split_idx {
                    let same_axis = matches!(
                        (axis, &split_axis),
                        (LayoutAxis::Horizontal, LayoutAxis::Horizontal)
                            | (LayoutAxis::Vertical, LayoutAxis::Vertical)
                    );

                    if same_axis {
                        let insert_at = if after { idx + 1 } else { idx };
                        children.insert(insert_at, LayoutNode::Pane(Box::new(new_pane)));
                        weights.insert(insert_at, 1.0);
                    } else {
                        let old_child = children.remove(idx);
                        let old_weight = weights.remove(idx); // Use existing weight for new nested split?
                        let nested_children = if after {
                            vec![old_child, LayoutNode::Pane(Box::new(new_pane))]
                        } else {
                            vec![LayoutNode::Pane(Box::new(new_pane)), old_child]
                        };
                        children.insert(
                            idx,
                            LayoutNode::Split {
                                id: format!("split-{}", uuid::Uuid::new_v4()),
                                axis: split_axis,
                                children: nested_children,
                                weights: vec![1.0, 1.0],
                            },
                        );
                        weights.insert(idx, old_weight);
                    }
                    return true;
                }
                false
            }
            _ => false,
        }
    }
}
