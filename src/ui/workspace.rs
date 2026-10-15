mod filters;
mod operations;
mod sidebar;
mod snapshots;
mod tab;
mod tree;

use crate::ui::date_groups::{CalendarReference, DateGroup};
use crate::ui::i18n::Message;
use gpui::{ScrollHandle, UniformListScrollHandle};
use oxyde_lib::models::{DriveInfo, FileEntry, QuickAccessItem, SidebarNode};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SavedWorkspace {
    pub layout: SavedLayoutNode,
    pub active_pane_id: String,
    #[serde(default = "default_sidebar_width")]
    pub sidebar_width: f32,
    #[serde(default)]
    pub sidebar_collapsed: bool,
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

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewMode {
    Details,
    #[default]
    Grid,
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
    pub view_mode: ViewMode,
    #[serde(default)]
    pub group_by_date: bool,
    #[serde(default)]
    pub back_stack: Vec<String>,
    #[serde(default)]
    pub forward_stack: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SortColumn {
    Name,
    Location,
    OriginalLocation,
    DeletedAt,
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
    OneDrive { item: QuickAccessItem },
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

#[derive(Clone)]
pub struct TabState {
    pub quick_search: Option<crate::ui::app::search::QuickSearchState>,
    pub search_input: Option<gpui::Entity<crate::ui::components::input::TextInput>>,
    pub search_slide: crate::ui::app::search::SearchInputSlide,
    pub search_text: String,
    pub search_revision: u64,
    pub pending_reveal: Option<String>,
    pub id: String,
    pub path: String,
    pub files: Vec<FileEntry>,
    pub show_size_histogram: bool,
    pub size_calculation: Option<std::sync::Arc<gpui::Task<()>>>,
    pub back_stack: Vec<String>,
    pub forward_stack: Vec<String>,
    pub list_scroll: UniformListScrollHandle,
    pub list_scroll_h: ScrollHandle,
    pub breadcrumb_scroll: ScrollHandle,
    pub breadcrumb_scroll_path: std::cell::RefCell<String>,
    pub breadcrumb_viewport_width: std::rc::Rc<std::cell::Cell<f32>>,
    pub sort_column: SortColumn,
    pub sort_direction: SortDirection,
    pub filter_types: std::collections::HashSet<String>,
    pub filter_name: String,
    pub filter_original_location: String,
    pub filter_sizes: std::collections::HashSet<SizeCategory>,
    pub filter_dates: std::collections::HashSet<DateGroup>,
    pub selected_paths: std::collections::HashSet<String>,
    pub selection_anchor: Option<String>,
    pub keyboard_cursor: Option<String>,
    pub view_mode: ViewMode,
    pub group_by_date: bool,
    pub collapsed_date_groups: std::collections::HashSet<crate::ui::date_groups::DateGroup>,
    pub cached_filtered_files: Vec<FileEntry>,
    filter_cache_revision: u64,
    calendar_reference: Option<CalendarReference>,
    pub type_col_width: f32,
    pub size_col_width: f32,
    pub date_col_width: f32,
    pub deleted_col_width: f32,
    pub column_type_labels: std::collections::HashSet<String>,
    pub column_size_values: std::collections::HashSet<(u64, bool, bool)>,
    pub column_measurement_key: Option<(u32, crate::ui::i18n::Language, crate::ui::format::DateFormat, bool, bool, bool)>,
    pub load_status: TabLoadStatus,
}

#[derive(Clone)]
pub enum TabLoadStatus {
    Loading,
    Ready,
    Error(Message),
    ArchiveLimit(oxyde_lib::models::error::ArchiveLimitFailure),
    ArchiveListing(oxyde_lib::models::error::ArchiveListingFailure),
}

#[derive(Clone)]
struct FileFilters {
    filter_types: std::collections::HashSet<String>,
    filter_name: String,
    filter_original_location: String,
    filter_sizes: std::collections::HashSet<SizeCategory>,
    filter_dates: std::collections::HashSet<DateGroup>,
}
pub(crate) struct CalendarFilterRequest {
    tab_id: String,
    revision: u64,
    reference: CalendarReference,
    files: Vec<FileEntry>,
    filters: FileFilters,
}
pub(crate) struct CalendarFilterResult {
    tab_id: String,
    revision: u64,
    reference: CalendarReference,
    files: Vec<FileEntry>,
}
#[derive(Clone)]
pub struct TabStripState {
    pub animation_generation: std::rc::Rc<std::cell::Cell<u64>>,
    pub animation_target: std::rc::Rc<std::cell::Cell<Option<f32>>>,
    pub scroll: ScrollHandle,
    pub overflow: std::rc::Rc<std::cell::Cell<bool>>,
    pub active_id: std::rc::Rc<std::cell::RefCell<String>>,
    pub active_index: std::rc::Rc<std::cell::Cell<usize>>,
}

#[derive(Clone)]
pub struct PaneState {
    pub id: String,
    pub tabs: Vec<TabState>,
    pub active_tab_index: usize,
    pub tab_strip: TabStripState,
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

pub struct WorkspaceState {
    pub pinned_items: Vec<QuickAccessItem>,
    pub onedrive_items: Vec<QuickAccessItem>,
    pub drives: Vec<DriveInfo>,
    pub layout: LayoutNode,
    pub active_pane_id: String,
    pub expanded_paths: std::collections::HashSet<String>,
    pub sidebar_items: Vec<SidebarItem>,
    pub sidebar_nodes: std::collections::HashMap<String, Vec<SidebarNode>>,
    next_tab_id: u64,
}
