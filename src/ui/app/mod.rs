pub(crate) mod tool_geometry;
use crate::ui::scale::{logical, rem_pixels};
pub use crate::ui::theme::{OxydeTheme, ThemeContext};
pub use crate::ui::workspace::WorkspaceState;
pub use gpui::prelude::*;
use gpui::*;
pub use oxyde_lib::models::FileEntry;

pub mod filters;
pub(crate) mod trash_settings;
pub(crate) mod properties;
pub(crate) mod disk_images;
mod drive_watchers;
pub mod layout;
pub mod navigation;
mod sizes;
mod preferences;
pub(crate) mod shutdown;
mod grouping;
mod group_selection;
pub(crate) use group_selection::GroupSelection;
mod keyboard;
mod toasts;
mod updates;
pub(crate) use updates::UpdateStatus;
mod shell_features;
mod calendar;
mod clipboard;
mod file_operations;
mod file_preparation;
mod archives;
mod rename;
mod context_menus;
mod drag;
pub(crate) use drag::external::FileDropTargetExt;
#[cfg(target_os = "windows")]
pub(crate) use drag::native::close as close_file_drag;
mod dialog_windows;
pub(crate) use dialog_windows::close_dialog_windows;
mod render;
pub(crate) mod viewer;
mod context_menu_render;
pub use drag::drives_differ;
use toasts::{Toast, ToastKind};
pub(crate) mod network;
pub mod selection;
mod item_pointer;
pub(crate) mod search;
pub(crate) mod advanced_search;
pub(crate) mod duplicates;

use crate::ui::components::input::TextInput;
use crate::ui::i18n::{FluentArgs, I18nContext, Message};

actions!(
    oxyde,
    [
        Ouvrir,
        Copier,
        CopierNom,
        CopierChemin,
        Couper,
        Coller,
        Renommer,
        Supprimer,
        ToutSelectionner,
        Annuler,
        Refaire,
        Actualiser
    ]
);

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum SettingsDataAction {
    ClearCache,
    ResetPreferences,
}

pub(crate) struct SettingsDataConfirmation {
    pub action: SettingsDataAction,
    pub focus: FocusHandle,
    pub confirm_focus: FocusHandle,
}

#[derive(Debug, PartialEq, Clone)]
pub enum DragTarget {
    SidebarV {
        viewport_h: f32,
        content_h: f32,
        track_id: String,
    },
    SidebarH {
        viewport_w: f32,
        content_w: f32,
        track_id: String,
    },
    SidebarResize,
    FileListV {
        pane_id: String,
        viewport_h: f32,
        content_h: f32,
        track_id: String,
    },
    FileListH {
        pane_id: String,
        viewport_w: f32,
        content_w: f32,
        track_id: String,
    },
    BreadcrumbV {
        viewport_h: f32,
        content_h: f32,
        track_id: String,
    },
    ContextMenuV {
        menu_idx: usize,
        viewport_h: f32,
        content_h: f32,
        track_id: String,
    },
    OperationsPanelV {
        viewport_h: f32,
        content_h: f32,
        track_id: String,
    },
    PaneResize {
        split_id: String,
        child_idx: usize,
        initial_weights: Vec<f32>,
    },
    SelectionRectangle {
        pane_id: String,
        start_pos: Point<Pixels>,
        current_pos: Point<Pixels>,
        modifiers: Modifiers,
        initial_selection: std::collections::HashSet<String>,
    },
}

#[derive(Clone)]
#[allow(clippy::enum_variant_names)]
pub enum ContextMenuHeader {
    TypeSelectAll { pane_id: String },
    SizeSelectAll { pane_id: String },
    DateSelectAll { pane_id: String },
}

#[derive(Clone)]
pub enum ContextMenuContent {
    Items(
        Vec<crate::ui::components::dropdown::DropdownItem>,
        Option<ContextMenuHeader>,
    ),
    NativeItems {
        path: Vec<String>,
        is_background: bool,
        items: Vec<oxyde_lib::models::WinMenuItem>,
    },
    NameFilter {
        input: Entity<crate::ui::components::input::TextInput>,
    },
}

#[derive(Clone)]
pub struct ContextMenuInstance {
    pub(crate) native_bounds: Option<Bounds<Pixels>>,
    pub position: Point<Pixels>,
    pub content: ContextMenuContent,
    pub scroll_handle: UniformListScrollHandle,
    pub width: Pixels,
    pub bottom_padding: f32,
    /// If true, the menu opens to the LEFT of its anchor (use Anchor::TopRight).
    pub open_left: bool,
    /// Opens above `position` instead of below it.
    pub open_up: bool,
    pub hovered_item: Option<usize>,
    /// Request token for an in-flight native submenu; stale replies are discarded.
    pub native_request_id: Option<u64>,
}

#[derive(Clone)]
pub(crate) struct PendingFileOperation {
    pub(crate) paths: Vec<String>,
    target_dir: String,
    pub(crate) is_move: bool,
    pub(crate) conflicts: Vec<oxyde_lib::models::ConflictEntry>,
    pub(crate) dialog: crate::ui::components::conflict_dialog::ConflictDialogState,
    runtime: tokio::runtime::Handle,
}

#[derive(Clone)]
pub struct FileDragData {
    pub paths: Vec<String>,
    pub source_pane_id: String,
}

pub struct FileDragPreview {
    pub paths: Vec<String>,
    single_is_dir: bool,
    pub cursor_offset: Point<Pixels>,
    pub app_handle: Entity<OxydeApp>,
    _subscription: Subscription,
    #[cfg(test)]
    rendered_copy_mode: Option<bool>,
}

pub struct OxydeApp {
    pub state: WorkspaceState,
    pub(crate) viewers: std::collections::HashMap<String, Entity<viewer::Viewer>>,
    pub handle: Option<Entity<Self>>,
    pub(crate) next_file_request: u64,
    pub(crate) next_native_menu_request: u64,
    pub(crate) pending_file_requests: std::collections::HashMap<String, u64>,
    pending_preparations: std::collections::HashMap<String, (u64, Task<()>)>,
    next_preparation: u64,
    pub folder_has_subdirs: std::sync::Arc<std::collections::HashMap<String, bool>>,
    pub(crate) pending_sidebar_requests: std::collections::HashMap<String, u64>,
    pub history: std::sync::Arc<oxyde_lib::models::HistoryManager>,
    pub(crate) history_operation_in_progress: bool,
    pub focus_handle: FocusHandle,
    pub(crate) sidebar_scroll: UniformListScrollHandle,
    pub(crate) sidebar_scroll_h: ScrollHandle,
    pub(crate) sidebar_rail_scroll: ScrollHandle,
    pub(crate) breadcrumb_scroll: UniformListScrollHandle,
    pub drag_target: Option<DragTarget>,
    item_press: Option<item_pointer::ItemPress>,
    item_selection_drag: bool,
    pub(crate) suppress_item_selection_click: bool,
    pub drop_target_idx: Option<usize>,
    pub drop_target_pane: Option<String>,
    pub highlight_right_zone: bool,
    pub highlight_left_zone: bool,
    pub highlight_top_zone: bool,
    pub highlight_bottom_zone: bool,
    pub highlight_split_pane: Option<String>,
    /// Last pane offered a split highlight during the current tab drag, kept so the highlight can fade out.
    pub(crate) split_highlight_last: Option<String>,
    /// Pane whose vertical scrollbar zone is under the pointer.
    pub(crate) scroll_zone_hover: Option<String>,
    /// Start of the operations halo cycle, and when its last cycle ends once operations stop.
    pub(crate) operations_breath: std::cell::Cell<Option<(std::time::Instant, Option<std::time::Instant>)>>,
    pub grab_offset: f32,
    pub sidebar_width: f32,
    pub(crate) sidebar_collapsed: bool,
    pub sidebar_content_width: f32,
    pub sidebar_selected_path: Option<String>,
    pub(crate) sidebar_follow_path: Option<String>,
    pub(crate) sidebar_follow_task: Option<Task<()>>,
    pub list_height: f32,
    pub list_width: f32,
    pub split_sizes: std::collections::HashMap<String, f32>,
    pub pane_sizes: std::collections::HashMap<String, Size<f32>>,
    pub list_bounds: std::collections::HashMap<String, Bounds<f32>>,
    pub scrollbar_origins: std::collections::HashMap<String, Point<f32>>,
    pub breadcrumb_menu: Option<(String, String, usize, Vec<FileEntry>)>, // (pane_id, path, index, dirs)
    pub context_menus: Vec<ContextMenuInstance>,
    /// Control that opened the current root menu, identified with that menu's scroll state.
    pub(crate) context_menu_owner: Option<(SharedString, UniformListScrollHandle)>,
    /// Owner whose menu the native outside click just dismissed, before the click reaches it.
    pub(crate) dismissed_menu_owner: Option<(SharedString, std::time::Instant)>,
    pub(crate) context_anchor_points: std::collections::HashMap<String, Point<Pixels>>,
    pub submenu_timer: Option<Task<()>>,
    pub submenu_close_timer: Option<Task<()>>,
    pub dragging_tab: Option<(String, usize)>,
    pub file_ops: std::sync::Arc<oxyde_lib::systems::file_ops::FileOperationManager>,
    pub watcher_changes: drive_watchers::WatcherChanges,
    pub(super) watcher_pending_listings: std::collections::HashSet<String>,
    pub _path_watchers: std::collections::HashMap<String, drive_watchers::DirectoryWatcher>,
    pub(super) pending_watchers: std::collections::HashMap<String, (u64, bool, bool)>,
    pub(super) next_watcher_request: u64,
    pub(super) drive_metadata_ready: bool,
    pub(super) drive_refresh_epoch: u64,
    pub _watcher_task: Option<Task<()>>,
    pub(super) shell_features_task: Option<Task<()>>,
    calendar_task: Option<Task<()>>,
    pub(super) quick_access_request: u64,
    pub(crate) onedrive_installed: bool,
    pub(crate) peek_status: oxyde_lib::platform::PeekStatus,
    pub _drive_refresh_task: Option<Task<()>>,
    pub(super) drive_details_task: Option<Task<()>>,
    pub(crate) ejecting_drive: bool,
    pub(crate) mounted_disk_images: Vec<oxyde_lib::platform::MountedDiskImage>,
    pub(super) disk_image_refresh_task: Option<Task<()>>,
    pub(crate) shift_delete: bool,
    pub(crate) trash_delete_policy: trash_settings::DeletePolicy,
    pub(super) trash_delete_check_pending: bool,
    pub(crate) pending_trash_delete: Option<trash_settings::PendingDelete>,
    pub(crate) trash_settings: Option<trash_settings::TrashSettings>,
    dialog_windows_open: [bool; dialog_windows::DIALOG_KINDS],
    pub(crate) network_dialog: Option<network::NetworkDialog>,
    pub(crate) network_dialog_revision: u64,
    pub(crate) network_force_confirmation: Option<network::NetworkForceConfirmation>,
    pub(crate) network_force_focus_pending: bool,
    pub(crate) network_focus_restore: bool,
    pub _subscriptions: Vec<Subscription>,
    pub(crate) window_geometry_save_task: Option<Task<()>>,
    /// Effective drag operation: true = copy, false = move.
    /// Derived from `drag_modifier_copy` (if Some) or `drag_target_default_copy` otherwise.
    pub drag_copy_mode: bool,
    pub(crate) drag_new_tab: bool,
    pub(crate) file_drag_origin: Option<(String, String)>,
    external_drop: drag::external::ExternalDropState,
    #[cfg(target_os = "windows")]
    file_drag_session: Option<drag::native::Outgoing>,
    /// Modifier override: Some(true) = Ctrl (always copy), Some(false) = Shift (always move),
    /// None = auto (determined by whether source and target are on different drives).
    pub drag_modifier_copy: Option<bool>,
    /// Drive-based default for the currently hovered drop target:
    /// true if source and target are on different drives → default to copy.
    pub drag_target_default_copy: bool,

    pub editing_path: Option<String>,
    pub editing_text: String,
    pub editing_view: Option<Entity<TextInput>>,
    pub inline_rename: bool,
    created_item_rename: Option<(String, String, bool)>,
    pub(crate) trash_busy: bool,
    pub(crate) clipboard_has_files: bool,
    pub breadcrumb_editing: bool,
    pub(crate) pinned_rail_open: bool,
    pub breadcrumb_text: String,
    pub breadcrumb_pane_id: Option<String>,
    pub breadcrumb_shake_id: u64,
    pub breadcrumb_invalid: bool,
    breadcrumb_validation_revision: u64,
    breadcrumb_validation_task: Option<Task<()>>,
    rename_click_candidate: Option<(String, std::time::Instant)>,
    rename_pending: bool,
    rename_cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
    rename_generation: u64,
    pub(crate) pending_file_operations: std::collections::VecDeque<PendingFileOperation>,
    file_operation_notice: Option<(Message, String)>,
    toasts: Vec<Toast>,
    next_toast_id: u64,
    pub(crate) update_status: UpdateStatus,
    main_window: Option<AnyWindowHandle>,
    pub(crate) operations_panel_view:
        Option<Entity<crate::ui::components::operations::OperationsPanelView>>,
    pub(crate) operations_panel_scroll: ScrollHandle,
    pub(crate) expanded_operations: std::collections::HashSet<String>,
    pub(crate) archive_creation: Option<(Vec<std::path::PathBuf>, String)>,
    pub(crate) rename_error: Option<Message>,
    pub(crate) rename_shake_id: u64,

    // ── Settings ─────────────────────────────────────────────────────────────
    pub show_hidden: bool,
    pub show_system: bool,
    pub show_media_previews: bool,
    pub show_grid_thumbnails: bool,
    pub settings_open: bool,
    pub(crate) settings_about_open: bool,
    pub(crate) settings_button_bounds: Option<Bounds<Pixels>>,
    pub(crate) tools_button_bounds: Option<Bounds<Pixels>>,
    pub(crate) settings_submenu_bounds: Option<Bounds<Pixels>>,
    pub(crate) archive_format_bounds: Option<Bounds<Pixels>>,
    pub(crate) settings_picker: Option<crate::ui::components::settings::SettingsPicker>,
    pub(crate) settings_confirmation: Option<SettingsDataConfirmation>,
    pub(crate) theme_id: crate::ui::theme::ThemeId,
    pub(crate) language: crate::ui::i18n::Language,
    pub(crate) settings_error: Option<Message>,
    pub(crate) settings_data_status: Option<Message>,
    pub(crate) clearing_cache: bool,
    config: oxyde_lib::models::AppConfig,
    config_path: Option<std::path::PathBuf>,
    preferences_load_failed: bool,
    preferences_save_generation: u64,
    preferences_save_task: Option<Task<()>>,
}

impl OxydeApp {
    pub fn new(
        config: oxyde_lib::models::AppConfig,
        config_path: Option<std::path::PathBuf>,
        load_failed: bool,
        cx: &mut Context<Self>,
    ) -> Self {
        oxyde_lib::commands::io::set_folder_size_turbo(config.default_turbo_mode);
        let restored_workspace = config
            .workspace_state
            .as_ref()
            .cloned()
            .and_then(|value| {
                match serde_json::from_value::<crate::ui::workspace::SavedWorkspace>(value) {
                    Ok(snapshot) => {
                        crate::ui::workspace::WorkspaceState::from_saved_snapshot(snapshot)
                    }
                    Err(error) => {
                        log::warn!("Cannot restore saved workspace: {error:?}");
                        None
                    }
                }
            });
        let (state, sidebar_width) =
            restored_workspace.unwrap_or_else(|| (WorkspaceState::new(), 260.0));
        Self {
            state,
            viewers: Default::default(),
            handle: None,
            next_file_request: 0,
            next_native_menu_request: 0,
            pending_file_requests: std::collections::HashMap::new(),
            folder_has_subdirs: Default::default(),
            pending_preparations: Default::default(),
            next_preparation: 0,
            pending_sidebar_requests: std::collections::HashMap::new(),
            history: std::sync::Arc::new(oxyde_lib::models::HistoryManager::new()),
            history_operation_in_progress: false,
            focus_handle: cx.focus_handle(),
            sidebar_scroll: UniformListScrollHandle::new(),
            sidebar_scroll_h: ScrollHandle::new(),
            sidebar_rail_scroll: ScrollHandle::new(),
            breadcrumb_scroll: UniformListScrollHandle::new(),
            drag_target: None,
            item_press: None,
            item_selection_drag: false,
            suppress_item_selection_click: false,
            drop_target_idx: None,
            drop_target_pane: None,
            highlight_right_zone: false,
            highlight_left_zone: false,
            highlight_top_zone: false,
            highlight_bottom_zone: false,
            highlight_split_pane: None,
            split_highlight_last: None,
            scroll_zone_hover: None,
            operations_breath: Default::default(),
            grab_offset: 0.0,
            sidebar_width,
            sidebar_collapsed: config
                .workspace_state
                .as_ref()
                .and_then(|value| value.get("sidebar_collapsed"))
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false),
            sidebar_content_width: 260.0,
            sidebar_selected_path: None,
            sidebar_follow_path: None,
            sidebar_follow_task: None,
            list_height: logical(800.0),
            list_width: logical(1000.0),
            split_sizes: std::collections::HashMap::new(),
            pane_sizes: std::collections::HashMap::new(),
            list_bounds: std::collections::HashMap::new(),
            scrollbar_origins: std::collections::HashMap::new(),
            breadcrumb_menu: None,
            context_menus: Vec::new(),
            context_menu_owner: None,
            dismissed_menu_owner: None,
            context_anchor_points: std::collections::HashMap::new(),
            submenu_timer: None,
            submenu_close_timer: None,
            dragging_tab: None,
            file_ops: std::sync::Arc::new(oxyde_lib::systems::file_ops::FileOperationManager::new()),
            watcher_changes: std::sync::Arc::new(std::sync::Mutex::new(
                std::collections::HashMap::new(),
            )),
            watcher_pending_listings: Default::default(),
            _path_watchers: std::collections::HashMap::new(),
            pending_watchers: std::collections::HashMap::new(),
            next_watcher_request: 0,
            drive_metadata_ready: false,
            drive_refresh_epoch: 0,
            _watcher_task: None,
            shell_features_task: None,
            calendar_task: None,
            quick_access_request: 0,
            onedrive_installed: false,
            peek_status: Default::default(),
            _drive_refresh_task: None,
            drive_details_task: None,
            ejecting_drive: false,
            mounted_disk_images: Vec::new(),
            disk_image_refresh_task: None,
            network_dialog: None,
            trash_settings: None,
            dialog_windows_open: [false; dialog_windows::DIALOG_KINDS],
            pending_trash_delete: None,
            shift_delete: false,
            trash_delete_policy: Default::default(),
            trash_delete_check_pending: false,
            network_dialog_revision: 0,
            network_force_confirmation: None,
            network_force_focus_pending: false,
            network_focus_restore: false,
            window_geometry_save_task: None,
            _subscriptions: vec![cx.on_app_quit(|this, cx| {
                this.flush_window_geometry(cx);
                crate::trace_shutdown_request("GPUI quit hook");
                // Undo history is session-only, so its replacement backups must
                // not outlive a normal application shutdown. Keep the last
                // history owner with the physical cleanup, away from the UI.
                let cleanup = shutdown::begin_history_cleanup(this.history.clone());
                let save = this.final_preferences_write();
                async move {
                    if let Some(save) = save {
                        crate::trace_shutdown_stage("waiting for preference saves");
                        save.await;
                    }
                    cleanup.await;
                    crate::trace_shutdown_stage("GPUI quit hook completed");
                }
            })],
            drag_copy_mode: false,
            drag_new_tab: false,
            file_drag_origin: None,
            external_drop: Default::default(),
            #[cfg(target_os = "windows")]
            file_drag_session: None,
            drag_modifier_copy: None,
            drag_target_default_copy: false,
            editing_path: None,
            editing_text: String::new(),
            editing_view: None,
            inline_rename: false,
            created_item_rename: None,
            trash_busy: false,
            clipboard_has_files: false,
            breadcrumb_editing: false,
            pinned_rail_open: false,
            breadcrumb_text: String::new(),
            breadcrumb_pane_id: None,
            breadcrumb_validation_revision: 0,
            breadcrumb_validation_task: None,
            breadcrumb_shake_id: 0,
            breadcrumb_invalid: false,
            rename_click_candidate: None,
            rename_pending: false,
            rename_cancel: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
            rename_generation: 0,
            pending_file_operations: std::collections::VecDeque::new(),
            file_operation_notice: None,
            toasts: Vec::new(),
            next_toast_id: 0,
            update_status: UpdateStatus::Idle,
            main_window: None,
            operations_panel_view: None,
            operations_panel_scroll: ScrollHandle::new(),
            expanded_operations: std::collections::HashSet::new(),
            archive_creation: None,
            rename_error: None,
            rename_shake_id: 0,
            show_hidden: config.show_hidden,
            show_system: config.show_system,
            show_media_previews: config.show_previews,
            show_grid_thumbnails: config.show_grid_thumbnails,
            settings_open: false,
            settings_about_open: false,
            settings_button_bounds: None,
            tools_button_bounds: None,
            settings_submenu_bounds: None,
            archive_format_bounds: None,
            settings_picker: None,
            settings_confirmation: None,
            theme_id: crate::ui::theme::ThemeId::from_id(&config.theme),
            language: crate::ui::i18n::Language::from_id(&config.language),
            settings_error: load_failed.then_some(Message::SettingsLoadError),
            settings_data_status: None,
            clearing_cache: false,
            config,
            config_path,
            preferences_load_failed: load_failed,
            preferences_save_generation: 0,
            preferences_save_task: None,
        }
    }

}
