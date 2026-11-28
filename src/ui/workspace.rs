mod filters;
mod operations;
mod closed_tabs;
mod sidebar;
mod snapshots;
mod tab;
mod presentation;
mod sorting;
mod preparation;
mod folder_sizes;
mod selected_paths;
pub(crate) use selected_paths::SelectedPaths;
pub(crate) use preparation::{resolve_native_kinds, FileUpdate};
pub(crate) use folder_sizes::{measurable as measurable_folder, SIZE_FLASH_DURATION};
pub(crate) use sorting::sort_files;
pub use filters::{DateRange, FilterColumn, FilterValue, SizeRange};
pub(crate) use filters::{deleted_ms, FacetKey, NameMatcher};
#[cfg(test)]
pub(crate) use filters::ColumnFilterFlags;
mod tree;
mod viewer;

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
    #[serde(default)]
    pub(crate) closed_tabs: closed_tabs::ClosedTabs,
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
    Tiles,
}

impl ViewMode {
    /// The card layout of the grid and the tiles; the details view has rows.
    pub fn cards(self) -> Option<crate::ui::style::CardLayout> {
        match self {
            Self::Details => None,
            Self::Grid => Some(crate::ui::style::CardLayout::grid()),
            Self::Tiles => Some(crate::ui::style::CardLayout::Tiles),
        }
    }
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
    pub grouping: crate::ui::date_groups::GroupMode,
    #[serde(default)]
    pub sort_column: SortColumn,
    #[serde(default)]
    pub sort_direction: SortDirection,
    #[serde(default)]
    pub back_stack: Vec<String>,
    #[serde(default)]
    pub forward_stack: Vec<String>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SortColumn {
    #[default]
    Name,
    Location,
    OriginalLocation,
    DeletedAt,
    Type,
    Size,
    Permissions,
    Modified,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SortDirection {
    #[default]
    Ascending,
    Descending,
}

#[derive(Clone, Debug)]
pub enum SidebarItem {
    CloudRoot { item: QuickAccessItem },
    /// Parent of the WSL distributions, whose rows are ordinary directories.
    Linux { is_expanded: bool },
    Header(Message),
    Pinned {
        item: QuickAccessItem,
    },
    Drive {
        drive: DriveInfo,
        is_expanded: bool,
    },
    /// A filesystem that a left click mounts; Windows lists none.
    Volume {
        volume: oxyde_lib::platform::UnmountedVolume,
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
    /// Renamed while selected: shown by the least scroll once the listing has it.
    pub pending_rename_reveal: Option<String>,
    /// Restored from the saved workspace and not listed yet: a location that
    /// vanished meanwhile, such as an unmounted volume, falls back home.
    pub restored: bool,
    /// When the tab was created by the user, for its opening flash. Never saved.
    pub opened_at: Option<std::time::Instant>,
    pub id: String,
    pub path: String,
    pub archive_root: Option<std::path::PathBuf>,
    /// The listed folder, when its permissions refuse the user's writes.
    pub denied_folder: Option<String>,
    /// Its listed subfolders whose permissions refuse the user's writes: drops onto them
    /// are refused.
    pub denied_subfolders: std::sync::Arc<std::collections::HashSet<String>>,
    /// Its entries the user may not rename or remove (others' files in `/tmp`).
    pub undeletable_entries: std::sync::Arc<std::collections::HashSet<String>>,
    pub files: std::sync::Arc<Vec<FileEntry>>,
    pub show_size_histogram: bool,
    pub size_calculation: Option<std::sync::Arc<gpui::Task<()>>>,
    pub(crate) folder_sizes: folder_sizes::FolderSizes,
    pub back_stack: Vec<String>,
    pub forward_stack: Vec<String>,
    pub list_scroll: UniformListScrollHandle,
    pub list_scroll_h: ScrollHandle,
    pub breadcrumb_scroll: ScrollHandle,
    pub breadcrumb_scroll_path: std::cell::RefCell<String>,
    pub breadcrumb_viewport_width: std::rc::Rc<std::cell::Cell<f32>>,
    /// Last position of the scroll-to-top button, kept while it fades out.
    pub scroll_top_anchor: std::rc::Rc<std::cell::Cell<f32>>,
    /// Start time, start offset and target of a breadcrumb glide; no target means its end.
    pub breadcrumb_glide: std::rc::Rc<std::cell::Cell<Option<(std::time::Instant, f32, Option<f32>)>>>,
    pub sort_column: SortColumn,
    pub sort_direction: SortDirection,
    pub filter_types: std::collections::HashSet<String>,
    pub filter_name: String,
    /// The name filter is a regular expression rather than a text or a `*` pattern.
    pub filter_name_regex: bool,
    pub filter_original_location: String,
    pub filter_sizes: std::collections::HashSet<SizeCategory>,
    /// Custom size bounds, set instead of `filter_sizes`.
    pub filter_size_range: Option<SizeRange>,
    /// Unix permission bits kept by the Permissions column filter.
    pub filter_permissions: std::collections::HashSet<u32>,
    pub filter_dates: std::collections::HashSet<DateGroup>,
    /// Custom days, set instead of `filter_dates`.
    pub filter_date_range: Option<DateRange>,
    /// Periods of deletion kept in the trash.
    pub filter_deleted: std::collections::HashSet<DateGroup>,
    /// Custom days of deletion, set instead of `filter_deleted`.
    pub filter_deleted_range: Option<DateRange>,
    /// Column filters survive navigating to another folder.
    pub keep_filters: bool,
    pub selected_paths: SelectedPaths,
    pub selection_anchor: Option<String>,
    pub keyboard_cursor: Option<String>,
    pub view_mode: ViewMode,
    pub group_mode: crate::ui::date_groups::GroupMode,
    pub collapsed_groups: std::collections::HashSet<crate::ui::date_groups::GroupKey>,
    pub cached_filtered_files: std::sync::Arc<Vec<FileEntry>>,
    presentation_cache: std::cell::RefCell<Option<std::sync::Arc<presentation::Presentation>>>,
    selection_cache: std::cell::RefCell<Option<std::rc::Rc<tab::SelectionSnapshot>>>,
    filter_flags_cache: std::cell::RefCell<Option<filters::FlagScan>>,
    filter_cache_revision: u64,
    /// The order the last preparation left `files` in. Valid for that vector only:
    /// its Weak keeps the address from another.
    files_sorted: Option<(SortColumn, SortDirection, std::sync::Weak<Vec<oxyde_lib::models::FileEntry>>)>,
    calendar_reference: Option<CalendarReference>,
    pub type_col_width: f32,
    pub size_col_width: f32,
    pub date_col_width: f32,
    pub deleted_col_width: f32,
    pub permissions_col_width: f32,
    pub column_type_labels: std::collections::HashSet<String>,
    pub column_size_values: std::collections::HashSet<(u64, bool, bool)>,
    pub column_measurement_key: Option<(u32, crate::ui::i18n::Language, crate::ui::format::DateFormat)>,
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

#[derive(Clone, PartialEq)]
struct FileFilters {
    filter_types: std::collections::HashSet<String>,
    filter_name: String,
    filter_name_regex: bool,
    trash_search: String,
    filter_original_location: String,
    filter_sizes: std::collections::HashSet<SizeCategory>,
    filter_size_range: Option<SizeRange>,
    filter_permissions: std::collections::HashSet<u32>,
    filter_dates: std::collections::HashSet<DateGroup>,
    filter_date_range: Option<DateRange>,
    filter_deleted: std::collections::HashSet<DateGroup>,
    filter_deleted_range: Option<DateRange>,
}
pub(crate) struct CalendarFilterRequest {
    tab_id: String,
    revision: u64,
    reference: CalendarReference,
    files: std::sync::Arc<Vec<FileEntry>>,
    filters: FileFilters,
}
pub(crate) struct CalendarFilterResult {
    tab_id: String,
    revision: u64,
    reference: CalendarReference,
    files: std::sync::Arc<Vec<FileEntry>>,
}
#[derive(Clone)]
pub struct TabStripState {
    pub animation_generation: std::rc::Rc<std::cell::Cell<u64>>,
    pub animation_target: std::rc::Rc<std::cell::Cell<Option<f32>>>,
    pub scroll: ScrollHandle,
    pub overflow: std::rc::Rc<std::cell::Cell<bool>>,
    pub active_id: std::rc::Rc<std::cell::RefCell<String>>,
    pub active_index: std::rc::Rc<std::cell::Cell<usize>>,
    /// Width of each tab, measured at paint: the room a dragged tab opens or leaves.
    pub tab_widths: std::rc::Rc<std::cell::RefCell<Vec<f32>>>,
    /// While something is dragged near an edge of an overflowing strip: how deep the
    /// pointer is in the edge band (negative on the left), and the last scroll step.
    pub drag_scroll: std::rc::Rc<std::cell::Cell<Option<(f32, std::time::Instant)>>>,
    /// When the chain of frames scrolling the strip last ran: a drag back into the band
    /// before its next frame keeps that chain rather than starting a second one.
    pub drag_scroll_frame: std::rc::Rc<std::cell::Cell<Option<std::time::Instant>>>,
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
    pub cloud_root_items: Vec<QuickAccessItem>,
    pub drives: Vec<DriveInfo>,
    pub unmounted_volumes: Vec<oxyde_lib::platform::UnmountedVolume>,
    pub layout: LayoutNode,
    pub active_pane_id: String,
    pub expanded_paths: std::collections::HashSet<String>,
    pub sidebar_items: Vec<SidebarItem>,
    pub sidebar_nodes: std::collections::HashMap<String, Vec<SidebarNode>>,
    next_tab_id: u64,
    pub(crate) closed_tabs: closed_tabs::ClosedTabs,
}
