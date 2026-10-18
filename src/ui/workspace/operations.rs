use super::{LayoutAxis, LayoutNode, PaneState, TabState, WorkspaceState};
use oxyde_lib::commands::system::{get_drives, get_quick_access_items};

impl WorkspaceState {
    pub fn new() -> Self {
        let drives = get_drives(Some(true));
        let pinned_items = get_quick_access_items().unwrap_or_default();
        let initial_path = dirs::home_dir()
            .or_else(|| {
                drives
                    .first()
                    .map(|drive| std::path::PathBuf::from(&drive.path))
            })
            .unwrap_or_else(|| std::path::PathBuf::from(std::path::MAIN_SEPARATOR.to_string()));
        let default_tab = TabState::new(
            "tab-0".to_string(),
            initial_path.to_string_lossy().into_owned(),
        );

        let initial_pane = PaneState::new(default_tab);
        let pane_id = initial_pane.id.clone();

        let mut state = Self {
            pinned_items,
            onedrive_items: Vec::new(),
            drives,
            layout: LayoutNode::Pane(Box::new(initial_pane)),
            active_pane_id: pane_id,
            expanded_paths: std::collections::HashSet::new(),
            sidebar_items: Vec::new(),
            sidebar_nodes: std::collections::HashMap::new(),
            next_tab_id: 1,
        };

        state.refresh_sidebar();
        state
    }

    pub fn find_pane_mut(&mut self, id: &str) -> Option<&mut PaneState> {
        self.layout.find_pane_mut(id)
    }

    pub fn find_pane(&self, id: &str) -> Option<&PaneState> {
        self.layout.find_pane(id)
    }

    pub fn find_tab_mut(&mut self, tab_id: &str) -> Option<&mut TabState> {
        let mut pane_id = None;
        self.layout.visit_panes(&mut |pane| {
            if pane_id.is_none() && pane.tabs.iter().any(|tab| tab.id == tab_id) {
                pane_id = Some(pane.id.clone());
            }
        });
        let pane = self.find_pane_mut(&pane_id?)?;
        pane.tabs.iter_mut().find(|tab| tab.id == tab_id)
    }

    pub fn active_tab_mut(&mut self) -> Option<&mut TabState> {
        let id = self.active_pane_id.clone();
        self.find_pane_mut(&id).map(|p| p.active_tab_mut())
    }

    pub fn remove_pane_by_id(&mut self, id: &str) {
        if self.active_pane_id == id {
            // Find another pane to activate
            let mut other_id = None;
            self.layout.visit_panes(&mut |p| {
                if p.id != id {
                    other_id = Some(p.id.clone());
                }
            });
            if let Some(oid) = other_id {
                self.active_pane_id = oid;
            }
        }

        if let LayoutNode::Pane(p) = &self.layout {
            if p.id == id {
                return;
            } // Cannot remove last pane
        }

        self.layout.remove_pane(id);
        self.normalize_layout();
    }

    fn normalize_layout(&mut self) {
        // Remove empty splits and collapse single-child splits
        fn normalize(node: &mut LayoutNode) {
            if let LayoutNode::Split {
                axis: _, children, ..
            } = node
            {
                for child in children.iter_mut() {
                    normalize(child);
                }

                if children.len() == 1 {
                    *node = children.remove(0);
                } else if children.is_empty() {
                    // This should probably be handled by the parent or by creating a placeholder pane
                }
            }
        }
        normalize(&mut self.layout);
    }

    pub fn add_tab(&mut self, pane_id: &str, path: String, activate: bool) {
        let id = format!("tab-{}", self.next_tab_id);
        self.next_tab_id = self.next_tab_id.wrapping_add(1);

        let new_tab = TabState::new(id, path);

        if let Some(pane) = self.find_pane_mut(pane_id) {
            let mut new_tab = new_tab;
            new_tab.view_mode = pane.active_tab().view_mode;
            let insertion_index = (pane.active_tab_index + 1).min(pane.tabs.len());
            pane.tabs.insert(insertion_index, new_tab);

            if activate {
                pane.active_tab_index = insertion_index;
                self.active_pane_id = pane_id.to_string();
            }
        }
    }

    /// Insert a full-height column after the column containing the active pane.
    pub fn add_panel_right(&mut self) -> Option<String> {
        let current = self.find_pane(&self.active_pane_id)?.active_tab();
        let path = if cfg!(target_os = "windows") { "C:\\" } else { "/" }.to_string();
        let view_mode = current.view_mode;
        let mut tab = TabState::new(format!("tab-{}", self.next_tab_id), path);
        self.next_tab_id = self.next_tab_id.wrapping_add(1);
        tab.view_mode = view_mode;
        let pane = PaneState::new(tab);
        let id = pane.id.clone();
        let node = LayoutNode::Pane(Box::new(pane));
        if let LayoutNode::Split { axis: LayoutAxis::Horizontal, children, weights, .. } = &mut self.layout {
            let index = children.iter().position(|child| child.find_pane(&self.active_pane_id).is_some())?;
            let weight = weights[index] / 2.0;
            weights[index] = weight;
            children.insert(index + 1, node);
            weights.insert(index + 1, weight);
        } else {
            let previous = std::mem::replace(&mut self.layout, node);
            let new_node = self.layout.clone();
            self.layout = LayoutNode::Split {
                id: format!("split-{}", uuid::Uuid::new_v4()),
                axis: LayoutAxis::Horizontal,
                children: vec![previous, new_node],
                weights: vec![1.0, 1.0],
            };
        }
        self.active_pane_id = id.clone();
        Some(id)
    }

    pub fn close_tab_id(&mut self, pane_id: &str, tab_idx: usize) {
        let mut pane_count = 0;
        self.layout.visit_panes(&mut |_| pane_count += 1);
        if pane_count == 1 {
            let Some(pane) = self.find_pane(pane_id) else {
                return;
            };
            if tab_idx >= pane.tabs.len() {
                return;
            }
            if pane.tabs.len() == 1 {
                let path = pane.tabs[0].path.clone();
                let view_mode = pane.tabs[0].view_mode;
                let id = format!("tab-{}", self.next_tab_id);
                self.next_tab_id = self.next_tab_id.wrapping_add(1);
                if let Some(pane) = self.find_pane_mut(pane_id) {
                    let mut replacement = TabState::new(id, path);
                    replacement.view_mode = view_mode;
                    pane.tabs[0] = replacement;
                    pane.active_tab_index = 0;
                }
                return;
            }
        }

        let mut should_remove_pane = false;
        if let Some(pane) = self.find_pane_mut(pane_id) {
            if tab_idx >= pane.tabs.len() {
                return;
            }
            let active_id = pane.active_tab().id.clone();
            if pane.tabs.len() > 1 {
                pane.tabs.remove(tab_idx);
                pane.active_tab_index = pane
                    .tabs
                    .iter()
                    .position(|tab| tab.id == active_id)
                    .unwrap_or_else(|| tab_idx.min(pane.tabs.len() - 1));
            } else {
                should_remove_pane = true;
            }
        }

        if should_remove_pane {
            self.remove_pane_by_id(pane_id);
        }
    }

    pub fn duplicate_tab(&mut self, pane_id: &str, tab_idx: usize) -> Option<usize> {
        let duplicate = if let Some(pane) = self.find_pane(pane_id) {
            pane.tabs
                .get(tab_idx)
                .map(|tab| (tab.path.clone(), tab.view_mode, tab.group_mode))
        } else {
            None
        };

        if let Some((path, view_mode, group_mode)) = duplicate {
            self.add_tab(pane_id, path, true);
            if let Some(pane) = self.find_pane_mut(pane_id) {
                pane.active_tab_mut().view_mode = view_mode;
                pane.active_tab_mut().group_mode = group_mode;
            }
            self.find_pane(pane_id).map(|pane| pane.active_tab_index)
        } else {
            None
        }
    }

    pub fn close_other_tabs(&mut self, pane_id: &str, tab_idx: usize) {
        if let Some(pane) = self.find_pane_mut(pane_id) {
            if tab_idx < pane.tabs.len() {
                let tab = pane.tabs[tab_idx].clone();
                pane.tabs = vec![tab];
                pane.active_tab_index = 0;
            }
        }
    }

    pub fn split_tab_to_axis(
        &mut self,
        from_pane_id: &str,
        from_tab_idx: usize,
        to_pane_id: &str,
        axis: LayoutAxis,
        after: bool,
    ) {
        if self.find_pane(to_pane_id).is_none() {
            return;
        }
        let tab = if let Some(pane) = self.find_pane_mut(from_pane_id) {
            if from_tab_idx < pane.tabs.len() {
                let active_id = pane.active_tab().id.clone();
                let t = pane.tabs.remove(from_tab_idx);
                if !pane.tabs.is_empty() {
                    pane.active_tab_index = pane
                        .tabs
                        .iter()
                        .position(|tab| tab.id == active_id)
                        .unwrap_or_else(|| from_tab_idx.min(pane.tabs.len() - 1));
                }
                Some(t)
            } else {
                None
            }
        } else {
            None
        };

        if let Some(tab) = tab {
            let new_pane = PaneState::new(tab);
            let new_id = new_pane.id.clone();

            // Special case for root pane
            if let LayoutNode::Pane(p) = &self.layout {
                if p.id == to_pane_id {
                    let old_root = self.layout.clone();
                    self.layout = LayoutNode::Split {
                        id: format!("split-{}", uuid::Uuid::new_v4()),
                        axis,
                        children: if after {
                            vec![old_root, LayoutNode::Pane(Box::new(new_pane))]
                        } else {
                            vec![LayoutNode::Pane(Box::new(new_pane)), old_root]
                        },
                        weights: vec![1.0, 1.0],
                    };
                    self.active_pane_id = new_id;
                    // Check if old pane became empty
                    if self
                        .find_pane_mut(from_pane_id)
                        .map(|p| p.tabs.is_empty())
                        .unwrap_or(false)
                    {
                        self.remove_pane_by_id(from_pane_id);
                    }
                    return;
                }
            }

            if self.layout.split_pane(to_pane_id, new_pane, axis, after) {
                self.active_pane_id = new_id;
            }

            // Check if from_pane became empty
            if self
                .find_pane_mut(from_pane_id)
                .map(|p| p.tabs.is_empty())
                .unwrap_or(false)
            {
                self.remove_pane_by_id(from_pane_id);
            }
        }
    }

    // Bridge for old methods to avoid breaking everything at once? No, let's replace
    pub fn reorder_tabs_id(&mut self, pane_id: &str, from: usize, to: usize) {
        if let Some(pane) = self.find_pane_mut(pane_id) {
            let len = pane.tabs.len();
            if from < len && to <= len {
                if from == to {
                    return;
                }
                let active_tab_id = pane
                    .tabs
                    .get(pane.active_tab_index)
                    .map(|tab| tab.id.clone());
                let tab = pane.tabs.remove(from);
                let mut target_idx = to;
                if from < to {
                    target_idx = target_idx.saturating_sub(1);
                }

                pane.tabs.insert(target_idx.min(pane.tabs.len()), tab);
                pane.active_tab_index = active_tab_id
                    .and_then(|id| pane.tabs.iter().position(|tab| tab.id == id))
                    .unwrap_or_else(|| target_idx.min(pane.tabs.len().saturating_sub(1)));
            }
        }
    }

    pub fn move_tab_to_pane_id(&mut self, from_pane_id: &str, from_tab: usize, to_pane_id: &str) {
        if from_pane_id == to_pane_id {
            return;
        }
        if self.find_pane(to_pane_id).is_none() {
            return;
        }
        let tab = if let Some(pane) = self.find_pane_mut(from_pane_id) {
            if from_tab < pane.tabs.len() {
                let active_id = pane.active_tab().id.clone();
                let t = pane.tabs.remove(from_tab);
                if !pane.tabs.is_empty() {
                    pane.active_tab_index = pane
                        .tabs
                        .iter()
                        .position(|tab| tab.id == active_id)
                        .unwrap_or_else(|| from_tab.min(pane.tabs.len() - 1));
                }
                Some(t)
            } else {
                None
            }
        } else {
            None
        };

        if let Some(tab) = tab {
            if let Some(to_pane) = self.find_pane_mut(to_pane_id) {
                to_pane.tabs.push(tab);
                to_pane.active_tab_index = to_pane.tabs.len() - 1;
                self.active_pane_id = to_pane_id.to_string();
            }

            if self
                .find_pane_mut(from_pane_id)
                .map(|p| p.tabs.is_empty())
                .unwrap_or(false)
            {
                self.remove_pane_by_id(from_pane_id);
            }
        }
    }

}
