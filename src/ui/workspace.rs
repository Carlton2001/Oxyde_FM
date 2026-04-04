use oxyde_lib::models::{DriveInfo, FileEntry, SidebarNode, QuickAccessItem};
use oxyde_lib::commands::system::{get_drives, get_quick_access_items};
use oxyde_lib::commands::io::list_dir;
use oxyde_lib::commands::sidebar::get_sidebar_nodes;
use gpui::{UniformListScrollHandle, ScrollHandle};
use std::cmp::Ordering;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SortColumn {
    Name,
    Type,
    Size,
    Modified,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SortDirection {
    Ascending,
    Descending,
}

#[derive(Clone, Debug)]
pub enum SidebarItem {
    Header(String),
    Pinned {
        item: QuickAccessItem,
    },
    Drive {
        drive: DriveInfo,
        is_expanded: bool,
    },
    Directory {
        node: SidebarNode,
        depth: usize,
        is_expanded: bool,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SizeCategory {
    Minuscule, // < 10 KB
    Petit,     // 10 KB - 1 MB
    Moyen,     // 1 MB - 100 MB
    Grand,     // 100 MB - 1 GB
    Huge,      // > 1 GB
}

impl SizeCategory {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Minuscule => "Minuscule (< 10 Ko)",
            Self::Petit => "Petit (10 Ko - 1 Mo)",
            Self::Moyen => "Moyen (1 Mo - 100 Mo)",
            Self::Grand => "Grand (100 Mo - 1 Go)",
            Self::Huge => "Très grand (> 1 Go)",
        }
    }

    pub fn matches(&self, size: u64) -> bool {
        let kb = 1024;
        let mb = 1024 * kb;
        let gb = 1024 * mb;
        match self {
            Self::Minuscule => size < 10 * kb,
            Self::Petit => size >= 10 * kb && size < mb,
            Self::Moyen => size >= mb && size < 100 * mb,
            Self::Grand => size >= 100 * mb && size < gb,
            Self::Huge => size >= gb,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DateCategory {
    Aujourdhui,
    Hier,
    CetteSemaine,
    LaSemaineDerniere,
    PlusAncien,
}

impl DateCategory {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Aujourdhui => "Aujourd'hui",
            Self::Hier => "Hier",
            Self::CetteSemaine => "Cette semaine",
            Self::LaSemaineDerniere => "La semaine dernière",
            Self::PlusAncien => "Plus ancien",
        }
    }

    pub fn matches(&self, modified_ms: u64) -> bool {
        let now_ms = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or(std::time::Duration::from_secs(0)).as_millis() as u64;
        let elapsed_ms = now_ms.saturating_sub(modified_ms);
        let hour_ms = 3600 * 1000;
        let day_ms = 24 * hour_ms;
        
        match self {
            Self::Aujourdhui => elapsed_ms < day_ms,
            Self::Hier => elapsed_ms >= day_ms && elapsed_ms < 2 * day_ms,
            Self::CetteSemaine => elapsed_ms >= 2 * day_ms && elapsed_ms < 7 * day_ms,
            Self::LaSemaineDerniere => elapsed_ms >= 7 * day_ms && elapsed_ms < 14 * day_ms,
            Self::PlusAncien => elapsed_ms >= 14 * day_ms,
        }
    }
}

#[derive(Clone)]
pub struct TabState {
    pub id: String,
    pub path: String,
    pub files: Vec<FileEntry>,
    pub back_stack: Vec<String>,
    pub forward_stack: Vec<String>,
    pub list_scroll: UniformListScrollHandle,
    pub list_scroll_h: ScrollHandle,
    pub sort_column: SortColumn,
    pub sort_direction: SortDirection,
    pub filter_types: std::collections::HashSet<String>,
    pub filter_name: String,
    pub filter_sizes: std::collections::HashSet<SizeCategory>,
    pub filter_dates: std::collections::HashSet<DateCategory>,
    pub selected_paths: std::collections::HashSet<String>,
    pub selection_anchor: Option<String>,
    pub cached_filtered_files: Vec<FileEntry>,
    pub type_col_width: f32,
    pub size_col_width: f32,
    pub date_col_width: f32,
}

impl TabState {
    pub fn new(id: String, path: String) -> Self {
        Self {
            id,
            path: path.clone(),
            files: Vec::new(),
            back_stack: Vec::new(),
            forward_stack: Vec::new(),
            list_scroll: UniformListScrollHandle::new(),
            list_scroll_h: ScrollHandle::new(),
            sort_column: SortColumn::Name,
            sort_direction: SortDirection::Ascending,
            filter_types: std::collections::HashSet::new(),
            filter_name: String::new(),
            filter_sizes: std::collections::HashSet::new(),
            filter_dates: std::collections::HashSet::new(),
            selected_paths: std::collections::HashSet::new(),
            selection_anchor: None,
            cached_filtered_files: Vec::new(),
            type_col_width: 100.0,
            size_col_width: 100.0,
            date_col_width: 180.0,
        }
    }

    pub fn update_cache(&mut self) {
        // 1. Check if "full" filters should be cleared to simplify logic and UI
        
        // Type filter
        if !self.filter_types.is_empty() {
            let mut all_present = std::collections::HashSet::new();
            for f in &self.files {
                if f.is_dir { all_present.insert("Dossier".to_string()); }
                else if let Some(ext) = std::path::Path::new(&f.name).extension().and_then(|e| e.to_str()) {
                    all_present.insert(ext.to_uppercase());
                }
            }
            if !all_present.is_empty() && all_present.iter().all(|t| self.filter_types.contains(t)) {
                self.filter_types.clear();
            }
        }

        // Size filter
        if !self.filter_sizes.is_empty() {
            let categories = [SizeCategory::Minuscule, SizeCategory::Petit, SizeCategory::Moyen, SizeCategory::Grand, SizeCategory::Huge];
            let mut present = std::collections::HashSet::new();
            for f in &self.files {
                for cat in &categories {
                    if cat.matches(f.size) { present.insert(*cat); break; }
                }
            }
            if !present.is_empty() && present.iter().all(|c| self.filter_sizes.contains(c)) {
                self.filter_sizes.clear();
            }
        }

        // Date filter
        if !self.filter_dates.is_empty() {
            let categories = [DateCategory::Aujourdhui, DateCategory::Hier, DateCategory::CetteSemaine, DateCategory::LaSemaineDerniere, DateCategory::PlusAncien];
            let mut present = std::collections::HashSet::new();
            for f in &self.files {
                for cat in &categories {
                    if cat.matches(f.modified) { present.insert(*cat); break; }
                }
            }
            if !present.is_empty() && present.iter().all(|c| self.filter_dates.contains(c)) {
                self.filter_dates.clear();
            }
        }

        if !self.filter_types.is_empty() || !self.filter_name.is_empty() || !self.filter_sizes.is_empty() || !self.filter_dates.is_empty() {
             self.cached_filtered_files = self.files.iter().filter(|f| {
                let matches_type = if self.filter_types.is_empty() { true } else {
                    if f.is_dir { self.filter_types.contains("Dossier") }
                    else { std::path::Path::new(&f.name).extension().and_then(|e| e.to_str()).map(|s| self.filter_types.contains(&s.to_uppercase())).unwrap_or(false) }
                };
                let matches_name = if self.filter_name.is_empty() { true } else {
                    f.name.to_lowercase().contains(&self.filter_name.to_lowercase())
                };
                let matches_size = if self.filter_sizes.is_empty() { true } else {
                    self.filter_sizes.iter().any(|cat| cat.matches(f.size))
                };
                let matches_date = if self.filter_dates.is_empty() { true } else {
                    self.filter_dates.iter().any(|cat| cat.matches(f.modified))
                };
                matches_type && matches_name && matches_size && matches_date
            }).cloned().collect();
        } else {
            self.cached_filtered_files = self.files.clone();
        }
        self.recalculate_column_widths();
    }

    pub fn recalculate_column_widths(&mut self) {
        use crate::ui::style::{format_size, format_date};
        let measure = |txt: &str| {
            txt.len() as f32 * 6.5
        };

        let mut max_type_w = measure("Type");
        let mut max_size_w = measure("Taille");
        let mut max_date_w = measure("Modifié le");

        // We use the full files list instead of cached_filtered_files 
        // to keep columns stable while the user interacts with filters.
        for f in &self.files {
            let type_str = if f.is_dir { "Dossier".to_string() } else { std::path::Path::new(&f.name).extension().and_then(|e| e.to_str()).map(|s| s.to_uppercase()).unwrap_or("Fichier".to_string()) };
            max_type_w = max_type_w.max(measure(&type_str));

            let size_str = format_size(f.size, f.is_dir, f.is_calculated);
            max_size_w = max_size_w.max(measure(&size_str));

            let date_str = format_date(f.modified);
            max_date_w = max_date_w.max(measure(&date_str));
        }

        self.type_col_width = (max_type_w + 36.0).max(70.0);
        self.size_col_width = (max_size_w + 36.0).max(70.0);
        self.date_col_width = (max_date_w + 36.0).max(130.0);
    }

    pub fn filtered_files(&self) -> &[FileEntry] {
        &self.cached_filtered_files
    }

    pub fn sort_files(&mut self) {
        let col = self.sort_column;
        let dir = self.sort_direction;

        self.files.sort_by(|a, b| {
            // Folders always first
            if a.is_dir && !b.is_dir {
                return Ordering::Less;
            }
            if !a.is_dir && b.is_dir {
                return Ordering::Greater;
            }

            let cmp = match col {
                SortColumn::Name => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
                SortColumn::Type => {
                    let ext_a = std::path::Path::new(&a.name).extension().and_then(|e| e.to_str()).unwrap_or("");
                    let ext_b = std::path::Path::new(&b.name).extension().and_then(|e| e.to_str()).unwrap_or("");
                    ext_a.to_lowercase().cmp(&ext_b.to_lowercase())
                }
                SortColumn::Size => a.size.cmp(&b.size),
                SortColumn::Modified => a.modified.cmp(&b.modified),
            };

            let cmp = if cmp == Ordering::Equal {
                a.name.to_lowercase().cmp(&b.name.to_lowercase())
            } else {
                cmp
            };

            match dir {
                SortDirection::Ascending => cmp,
                SortDirection::Descending => cmp.reverse(),
            }
        });
    }
}

#[derive(Clone)]
pub struct PaneState {
    pub id: String,
    pub tabs: Vec<TabState>,
    pub active_tab_index: usize,
}

impl PaneState {
    pub fn new(tab: TabState) -> Self {
        Self {
            id: format!("pane-{}", uuid::Uuid::new_v4()), // We need a way to ID panes in the tree
            tabs: vec![tab],
            active_tab_index: 0,
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

#[derive(Clone)]
pub enum LayoutAxis {
    Horizontal,
    Vertical,
}

#[derive(Clone)]
pub enum LayoutNode {
    Pane(Box<PaneState>),
    Split {
        id: String,
        axis: LayoutAxis,
        children: Vec<LayoutNode>,
        weights: Vec<f32>,
    },
}

impl LayoutNode {
    #[allow(dead_code)]
    pub fn is_pane(&self) -> bool {
        matches!(self, LayoutNode::Pane(_))
    }
    
    #[allow(dead_code)]
    pub fn as_pane_mut(&mut self) -> Option<&mut PaneState> {
        if let LayoutNode::Pane(p) = self {
            Some(p)
        } else {
            None
        }
    }

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

    pub fn visit_panes<F>(&self, f: &mut F) where F: FnMut(&PaneState) {
        match self {
            LayoutNode::Pane(p) => f(p),
            LayoutNode::Split { children, .. } => {
                for child in children {
                    child.visit_panes(f);
                }
            }
        }
    }

    #[allow(dead_code)]
    pub fn visit_panes_mut<F>(&mut self, f: &mut F) where F: FnMut(&mut PaneState) {
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
            LayoutNode::Split { id: s_id, axis, weights, children, .. } => {
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
            LayoutNode::Split { axis: _, children, weights, .. } => {
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

    pub fn split_pane(&mut self, target_id: &str, new_pane: PaneState, split_axis: LayoutAxis, after: bool) -> bool {
        match self {
            LayoutNode::Pane(p) if p.id == target_id => {
                // We found the target pane, but we need to replace it with a split
                // This is tricky without access to the parent.
                // We'll handle this in the parent Split or special case the root.
                false
            }
            LayoutNode::Split { axis, children, weights, .. } => {
                let mut split_idx = None;
                for (i, child) in children.iter_mut().enumerate() {
                    if let LayoutNode::Pane(p) = child {
                        if p.id == target_id {
                            split_idx = Some(i);
                            break;
                        }
                    } else {
                        if child.split_pane(target_id, new_pane.clone(), split_axis.clone(), after) {
                            return true;
                        }
                    }
                }

                if let Some(idx) = split_idx {
                    let same_axis = match (axis, &split_axis) {
                        (LayoutAxis::Horizontal, LayoutAxis::Horizontal) => true,
                        (LayoutAxis::Vertical, LayoutAxis::Vertical) => true,
                        _ => false,
                    };

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
                        children.insert(idx, LayoutNode::Split { 
                            id: format!("split-{}", uuid::Uuid::new_v4()),
                            axis: split_axis, 
                            children: nested_children,
                            weights: vec![1.0, 1.0],
                        });
                        weights.insert(idx, old_weight);
                    }
                    return true;
                }
                false
            }
            _ => false
        }
    }
}

pub struct WorkspaceState {
    pub pinned_items: Vec<QuickAccessItem>,
    pub drives: Vec<DriveInfo>,
    pub layout: LayoutNode,
    pub active_pane_id: String,
    pub expanded_paths: std::collections::HashSet<String>,
    pub sidebar_items: Vec<SidebarItem>,
}

impl WorkspaceState {
    pub fn new() -> Self {
        let drives = get_drives(Some(true));
        let pinned_items = get_quick_access_items().unwrap_or_default();
        let default_tab = TabState::new("tab-0".to_string(), "C:/".to_string());
        
        let mut initial_pane = PaneState::new(default_tab);
        let pane_id = initial_pane.id.clone();

        let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
        let active_path = initial_pane.active_tab().path.clone();
        if let Ok(r) = rt.block_on(list_dir(active_path, None, None, None, None)) {
            let tab = initial_pane.active_tab_mut();
            tab.files = r.entries;
            tab.sort_files();
            tab.update_cache();
            tab.list_scroll = UniformListScrollHandle::new();        }

        let mut state = Self {
            pinned_items,
            drives,
            layout: LayoutNode::Pane(Box::new(initial_pane)),
            active_pane_id: pane_id,
            expanded_paths: std::collections::HashSet::new(),
            sidebar_items: Vec::new(),
        };

        state.refresh_sidebar();
        state
    }


    #[allow(dead_code)]
    pub fn active_pane_id(&self) -> String {
        self.active_pane_id.clone()
    }

    pub fn find_pane_mut(&mut self, id: &str) -> Option<&mut PaneState> {
        self.layout.find_pane_mut(id)
    }

    pub fn find_pane(&self, id: &str) -> Option<&PaneState> {
        self.layout.find_pane(id)
    }

    #[allow(dead_code)]
    pub fn active_tab_mut(&mut self) -> Option<&mut TabState> {
        let id = self.active_pane_id.clone();
        self.find_pane_mut(&id).map(|p| p.active_tab_mut())
    }

    pub fn remove_pane_by_id(&mut self, id: &str) {
        if self.active_pane_id == id {
            // Find another pane to activate
            let mut other_id = None;
            self.layout.visit_panes(&mut |p| {
                if p.id != id { other_id = Some(p.id.clone()); }
            });
            if let Some(oid) = other_id {
                self.active_pane_id = oid;
            }
        }
        
        if let LayoutNode::Pane(p) = &self.layout {
            if p.id == id { return; } // Cannot remove last pane
        }
        
        self.layout.remove_pane(id);
        self.normalize_layout();
    }

    fn normalize_layout(&mut self) {
        // Remove empty splits and collapse single-child splits
        fn normalize(node: &mut LayoutNode) {
            if let LayoutNode::Split { axis: _, children, weights: _, .. } = node {
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
        // Calculate new tab ID
        let mut max_id = 0;
        self.layout.visit_panes(&mut |p| {
            for t in &p.tabs {
                if let Some(i) = t.id.split('-').last().and_then(|i| i.parse::<usize>().ok()) {
                    max_id = max_id.max(i);
                }
            }
        });
        let id = format!("tab-{}", max_id + 1);
        
        let mut new_tab = TabState::new(id, path);
        let rt = tokio::runtime::Runtime::new().expect("rt");
        if let Ok(r) = rt.block_on(list_dir(new_tab.path.clone(), None, None, None, None)) {
            new_tab.files = r.entries;
            new_tab.sort_files();
            new_tab.update_cache();
            new_tab.list_scroll = UniformListScrollHandle::new();        }

        if let Some(pane) = self.find_pane_mut(pane_id) {
            let insert_at = pane.active_tab_index + 1;
            if insert_at <= pane.tabs.len() {
                pane.tabs.insert(insert_at, new_tab);
            } else {
                pane.tabs.push(new_tab);
            }

            if activate {
                pane.active_tab_index = insert_at;
                self.active_pane_id = pane_id.to_string();
            }
        }
    }

    pub fn close_tab_id(&mut self, pane_id: &str, tab_idx: usize) {
        let mut should_remove_pane = false;
        if let Some(pane) = self.find_pane_mut(pane_id) {
            if pane.tabs.len() > 1 {
                pane.tabs.remove(tab_idx);
                if pane.active_tab_index >= pane.tabs.len() {
                    pane.active_tab_index = pane.tabs.len() - 1;
                }
            } else {
                should_remove_pane = true;
            }
        }
        
        if should_remove_pane {
            self.remove_pane_by_id(pane_id);
        }
    }

    pub fn split_tab_to_axis(&mut self, from_pane_id: &str, from_tab_idx: usize, to_pane_id: &str, axis: LayoutAxis, after: bool) {
        let tab = if let Some(pane) = self.find_pane_mut(from_pane_id) {
            if from_tab_idx < pane.tabs.len() {
                let t = pane.tabs.remove(from_tab_idx);
                if pane.active_tab_index >= pane.tabs.len() && !pane.tabs.is_empty() {
                    pane.active_tab_index = pane.tabs.len() - 1;
                }
                Some(t)
            } else { None }
        } else { None };

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
                    if self.find_pane_mut(from_pane_id).map(|p| p.tabs.is_empty()).unwrap_or(false) {
                        self.remove_pane_by_id(from_pane_id);
                    }
                    return;
                }
            }

            if self.layout.split_pane(to_pane_id, new_pane, axis, after) {
                self.active_pane_id = new_id;
            }
            
            // Check if from_pane became empty
            if self.find_pane_mut(from_pane_id).map(|p| p.tabs.is_empty()).unwrap_or(false) {
                self.remove_pane_by_id(from_pane_id);
            }
        }
    }

    // Bridge for old methods to avoid breaking everything at once? No, let's replace
    pub fn reorder_tabs_id(&mut self, pane_id: &str, from: usize, to: usize) {
        if let Some(pane) = self.find_pane_mut(pane_id) {
            let len = pane.tabs.len();
            if from < len && to <= len {
                if from == to { return; }
                let tab = pane.tabs.remove(from);
                let mut target_idx = to;
                if from < to {
                    target_idx = target_idx.saturating_sub(1);
                }
                
                pane.tabs.insert(target_idx.min(pane.tabs.len()), tab);
                pane.active_tab_index = target_idx.min(pane.tabs.len().saturating_sub(1));
            }
        }
    }

    pub fn move_tab_to_pane_id(&mut self, from_pane_id: &str, from_tab: usize, to_pane_id: &str) {
        if from_pane_id == to_pane_id { return; }
        let tab = if let Some(pane) = self.find_pane_mut(from_pane_id) {
            if from_tab < pane.tabs.len() {
                let t = pane.tabs.remove(from_tab);
                if pane.active_tab_index >= pane.tabs.len() && !pane.tabs.is_empty() {
                    pane.active_tab_index = pane.tabs.len() - 1;
                }
                Some(t)
            } else { None }
        } else { None };

        if let Some(tab) = tab {
            if let Some(to_pane) = self.find_pane_mut(to_pane_id) {
                to_pane.tabs.push(tab);
                to_pane.active_tab_index = to_pane.tabs.len() - 1;
                self.active_pane_id = to_pane_id.to_string();
            }
            
            if self.find_pane_mut(from_pane_id).map(|p| p.tabs.is_empty()).unwrap_or(false) {
                self.remove_pane_by_id(from_pane_id);
            }
        }
    }

    pub fn refresh_sidebar(&mut self) {
        let mut new_items = Vec::new();
        
        if !self.pinned_items.is_empty() {
            new_items.push(SidebarItem::Header("ACCÈS RAPIDE".to_string()));
            for item in &self.pinned_items {
                new_items.push(SidebarItem::Pinned {
                    item: item.clone(),
                });
            }
        }

        new_items.push(SidebarItem::Header("LECTEURS".to_string()));

        let rt = tokio::runtime::Runtime::new().expect("rt");
        for drive in &self.drives {
            let is_expanded = self.expanded_paths.contains(&drive.path);
            new_items.push(SidebarItem::Drive {
                drive: drive.clone(),
                is_expanded,
            });

            if is_expanded {
                self.populate_sidebar_subdir(&drive.path, 1, &mut new_items, &rt);
            }
        }
        self.sidebar_items = new_items;
    }

    fn populate_subdir(&self, path: &str, depth: usize, items: &mut Vec<SidebarItem>, rt: &tokio::runtime::Runtime) {
        if let Ok(nodes) = rt.block_on(get_sidebar_nodes(path.to_string())) {
            for node in nodes {
                let is_expanded = self.expanded_paths.contains(&node.path);
                let path_copy = node.path.clone();
                items.push(SidebarItem::Directory {
                    node,
                    depth,
                    is_expanded,
                });

                if is_expanded {
                    self.populate_subdir(&path_copy, depth + 1, items, rt);
                }
            }
        }
    }

    #[allow(dead_code)]
    fn populate_sidebar_subdir(&self, path: &str, depth: usize, items: &mut Vec<SidebarItem>, rt: &tokio::runtime::Runtime) {
       self.populate_subdir(path, depth, items, rt);
    }

    pub fn toggle_sidebar_path(&mut self, path: String) {
        if self.expanded_paths.contains(&path) {
            self.expanded_paths.remove(&path);
        } else {
            self.expanded_paths.insert(path);
        }
        self.refresh_sidebar();
    }

}
