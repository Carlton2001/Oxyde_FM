use crate::ui::i18n::Message;
use gpui::{ScrollHandle, UniformListScrollHandle};
use oxyde_lib::commands::system::{get_drives, get_quick_access_items};
use oxyde_lib::models::{DriveInfo, FileEntry, QuickAccessItem, SidebarNode};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;

pub const WORKSPACE_CONFIG_KEY: &str = "workspace_state";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SavedWorkspace {
    pub layout: SavedLayoutNode,
    pub active_pane_id: String,
    #[serde(default = "default_sidebar_width")]
    pub sidebar_width: f32,
}

fn default_sidebar_width() -> f32 {
    260.0
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SavedLayoutNode {
    Pane {
        id: String,
        tabs: Vec<SavedTab>,
        active_tab_index: usize,
    },
    Split {
        id: String,
        axis: SavedLayoutAxis,
        children: Vec<SavedLayoutNode>,
        weights: Vec<f32>,
    },
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SavedLayoutAxis {
    Horizontal,
    Vertical,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SavedTab {
    pub id: String,
    pub path: String,
    #[serde(default)]
    pub back_stack: Vec<String>,
    #[serde(default)]
    pub forward_stack: Vec<String>,
}

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
    Header(Message),
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
    pub fn message(&self) -> Message {
        match self {
            Self::Minuscule => Message::SizeTiny,
            Self::Petit => Message::SizeSmall,
            Self::Moyen => Message::SizeMedium,
            Self::Grand => Message::SizeLarge,
            Self::Huge => Message::SizeHuge,
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
    pub fn message(&self) -> Message {
        match self {
            Self::Aujourdhui => Message::DateToday,
            Self::Hier => Message::DateYesterday,
            Self::CetteSemaine => Message::DateThisWeek,
            Self::LaSemaineDerniere => Message::DateLastWeek,
            Self::PlusAncien => Message::DateOlder,
        }
    }

    pub fn matches(&self, modified_ms: u64) -> bool {
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or(std::time::Duration::from_secs(0))
            .as_millis() as u64;
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
    pub load_status: TabLoadStatus,
}

#[derive(Clone)]
pub enum TabLoadStatus {
    Loading,
    Ready,
    Error(Message),
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
            load_status: TabLoadStatus::Loading,
        }
    }

    pub fn update_cache(&mut self) {
        if !self.filter_types.is_empty()
            || !self.filter_name.is_empty()
            || !self.filter_sizes.is_empty()
            || !self.filter_dates.is_empty()
        {
            self.cached_filtered_files = self
                .files
                .iter()
                .filter(|f| {
                    let matches_type = if self.filter_types.is_empty() {
                        true
                    } else {
                        if f.is_dir {
                            self.filter_types.contains("Dossier")
                        } else {
                            std::path::Path::new(&f.name)
                                .extension()
                                .and_then(|e| e.to_str())
                                .map(|s| self.filter_types.contains(&s.to_uppercase()))
                                .unwrap_or(false)
                        }
                    };
                    let matches_name = if self.filter_name.is_empty() {
                        true
                    } else {
                        f.name
                            .to_lowercase()
                            .contains(&self.filter_name.to_lowercase())
                    };
                    let matches_size = if self.filter_sizes.is_empty() {
                        true
                    } else {
                        self.filter_sizes.iter().any(|cat| cat.matches(f.size))
                    };
                    let matches_date = if self.filter_dates.is_empty() {
                        true
                    } else {
                        self.filter_dates.iter().any(|cat| cat.matches(f.modified))
                    };
                    matches_type && matches_name && matches_size && matches_date
                })
                .cloned()
                .collect();
        } else {
            self.cached_filtered_files = self.files.clone();
        }
        self.recalculate_column_widths();
    }

    pub fn recalculate_column_widths(&mut self) {
        use crate::ui::format::{format_date, format_size};
        use crate::ui::i18n::Language;
        let measure = |txt: &str| txt.chars().count() as f32 * 6.5;

        // Width reservations cover the longest built-in labels in both catalogs.
        // These are layout measurements, not user-facing strings.
        let mut max_type_w: f32 = 15.0 * 6.5;
        let mut max_size_w: f32 = 6.0 * 6.5;
        let mut max_date_w: f32 = 13.0 * 6.5;

        // We use the full files list instead of cached_filtered_files
        // to keep columns stable while the user interacts with filters.
        for f in &self.files {
            let type_str = if f.is_dir {
                "Dossier".to_string()
            } else {
                std::path::Path::new(&f.name)
                    .extension()
                    .and_then(|e| e.to_str())
                    .map(|s| s.to_uppercase())
                    .unwrap_or("Fichier".to_string())
            };
            max_type_w = max_type_w.max(measure(&type_str));

            // Reserve enough width for both supported display languages.
            // This keeps columns stable when the user switches language.
            for language in [Language::French, Language::English] {
                let size_str = format_size(f.size, f.is_dir, f.is_calculated, language);
                max_size_w = max_size_w.max(measure(&size_str));
                let date_str = format_date(f.modified, language);
                max_date_w = max_date_w.max(measure(&date_str));
            }
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
                    let ext_a = std::path::Path::new(&a.name)
                        .extension()
                        .and_then(|e| e.to_str())
                        .unwrap_or("");
                    let ext_b = std::path::Path::new(&b.name)
                        .extension()
                        .and_then(|e| e.to_str())
                        .unwrap_or("");
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

    #[allow(dead_code)]
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

pub struct WorkspaceState {
    pub pinned_items: Vec<QuickAccessItem>,
    pub drives: Vec<DriveInfo>,
    pub layout: LayoutNode,
    pub active_pane_id: String,
    pub expanded_paths: std::collections::HashSet<String>,
    pub sidebar_items: Vec<SidebarItem>,
    pub sidebar_nodes: std::collections::HashMap<String, Vec<SidebarNode>>,
    next_tab_id: u64,
}

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

    pub fn saved_snapshot(&self, sidebar_width: f32) -> SavedWorkspace {
        SavedWorkspace {
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
            pane.tabs.push(new_tab);

            if activate {
                pane.active_tab_index = pane.tabs.len() - 1;
                self.active_pane_id = pane_id.to_string();
            }
        }
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
                let id = format!("tab-{}", self.next_tab_id);
                self.next_tab_id = self.next_tab_id.wrapping_add(1);
                if let Some(pane) = self.find_pane_mut(pane_id) {
                    pane.tabs[0] = TabState::new(id, path);
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
        let path = if let Some(pane) = self.find_pane(pane_id) {
            pane.tabs.get(tab_idx).map(|t| t.path.clone())
        } else {
            None
        };

        if let Some(path) = path {
            self.add_tab(pane_id, path, true);
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

    /// Re-fetches Quick Access items from Windows then rebuilds the sidebar.
    /// Call this after add_to_quick_access / remove_from_quick_access.
    pub fn reload_quick_access_and_refresh(&mut self) {
        self.pinned_items = get_quick_access_items().unwrap_or_default();
        self.refresh_sidebar();
    }

    pub fn refresh_sidebar(&mut self) {
        let mut new_items = Vec::new();

        if !self.pinned_items.is_empty() {
            new_items.push(SidebarItem::Header(Message::SidebarQuickAccess));
            for item in &self.pinned_items {
                new_items.push(SidebarItem::Pinned { item: item.clone() });
            }
        }

        new_items.push(SidebarItem::Header(Message::SidebarDrives));

        for drive in &self.drives {
            let is_expanded = self.expanded_paths.contains(&drive.path);
            new_items.push(SidebarItem::Drive {
                drive: drive.clone(),
                is_expanded,
            });

            if is_expanded {
                self.populate_subdir(&drive.path, 1, &mut new_items);
            }
        }

        new_items.push(SidebarItem::Header(Message::SidebarLocations));
        #[cfg(target_os = "windows")]
        new_items.push(SidebarItem::Pinned {
            item: QuickAccessItem {
                name: "Réseau".to_string(),
                path: "__network_vincinity__".to_string(),
            },
        });
        new_items.push(SidebarItem::Pinned {
            item: QuickAccessItem {
                name: "Corbeille".to_string(),
                path: "trash://".to_string(),
            },
        });

        self.sidebar_items = new_items;
    }

    fn populate_subdir(&self, path: &str, depth: usize, items: &mut Vec<SidebarItem>) {
        if let Some(nodes) = self.sidebar_nodes.get(path) {
            for node in nodes {
                let is_expanded = self.expanded_paths.contains(&node.path);
                let path_copy = node.path.clone();
                items.push(SidebarItem::Directory {
                    node: node.clone(),
                    depth,
                    is_expanded,
                });

                if is_expanded {
                    self.populate_subdir(&path_copy, depth + 1, items);
                }
            }
        }
    }

    pub fn toggle_sidebar_path(&mut self, path: String) {
        if self.expanded_paths.contains(&path) {
            self.expanded_paths.remove(&path);
        } else {
            self.expanded_paths.insert(path);
        }
        self.refresh_sidebar();
    }

    pub fn _collapse_all_sidebar_path(&mut self, path: String) {
        let norm_path = path.replace("\\", "/");
        let target_prefix = if norm_path.ends_with("/") {
            norm_path.clone()
        } else {
            format!("{}/", norm_path)
        };
        self.expanded_paths.retain(|p| {
            let norm_p = p.replace("\\", "/");
            norm_p != norm_path && !norm_p.starts_with(&target_prefix)
        });
        self.refresh_sidebar();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn closing_tab_before_active_preserves_active_tab_identity() {
        let mut state = WorkspaceState::new();
        let pane_id = state.active_pane_id.clone();
        state.add_tab(&pane_id, "B".to_string(), true);
        state.add_tab(&pane_id, "C".to_string(), false);
        let active_id = state.find_pane(&pane_id).unwrap().active_tab().id.clone();

        state.close_tab_id(&pane_id, 0);

        let pane = state.find_pane(&pane_id).unwrap();
        assert_eq!(pane.active_tab().id, active_id);
    }

    #[test]
    fn invalid_tab_close_is_ignored() {
        let mut state = WorkspaceState::new();
        let pane_id = state.active_pane_id.clone();
        let initial_len = state.find_pane(&pane_id).unwrap().tabs.len();

        state.close_tab_id(&pane_id, usize::MAX);

        assert_eq!(state.find_pane(&pane_id).unwrap().tabs.len(), initial_len);
    }

    #[test]
    fn tab_ids_are_not_reused_after_close() {
        let mut state = WorkspaceState::new();
        let pane_id = state.active_pane_id.clone();
        state.add_tab(&pane_id, "B".to_string(), true);
        let removed_id = state.find_pane(&pane_id).unwrap().active_tab().id.clone();
        state.close_tab_id(&pane_id, 1);
        state.add_tab(&pane_id, "C".to_string(), true);
        let new_id = state.find_pane(&pane_id).unwrap().active_tab().id.clone();

        assert_ne!(removed_id, new_id);
    }

    #[test]
    fn reordering_an_inactive_tab_preserves_the_active_tab() {
        let mut state = WorkspaceState::new();
        let pane_id = state.active_pane_id.clone();
        state.add_tab(&pane_id, "B".to_string(), true);
        state.add_tab(&pane_id, "C".to_string(), false);
        let active_id = state.find_pane(&pane_id).unwrap().active_tab().id.clone();

        state.reorder_tabs_id(&pane_id, 2, 0);

        assert_eq!(
            state.find_pane(&pane_id).unwrap().active_tab().id,
            active_id
        );
    }

    #[test]
    fn moving_to_an_unknown_pane_keeps_the_tab_and_active_selection() {
        let mut state = WorkspaceState::new();
        let pane_id = state.active_pane_id.clone();
        state.add_tab(&pane_id, "B".to_string(), true);
        let tabs_before = state.find_pane(&pane_id).unwrap().tabs.clone();
        let active_id = state.find_pane(&pane_id).unwrap().active_tab().id.clone();

        state.move_tab_to_pane_id(&pane_id, 0, "missing-pane");

        let pane = state.find_pane(&pane_id).unwrap();
        assert_eq!(pane.tabs.len(), tabs_before.len());
        assert_eq!(pane.active_tab().id, active_id);
    }

    #[test]
    fn splitting_a_tab_preserves_both_tab_identities_and_source_selection() {
        let mut state = WorkspaceState::new();
        let source_pane_id = state.active_pane_id.clone();
        state.add_tab(&source_pane_id, "second".to_string(), true);
        let moved_id = state.find_pane(&source_pane_id).unwrap().tabs[0].id.clone();
        let active_id = state
            .find_pane(&source_pane_id)
            .unwrap()
            .active_tab()
            .id
            .clone();

        state.split_tab_to_axis(
            &source_pane_id,
            0,
            &source_pane_id,
            LayoutAxis::Horizontal,
            true,
        );

        assert_eq!(
            state.find_pane(&source_pane_id).unwrap().active_tab().id,
            active_id
        );
        assert!(state.find_tab_mut(&moved_id).is_some());
        let mut pane_count = 0;
        state.layout.visit_panes(&mut |_| pane_count += 1);
        assert_eq!(pane_count, 2);
    }

    #[test]
    fn saved_workspace_restores_split_panels_and_open_tab_paths() {
        let mut state = WorkspaceState::new();
        let source_pane_id = state.active_pane_id.clone();
        state.add_tab(&source_pane_id, "D:/first".to_string(), true);
        state.add_tab(&source_pane_id, "E:/active".to_string(), true);
        state.split_tab_to_axis(
            &source_pane_id,
            2,
            &source_pane_id,
            LayoutAxis::Horizontal,
            true,
        );

        let active_pane_id = state.active_pane_id.clone();
        state.add_tab(&active_pane_id, "F:/second".to_string(), true);
        if let LayoutNode::Split { weights, .. } = &mut state.layout {
            *weights = vec![1.4, 0.6];
        } else {
            panic!("expected a split layout");
        }

        let value = serde_json::to_value(state.saved_snapshot(320.0)).unwrap();
        let snapshot: SavedWorkspace = serde_json::from_value(value).unwrap();
        let (restored, sidebar_width) = WorkspaceState::from_saved_snapshot(snapshot).unwrap();

        assert_eq!(sidebar_width, 320.0);
        assert_eq!(restored.active_pane_id, active_pane_id);
        let pane = restored.find_pane(&active_pane_id).unwrap();
        assert_eq!(pane.tabs.len(), 2);
        assert_eq!(pane.tabs[0].path, "E:/active");
        assert_eq!(pane.active_tab().path, "F:/second");
        match &restored.layout {
            LayoutNode::Split { weights, .. } => assert_eq!(weights, &[1.4, 0.6]),
            LayoutNode::Pane(_) => panic!("expected split layout after restore"),
        }
    }

    #[test]
    fn selecting_every_present_type_remains_an_explicit_filter_state() {
        let mut tab = TabState::new("tab".to_string(), String::new());
        tab.files = vec![FileEntry {
            name: "notes.txt".to_string(),
            ..FileEntry::default()
        }];
        tab.filter_types.insert("TXT".to_string());

        tab.update_cache();

        assert!(tab.filter_types.contains("TXT"));
        assert_eq!(tab.cached_filtered_files.len(), 1);
    }
}
