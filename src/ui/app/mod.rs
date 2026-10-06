use crate::ui::scale::{logical, rem_pixels};
pub use crate::ui::theme::{OxydeTheme, ThemeContext};
pub use crate::ui::workspace::WorkspaceState;
pub use gpui::prelude::*;
use gpui::*;
pub use oxyde_lib::models::FileEntry;

pub mod filters;
pub(crate) mod disk_images;
mod drive_watchers;
pub mod layout;
pub mod navigation;
mod preferences;
pub(crate) mod network;
pub mod selection;
pub(crate) mod search;
pub(crate) mod advanced_search;
pub(crate) mod duplicates;

#[allow(unused_imports)]
pub use filters::*;
#[allow(unused_imports)]
pub use layout::*;
#[allow(unused_imports)]
pub use navigation::*;
#[allow(unused_imports)]
pub use selection::*;

use crate::ui::components::input::TextInput;
use crate::ui::i18n::{FluentArgs, I18n, I18nContext, Message};

actions!(
    oxyde,
    [
        Ouvrir,
        Copier,
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
        path: String,
        is_background: bool,
        items: Vec<oxyde_lib::models::WinMenuItem>,
    },
    NameFilter {
        input: Entity<crate::ui::components::input::TextInput>,
    },
}

#[derive(Clone)]
pub struct ContextMenuInstance {
    pub position: Point<Pixels>,
    pub content: ContextMenuContent,
    pub scroll_handle: UniformListScrollHandle,
    pub width: Pixels,
    pub bottom_padding: f32,
    /// If true, the menu opens to the LEFT of its anchor (use Anchor::TopRight).
    pub open_left: bool,
    pub hovered_item: Option<usize>,
    /// Request token for an in-flight native submenu; stale replies are discarded.
    pub native_request_id: Option<u64>,
}

#[derive(Clone)]
struct PendingFileOperation {
    paths: Vec<String>,
    target_dir: String,
    is_move: bool,
    conflict_paths: Vec<String>,
    runtime: tokio::runtime::Handle,
}

fn native_menu_max_height(viewport_height: Pixels) -> f32 {
    // Leave 8 px on each side, in addition to the menu's 6 px of chrome.
    (viewport_height.as_f32() - logical(22.0)).max(logical(26.0))
}

fn context_menu_contains_point(
    menu: &ContextMenuInstance,
    position: Point<Pixels>,
    viewport_height: Pixels,
) -> bool {
    let height = context_menu_height(menu, viewport_height);
    let left = if menu.open_left {
        menu.position.x - menu.width
    } else {
        menu.position.x
    };
    let top = context_menu_visible_top(menu.position.y, height, viewport_height);
    Bounds::new(point(left, top), size(menu.width, height)).contains(&position)
}

fn context_menu_height(menu: &ContextMenuInstance, viewport_height: Pixels) -> Pixels {
    let (item_count, header_height, max_height) = match &menu.content {
        ContextMenuContent::Items(items, header) => (
            items.iter().filter(|item| !matches!(item.icon, Some(crate::ui::components::dropdown::IconName::Separator))).count(),
            if header.is_some() { logical(42.0) } else { logical(6.0) },
            logical(480.0),
        ),
        ContextMenuContent::NativeItems { items, .. } => {
            (items.len(), logical(6.0), native_menu_max_height(viewport_height))
        }
        ContextMenuContent::NameFilter { .. } => (0, crate::ui::style::row_height() + logical(8.0), logical(480.0)),
    };
    let header = matches!(&menu.content, ContextMenuContent::Items(_, Some(_)));
    let max_height = crate::ui::style::menu_list_max_height(viewport_height.as_f32(), header, menu.bottom_padding, max_height);
    let separator_height = match &menu.content {
        ContextMenuContent::Items(items, _) => items.iter().filter(|item| matches!(item.icon, Some(crate::ui::components::dropdown::IconName::Separator))).count() as f32 * logical(9.0),
        _ => 0.0,
    };
    px((item_count as f32 * logical(26.0) + separator_height).min(max_height) + header_height + logical(menu.bottom_padding))
}

fn context_menu_visible_top(
    requested_top: Pixels,
    menu_height: Pixels,
    viewport_height: Pixels,
) -> Pixels {
    requested_top
        .min((viewport_height - menu_height).max(px(0.0)))
        .max(px(0.0))
}

/// Returns true if `src` and `dst` are on different drives/volumes.
/// On Windows this compares the root prefix (e.g. "C:" vs "D:").
pub fn drives_differ(src: &str, dst: &str) -> bool {
    let src_root = std::path::Path::new(src).components().next();
    let dst_root = std::path::Path::new(dst).components().next();
    src_root != dst_root
}

#[derive(Clone)]
pub struct FileDragData {
    pub paths: Vec<String>,
    #[allow(dead_code)]
    pub source_pane_id: String,
}

pub struct FileDragPreview {
    pub paths: Vec<String>,
    pub cursor_offset: Point<Pixels>,
    pub app_handle: Entity<OxydeApp>,
    _subscription: Subscription,
}

impl FileDragPreview {
    pub fn new(
        paths: Vec<String>,
        cursor_offset: Point<Pixels>,
        app_handle: Entity<OxydeApp>,
        cx: &mut Context<Self>,
    ) -> Self {
        let sub = cx.observe(&app_handle, |_, _entity, cx| cx.notify());
        Self {
            paths,
            cursor_offset,
            app_handle,
            _subscription: sub,
        }
    }
}

fn drag_op_pill(key_hint: &str, label: &str, active: bool, theme: &OxydeTheme) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .gap(rems(0.25))
        .px(rems(0.375))
        .py(rems(0.1875))
        .rounded_md()
        .bg(if active { theme.accent } else { theme.header })
        .when(!key_hint.is_empty(), |d| {
            d.child(
                div()
                    .text_size(rems(0.5625))
                    .font_weight(FontWeight::BOLD)
                    .text_color(if active {
                        theme.background
                    } else {
                        theme.text_muted
                    })
                    .child(key_hint.to_string()),
            )
        })
        .child(
            div()
                .text_size(rems(0.6875))
                .font_weight(FontWeight::MEDIUM)
                .text_color(if active {
                    theme.background
                } else {
                    theme.text_muted
                })
                .child(label.to_string()),
        )
}

impl Render for FileDragPreview {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let is_copy = self.app_handle.read(cx).drag_copy_mode;
        let theme = *cx.theme();

        let (icon_path, name_label) = if self.paths.len() == 1 {
            let p = std::path::Path::new(&self.paths[0]);
            let is_dir = p.is_dir();
            let name = p
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(cx.i18n().text(Message::FileTypeFile).as_ref())
                .to_string();
            (
                if is_dir {
                    "icons/folder.svg"
                } else {
                    "icons/file.svg"
                },
                name,
            )
        } else {
            ("icons/file.svg", {
                let mut args = FluentArgs::new();
                args.set("count", self.paths.len());
                args.set(
                    "number",
                    crate::ui::format::format_number(self.paths.len() as u64, cx.i18n().language()),
                );
                cx.i18n()
                    .text_with_args(Message::DragItems, &args)
                    .to_string()
            })
        };

        // Root is 0×0; the visible ghost is absolutely positioned at cursor + (12, 4).
        div().relative().child(
            div()
                .absolute()
                .left(self.cursor_offset.x + rem_pixels(rems(0.75)))
                .top(self.cursor_offset.y + rem_pixels(rems(0.25)))
                .px(rems(0.625))
                .py(rems(0.5))
                .rounded_md()
                .bg(theme.surface)
                .border(rems(0.0625))
                .border_color(theme.accent)
                .flex()
                .flex_col()
                .gap(rems(0.375))
                .whitespace_nowrap()
                .child(
                    // File icon + name row
                    div()
                        .flex()
                        .items_center()
                        .gap(rems(0.375))
                        .child(
                            svg()
                                .path(icon_path)
                                .size(rems(0.875))
                                .flex_none()
                                .text_color(theme.accent),
                        )
                        .child(
                            div()
                                .text_color(theme.text)
                                .text_size(rems(0.75))
                                .font_weight(FontWeight::MEDIUM)
                                .child(name_label),
                        ),
                )
                .child(
                    // Operation pills row
                    div()
                        .flex()
                        .gap(rems(0.25))
                        .child(drag_op_pill(
                            "+Ctrl",
                            cx.i18n().text(Message::DragCopy).as_ref(),
                            is_copy,
                            &theme,
                        ))
                        .child(drag_op_pill(
                            "+Shift",
                            cx.i18n().text(Message::DragMove).as_ref(),
                            !is_copy,
                            &theme,
                        )),
                ),
        )
    }
}

pub struct OxydeApp {
    pub state: WorkspaceState,
    pub handle: Option<Entity<Self>>,
    pub(crate) next_file_request: u64,
    pub(crate) next_native_menu_request: u64,
    pub(crate) pending_file_requests: std::collections::HashMap<String, u64>,
    pub folder_has_subdirs: std::collections::HashMap<String, bool>,
    pub(crate) pending_sidebar_requests: std::collections::HashMap<String, u64>,
    pub history: std::sync::Arc<oxyde_lib::models::HistoryManager>,
    pub focus_handle: FocusHandle,
    pub(crate) sidebar_scroll: UniformListScrollHandle,
    pub(crate) sidebar_scroll_h: ScrollHandle,
    pub(crate) breadcrumb_scroll: UniformListScrollHandle,
    pub drag_target: Option<DragTarget>,
    pub drop_target_idx: Option<usize>,
    pub drop_target_pane: Option<String>,
    pub highlight_right_zone: bool,
    pub highlight_left_zone: bool,
    pub highlight_top_zone: bool,
    pub highlight_bottom_zone: bool,
    pub highlight_split_pane: Option<String>,
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
    pub pane_origins: std::collections::HashMap<String, Point<f32>>,
    pub list_bounds: std::collections::HashMap<String, Bounds<f32>>,
    pub scrollbar_origins: std::collections::HashMap<String, Point<f32>>,
    pub breadcrumb_menu: Option<(String, String, usize, Vec<FileEntry>)>, // (pane_id, path, index, dirs)
    pub context_menus: Vec<ContextMenuInstance>,
    pub(crate) context_anchor_points: std::collections::HashMap<String, Point<Pixels>>,
    pub submenu_timer: Option<Task<()>>,
    pub submenu_close_timer: Option<Task<()>>,
    pub dragging_tab: Option<(String, usize)>,
    pub mouse_pos: Point<Pixels>,
    pub file_ops: std::sync::Arc<oxyde_lib::systems::file_ops::FileOperationManager>,
    pub watcher_changes: std::sync::Arc<std::sync::Mutex<std::collections::VecDeque<String>>>,
    pub _path_watchers: std::collections::HashMap<String, drive_watchers::DirectoryWatcher>,
    pub(super) pending_watchers: std::collections::HashMap<String, (u64, bool)>,
    pub(super) next_watcher_request: u64,
    pub(super) drive_metadata_ready: bool,
    pub(super) drive_refresh_epoch: u64,
    pub _watcher_task: Option<Task<()>>,
    pub _drive_refresh_task: Option<Task<()>>,
    pub(super) drive_details_task: Option<Task<()>>,
    pub(crate) ejecting_drive: bool,
    pub(crate) mounted_disk_images: Vec<oxyde_lib::platform::MountedDiskImage>,
    pub(super) disk_image_refresh_task: Option<Task<()>>,
    pub(crate) network_dialog: Option<network::NetworkDialog>,
    pub(crate) network_dialog_revision: u64,
    pub(crate) network_force_confirmation: Option<network::NetworkForceConfirmation>,
    pub(crate) network_force_focus_pending: bool,
    pub(crate) network_focus_restore: bool,
    pub(crate) network_notice: Option<(Message, u64)>,
    pub _subscriptions: Vec<Subscription>,
    /// Effective drag operation: true = copy, false = move.
    /// Derived from `drag_modifier_copy` (if Some) or `drag_target_default_copy` otherwise.
    pub drag_copy_mode: bool,
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
    pub breadcrumb_editing: bool,
    pub breadcrumb_text: String,
    pub breadcrumb_pane_id: Option<String>,
    pub breadcrumb_shake_id: u64,
    pub breadcrumb_invalid: bool,
    rename_click_candidate: Option<(String, std::time::Instant)>,
    rename_pending: bool,
    rename_cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
    rename_generation: u64,
    pending_file_operations: std::collections::VecDeque<PendingFileOperation>,
    file_operation_notice: Option<(Message, String)>,
    pub(crate) operations_panel_open: bool,
    pub(crate) operations_panel_opened_manually: bool,
    pub(crate) operations_panel_position: Option<Point<Pixels>>,
    pub(crate) operations_panel_view:
        Option<Entity<crate::ui::components::operations::OperationsPanelView>>,
    pub(crate) operations_panel_scroll: ScrollHandle,
    pub(crate) archive_creation: Option<(Vec<std::path::PathBuf>, String)>,
    pub(crate) rename_error: Option<Message>,
    pub(crate) rename_shake_id: u64,

    // ── Settings ─────────────────────────────────────────────────────────────
    pub show_hidden: bool,
    pub show_system: bool,
    pub show_media_previews: bool,
    pub show_grid_thumbnails: bool,
    pub settings_open: bool,
    pub(crate) settings_button_bounds: Option<Bounds<Pixels>>,
    pub(crate) tools_button_bounds: Option<Bounds<Pixels>>,
    pub(crate) settings_submenu_bounds: Option<Bounds<Pixels>>,
    pub(crate) settings_picker: Option<crate::ui::components::settings::SettingsPicker>,
    pub(crate) theme_id: crate::ui::theme::ThemeId,
    pub(crate) language: crate::ui::i18n::Language,
    pub(crate) settings_error: Option<Message>,
    config: oxyde_lib::models::AppConfig,
    config_path: Option<std::path::PathBuf>,
    preferences_load_failed: bool,
    preferences_save_generation: u64,
    preferences_save_task: Option<Task<()>>,
}

impl OxydeApp {
    pub(crate) fn request_native_context_menu(
        &mut self,
        parent_menu_idx: usize,
        path: String,
        is_background: bool,
        position: Point<Pixels>,
        open_left: bool,
        cx: &mut Context<Self>,
    ) {
        let Some(handle) = self.handle.clone() else {
            return;
        };
        if parent_menu_idx >= self.context_menus.len() {
            return;
        }
        if let Some(child) = self.context_menus.get(parent_menu_idx + 1) {
            if matches!(
                &child.content,
                ContextMenuContent::NativeItems {
                    path: child_path,
                    is_background: child_background,
                    ..
                } if child_path == &path && *child_background == is_background
            ) && (child.native_request_id.is_some()
                || matches!(&child.content, ContextMenuContent::NativeItems { items, .. } if !items.is_empty()))
            {
                return;
            }
        }

        self.next_native_menu_request = self.next_native_menu_request.wrapping_add(1);
        let request_id = self.next_native_menu_request;
        self.context_menus.truncate(parent_menu_idx + 1);
        self.context_menus.push(ContextMenuInstance {
            position,
            content: ContextMenuContent::NativeItems {
                path: path.clone(),
                is_background,
                items: vec![oxyde_lib::models::WinMenuItem {
                    id: -1,
                    label: cx.i18n().text(Message::NativeMenuLoading).to_string(),
                    verb: None,
                    has_submenu: false,
                    children: Vec::new(),
                }],
            },
            scroll_handle: UniformListScrollHandle::new(),
            width: px(0.0),
            bottom_padding: 0.0,
            open_left,
            hovered_item: None,
            native_request_id: Some(request_id),
        });
        cx.notify();

        let language = cx.i18n().language();
        cx.spawn(move |_, cx: &mut AsyncApp| {
            let mut cx = cx.clone();
            async move {
                let path_for_worker = path.clone();
                let result = tokio::task::spawn_blocking(move || {
                    oxyde_lib::platform::platform()
                        .native_context_menu_items(&path_for_worker, is_background)
                })
                .await;
                let i18n = I18n::for_language(language);
                let result = match result {
                    Ok(Ok(items)) if !items.is_empty() => Ok(items),
                    Ok(Ok(_)) => Err(i18n.text(Message::NativeMenuEmpty).to_string()),
                    Ok(Err(error)) => {
                        let mut args = FluentArgs::new();
                        args.set("detail", error.to_string());
                        Err(i18n
                            .text_with_args(Message::NativeMenuFailed, &args)
                            .to_string())
                    }
                    Err(error) => {
                        let mut args = FluentArgs::new();
                        args.set("detail", error.to_string());
                        Err(i18n
                            .text_with_args(Message::NativeMenuFailed, &args)
                            .to_string())
                    }
                };
                handle.update(&mut cx, move |this, cx| {
                    let Some(menu) = this.context_menus.get_mut(parent_menu_idx + 1) else {
                        return;
                    };
                    if menu.native_request_id != Some(request_id) {
                        return;
                    }
                    let ContextMenuContent::NativeItems {
                        path: current_path,
                        is_background: current_background,
                        items,
                    } = &mut menu.content
                    else {
                        return;
                    };
                    if current_path != &path || *current_background != is_background {
                        return;
                    }
                    match result {
                        Ok(native_items) => *items = native_items,
                        Err(message) => {
                            *items = vec![oxyde_lib::models::WinMenuItem {
                                id: -1,
                                label: message,
                                verb: None,
                                has_submenu: false,
                                children: Vec::new(),
                            }];
                        }
                    }
                    menu.native_request_id = None;
                    cx.notify();
                });
            }
        })
        .detach();
    }

    pub(crate) fn execute_native_context_menu_item(
        &mut self,
        path: String,
        id: i32,
        is_background: bool,
        creates_item: bool,
        cx: &mut Context<Self>,
    ) {
        let Some(handle) = self.handle.clone() else {
            return;
        };
        let tab_id = self
            .state
            .layout
            .find_pane(&self.state.active_pane_id)
            .map(|pane| pane.active_tab().id.clone());
        cx.spawn(move |_, cx: &mut AsyncApp| {
            let mut cx = cx.clone();
            async move {
                let result = tokio::task::spawn_blocking(move || {
                    let before = if creates_item {
                        std::fs::read_dir(&path).ok().map(|entries| {
                            entries
                                .filter_map(Result::ok)
                                .map(|entry| entry.path())
                                .collect::<std::collections::HashSet<_>>()
                        })
                    } else {
                        None
                    };
                    oxyde_lib::platform::platform().execute_native_context_menu_item(
                        &path,
                        id,
                        is_background,
                    )?;
                    if let Some(before) = before {
                        for _ in 0..20 {
                            let added: Vec<_> = std::fs::read_dir(&path)
                                .map_err(|error| error.to_string())?
                                .filter_map(Result::ok)
                                .filter(|entry| !before.contains(&entry.path()))
                                .collect();
                            if added.len() == 1 {
                                let entry = &added[0];
                                return Ok(Some((
                                    entry.path().to_string_lossy().into_owned(),
                                    entry
                                        .file_type()
                                        .map_err(|error| error.to_string())?
                                        .is_dir(),
                                )));
                            }
                            if added.len() > 1 {
                                break;
                            }
                            std::thread::sleep(std::time::Duration::from_millis(100));
                        }
                    }
                    Ok::<_, String>(None)
                })
                .await;
                match result {
                    Ok(Ok(Some((path, is_dir)))) => {
                        handle.update(&mut cx, |this, cx| {
                            if let Some(tab_id) = tab_id {
                                this.queue_created_item_rename(tab_id, path, is_dir, cx);
                            }
                        });
                    }
                    Ok(Ok(None)) => {}
                    Ok(Err(error)) => log::warn!("Native context-menu action failed: {error:?}"),
                    Err(error) => log::warn!("Native context-menu worker failed: {error}"),
                }
                handle.update(&mut cx, |_, cx| cx.notify());
            }
        })
        .detach();
    }

    pub fn bind_keys(cx: &mut App) {
        cx.bind_keys([
            KeyBinding::new("f5", Actualiser, None),
            KeyBinding::new("ctrl-a", ToutSelectionner, None),
            KeyBinding::new("ctrl-c", Copier, None),
            KeyBinding::new("ctrl-x", Couper, None),
            KeyBinding::new("ctrl-v", Coller, None),
            KeyBinding::new("ctrl-z", Annuler, None),
            KeyBinding::new("ctrl-y", Refaire, None),
            KeyBinding::new("ctrl-shift-z", Refaire, None),
        ]);
    }

    pub fn execute_undo(&mut self, cx: &mut Context<Self>) {
        let history = self.history.clone();
        let Some(handle) = self.handle.clone() else {
            return;
        };
        cx.spawn(move |_, cx: &mut AsyncApp| {
            let mut cx = cx.clone();
            async move {
                let runtime = tokio::runtime::Handle::current();
                let result = tokio::task::spawn_blocking(move || {
                    runtime.block_on(oxyde_lib::commands::ops::undo_last_action(&history))
                })
                .await;
                handle.update(&mut cx, |this, cx| {
                    if let Ok(Err(error)) = &result {
                        log::warn!("Undo failed: {error:?}");
                        this.file_operation_notice =
                            Some((Message::UndoFailed, format!("{error:?}")));
                    } else if let Err(error) = &result {
                        this.file_operation_notice =
                            Some((Message::UndoFailed, format!("{error}")));
                    }
                    cx.notify();
                });
            }
        })
        .detach();
    }

    pub(crate) fn persist_operations_panel_position(
        &mut self,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        self.operations_panel_position = Some(position);
        self.config.operations_panel_position = Some([position.x.as_f32(), position.y.as_f32()]);
        self.save_preferences(cx);
    }

    pub fn execute_redo(&mut self, cx: &mut Context<Self>) {
        let history = self.history.clone();
        let Some(handle) = self.handle.clone() else {
            return;
        };
        cx.spawn(move |_, cx: &mut AsyncApp| {
            let mut cx = cx.clone();
            async move {
                let runtime = tokio::runtime::Handle::current();
                let result = tokio::task::spawn_blocking(move || {
                    runtime.block_on(oxyde_lib::commands::ops::redo_last_action(&history))
                })
                .await;
                handle.update(&mut cx, |this, cx| {
                    if let Ok(Err(error)) = &result {
                        log::warn!("Redo failed: {error:?}");
                        this.file_operation_notice =
                            Some((Message::RedoFailed, format!("{error:?}")));
                    } else if let Err(error) = &result {
                        this.file_operation_notice =
                            Some((Message::RedoFailed, format!("{error}")));
                    }
                    cx.notify();
                });
            }
        })
        .detach();
    }

    pub fn new(
        config: oxyde_lib::models::AppConfig,
        config_path: Option<std::path::PathBuf>,
        load_failed: bool,
        cx: &mut Context<Self>,
    ) -> Self {
        let restored_workspace = config
            .extra
            .get(crate::ui::workspace::WORKSPACE_CONFIG_KEY)
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
        let operations_panel_position = config
            .operations_panel_position
            .filter(|position| position[0].is_finite() && position[1].is_finite())
            .map(|position| point(px(position[0]), px(position[1])));

        Self {
            state,
            handle: None,
            next_file_request: 0,
            next_native_menu_request: 0,
            pending_file_requests: std::collections::HashMap::new(),
            folder_has_subdirs: std::collections::HashMap::new(),
            pending_sidebar_requests: std::collections::HashMap::new(),
            history: std::sync::Arc::new(oxyde_lib::models::HistoryManager::new()),
            focus_handle: cx.focus_handle(),
            sidebar_scroll: UniformListScrollHandle::new(),
            sidebar_scroll_h: ScrollHandle::new(),
            breadcrumb_scroll: UniformListScrollHandle::new(),
            drag_target: None,
            drop_target_idx: None,
            drop_target_pane: None,
            highlight_right_zone: false,
            highlight_left_zone: false,
            highlight_top_zone: false,
            highlight_bottom_zone: false,
            highlight_split_pane: None,
            grab_offset: 0.0,
            sidebar_width,
            sidebar_collapsed: config
                .extra
                .get(crate::ui::workspace::WORKSPACE_CONFIG_KEY)
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
            pane_origins: std::collections::HashMap::new(),
            list_bounds: std::collections::HashMap::new(),
            scrollbar_origins: std::collections::HashMap::new(),
            breadcrumb_menu: None,
            context_menus: Vec::new(),
            context_anchor_points: std::collections::HashMap::new(),
            submenu_timer: None,
            submenu_close_timer: None,
            dragging_tab: None,
            mouse_pos: Point::default(),
            file_ops: std::sync::Arc::new(oxyde_lib::systems::file_ops::FileOperationManager::new()),
            watcher_changes: std::sync::Arc::new(std::sync::Mutex::new(
                std::collections::VecDeque::new(),
            )),
            _path_watchers: std::collections::HashMap::new(),
            pending_watchers: std::collections::HashMap::new(),
            next_watcher_request: 0,
            drive_metadata_ready: false,
            drive_refresh_epoch: 0,
            _watcher_task: None,
            _drive_refresh_task: None,
            drive_details_task: None,
            ejecting_drive: false,
            mounted_disk_images: Vec::new(),
            disk_image_refresh_task: None,
            network_dialog: None,
            network_dialog_revision: 0,
            network_force_confirmation: None,
            network_force_focus_pending: false,
            network_focus_restore: false,
            network_notice: None,
            _subscriptions: vec![cx.on_app_quit(|this, _| {
                crate::trace_shutdown_request("GPUI quit hook");
                crate::trace_shutdown_stage("history cleanup started");
                // Undo history is session-only, so its replacement backups must
                // not outlive a normal application shutdown.
                this.history.clear();
                crate::trace_shutdown_stage("history cleanup completed");
                let save = this.preferences_save_task.take();
                async move {
                    if let Some(save) = save {
                        crate::trace_shutdown_stage("waiting for preference saves");
                        save.await;
                    }
                    crate::trace_shutdown_stage("GPUI quit hook completed");
                }
            })],
            drag_copy_mode: false,
            drag_modifier_copy: None,
            drag_target_default_copy: false,
            editing_path: None,
            editing_text: String::new(),
            editing_view: None,
            inline_rename: false,
            created_item_rename: None,
            breadcrumb_editing: false,
            breadcrumb_text: String::new(),
            breadcrumb_pane_id: None,
            breadcrumb_shake_id: 0,
            breadcrumb_invalid: false,
            rename_click_candidate: None,
            rename_pending: false,
            rename_cancel: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
            rename_generation: 0,
            pending_file_operations: std::collections::VecDeque::new(),
            file_operation_notice: None,
            operations_panel_open: false,
            operations_panel_opened_manually: false,
            operations_panel_position,
            operations_panel_view: None,
            operations_panel_scroll: ScrollHandle::new(),
            archive_creation: None,
            rename_error: None,
            rename_shake_id: 0,
            show_hidden: config.show_hidden,
            show_system: config.show_system,
            show_media_previews: config.show_previews,
            show_grid_thumbnails: config.show_grid_thumbnails,
            settings_open: false,
            settings_button_bounds: None,
            tools_button_bounds: None,
            settings_submenu_bounds: None,
            settings_picker: None,
            theme_id: crate::ui::theme::ThemeId::from_id(&config.theme),
            language: crate::ui::i18n::Language::from_id(&config.language),
            settings_error: load_failed.then_some(Message::SettingsLoadError),
            config,
            config_path,
            preferences_load_failed: load_failed,
            preferences_save_generation: 0,
            preferences_save_task: None,
        }
    }

    pub fn on_action_tout_selectionner(
        &mut self,
        _: &ToutSelectionner,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.execute_tout_selectionner(cx);
    }

    pub fn execute_tout_selectionner(&mut self, cx: &mut Context<Self>) {
        let pane_id = self.state.active_pane_id.clone();
        self.select_all(&pane_id, cx);
    }

    pub fn on_action_annuler(&mut self, _: &Annuler, _: &mut Window, cx: &mut Context<Self>) {
        self.execute_undo(cx);
    }

    pub fn on_action_refaire(&mut self, _: &Refaire, _: &mut Window, cx: &mut Context<Self>) {
        self.execute_redo(cx);
    }

    pub fn on_action_copier(&mut self, _: &Copier, _: &mut Window, cx: &mut Context<Self>) {
        self.execute_copier(cx);
    }

    pub fn on_action_renommer(
        &mut self,
        _: &Renommer,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.execute_renommer(window, cx);
    }

    pub fn execute_renommer(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let pane_id = self.state.active_pane_id.clone();
        let selected = self.state.layout.find_pane(&pane_id).and_then(|pane| {
            let tab = pane.active_tab();
            tab.selected_paths.iter().next().cloned().map(|path| {
                let is_dir = tab
                    .files
                    .iter()
                    .any(|file| file.path == path && file.is_dir);
                (path, is_dir)
            })
        });
        if let Some((path, is_dir)) = selected {
            self.begin_rename(path, is_dir, false, window, cx);
        }
    }

    pub(crate) fn handle_slow_rename_click(
        &mut self,
        path: String,
        is_dir: bool,
        click_count: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let now = std::time::Instant::now();
        let previous = self.rename_click_candidate.take();
        let is_same_selected_item = self
            .state
            .layout
            .find_pane(&self.state.active_pane_id)
            .map(|pane| pane.active_tab().selected_paths.contains(&path))
            .unwrap_or(false);
        let is_slow_second_click = previous
            .as_ref()
            .map(|(previous_path, previous_at)| {
                previous_path == &path
                    && now.duration_since(*previous_at) >= std::time::Duration::from_millis(250)
                    && now.duration_since(*previous_at) <= std::time::Duration::from_secs(2)
            })
            .unwrap_or(false);

        if self.inline_rename || self.editing_path.is_some() {
            return;
        }

        if is_same_selected_item && is_slow_second_click {
            self.begin_rename(path, is_dir, true, window, cx);
        } else if click_count <= 1 {
            self.rename_click_candidate = Some((path, now));
        }
    }

    fn begin_rename(
        &mut self,
        path: String,
        is_dir: bool,
        inline: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Rename and breadcrumb editing share the same input entity.
        if self.breadcrumb_editing {
            self.cancel_breadcrumb_edit(cx);
        }
        let initial_name = std::path::Path::new(&path)
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("")
            .to_string();
        let selected_len = if is_dir {
            initial_name.len()
        } else {
            std::path::Path::new(&initial_name)
                .file_stem()
                .and_then(|stem| stem.to_str())
                .map(str::len)
                .unwrap_or(initial_name.len())
        };
        let initial_selection = Some(0..selected_len);
        let Some(handle) = self.handle.clone() else {
            return;
        };

        self.archive_creation = None;
        self.rename_cancel
            .store(true, std::sync::atomic::Ordering::Relaxed);
        self.rename_cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        self.rename_generation = self.rename_generation.wrapping_add(1);
        self.rename_pending = false;
        self.rename_click_candidate = None;
        self.editing_path = Some(path);
        self.editing_text = initial_name.clone();
        self.rename_error = None;
        self.inline_rename = inline;

        let view = cx.new(|cx| {
            TextInput::new(
                cx,
                cx.i18n().text(Message::RenameNamePlaceholder),
                initial_name,
                {
                    let handle = handle.clone();
                    move |text, _window, cx| {
                        handle.update(cx, |this, cx| {
                            this.editing_text = text.to_string();
                            this.rename_error = None;
                            cx.notify();
                        });
                    }
                },
                {
                    let handle = handle.clone();
                    move |_window, cx| {
                        handle.update(cx, |this, cx| this.submit_rename(cx));
                    }
                },
                {
                    let handle = handle.clone();
                    move |_window, cx| {
                        handle.update(cx, |this, cx| this.cancel_rename(cx));
                    }
                },
                {
                    let handle = handle.clone();
                    move |input, has_selection, position, window, cx| {
                        handle.update(cx, |this, cx| {
                            this.open_text_context_menu(input, has_selection, position, window, cx)
                        });
                    }
                },
                true,
                false,
                inline,
                initial_selection,
                window,
            )
        });

        let focus_handle = view.read(cx).focus_handle.clone();
        self.editing_view = Some(view);
        window.focus(&focus_handle, cx);
        cx.notify();
    }

    pub(crate) fn begin_breadcrumb_edit(
        &mut self,
        pane_id: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.inline_rename || self.editing_path.is_some() {
            return;
        }
        let Some(path) = self
            .state
            .layout
            .find_pane(&pane_id)
            .map(|pane| pane.active_tab().path.clone())
        else {
            return;
        };
        let Some(handle) = self.handle.clone() else {
            return;
        };
        self.breadcrumb_editing = true;
        let path = crate::ui::components::input::native_path_text(&path);
        self.breadcrumb_text = path.clone();
        self.breadcrumb_invalid = false;
        self.breadcrumb_pane_id = Some(pane_id);
        let input = cx.new(|cx| {
            TextInput::new(
                cx,
                cx.i18n().text(Message::PathPlaceholder).to_string(),
                path.clone(),
                {
                    let handle = handle.clone();
                    move |text, _, cx| {
                        handle.update(cx, |this, cx| {
                            this.breadcrumb_text = text.to_string();
                            this.breadcrumb_invalid = false;
                            cx.notify();
                        })
                    }
                },
                {
                    let handle = handle.clone();
                    move |_, cx| handle.update(cx, |this, cx| this.submit_breadcrumb_edit(cx))
                },
                {
                    let handle = handle.clone();
                    move |_, cx| handle.update(cx, |this, cx| this.cancel_breadcrumb_edit(cx))
                },
                {
                    let handle = handle.clone();
                    move |input, has_selection, position, window, cx| {
                        handle.update(cx, |this, cx| {
                            this.open_text_context_menu(input, has_selection, position, window, cx)
                        });
                    }
                },
                true,
                false,
                true,
                Some(0..path.len()),
                window,
            )
            .with_path_submit()
        });
        let focus = input.read(cx).focus_handle.clone();
        self.editing_view = Some(input);
        window.focus(&focus, cx);
        cx.on_next_frame(window, move |this, window, cx| {
            if this.breadcrumb_editing {
                window.focus(&focus, cx);
            }
        });
        cx.notify();
    }

    pub(crate) fn submit_breadcrumb_edit(&mut self, cx: &mut Context<Self>) {
        if !self.breadcrumb_editing || !self.context_menus.is_empty() {
            return;
        }
        let path = crate::ui::components::input::navigation_path_text(&self.breadcrumb_text);
        let network_server = cfg!(target_os = "windows")
            && oxyde_lib::commands::io::is_network_server(&path);
        if path != "__network_vincinity__" && !network_server && !std::path::Path::new(&path).is_dir() {
            self.breadcrumb_shake_id = self.breadcrumb_shake_id.wrapping_add(1);
            self.breadcrumb_invalid = true;
            cx.notify();
            return;
        }
        let pane_id = self.breadcrumb_pane_id.take();
        self.breadcrumb_editing = false;
        self.breadcrumb_invalid = false;
        self.editing_view = None;
        if let Some(pane_id) = pane_id {
            if !path.is_empty() {
                self.navigate_to(&pane_id, path, cx);
            }
        }
        cx.notify();
    }

    pub(crate) fn cancel_breadcrumb_edit(&mut self, cx: &mut Context<Self>) {
        self.breadcrumb_editing = false;
        self.breadcrumb_invalid = false;
        self.breadcrumb_pane_id = None;
        self.editing_view = None;
        self.context_menus.clear();
        cx.notify();
    }

    pub(crate) fn submit_rename(&mut self, cx: &mut Context<Self>) {
        if self.inline_rename && !self.context_menus.is_empty() {
            return;
        }
        if self.rename_pending {
            return;
        }
        let Some(path) = self.editing_path.clone() else {
            return;
        };
        if let Some((sources, extension)) = self.archive_creation.clone() {
            let name = self.editing_text.trim();
            if name.is_empty() {
                self.rename_error = Some(Message::RenameEmptyName);
                cx.notify();
                return;
            }
            if name == "."
                || name == ".."
                || name.contains(['/', '\\', ':', '*', '?', '"', '<', '>', '|'])
                || name.chars().any(char::is_control)
                || name.ends_with(['.', ' '])
            {
                self.rename_error = Some(Message::RenameInvalidName);
                cx.notify();
                return;
            }
            let suffix = format!(".{extension}");
            let name = if name.to_lowercase().ends_with(&suffix) {
                name.to_string()
            } else {
                format!("{name}{suffix}")
            };
            let destination = std::path::Path::new(&path)
                .parent()
                .unwrap_or(std::path::Path::new(""))
                .join(name);
            self.cancel_rename(cx);
            self.queue_archive_operation(
                oxyde_lib::systems::file_ops::FileOpType::Compress,
                sources,
                destination,
                cx,
            );
            return;
        }
        let current_name = std::path::Path::new(&path)
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("");
        if self.editing_text == current_name {
            self.cancel_rename(cx);
            return;
        }
        if self.editing_text.trim().is_empty() {
            self.rename_error = Some(Message::RenameEmptyName);
            self.rename_shake_id = self.rename_shake_id.wrapping_add(1);
            cx.notify();
            return;
        }
        self.rename_pending = true;
        self.execute_file_rename(path, self.editing_text.clone(), cx);
    }

    pub(crate) fn cancel_rename(&mut self, cx: &mut Context<Self>) {
        self.rename_cancel
            .store(true, std::sync::atomic::Ordering::Relaxed);
        self.archive_creation = None;
        self.rename_generation = self.rename_generation.wrapping_add(1);
        self.editing_path = None;
        self.editing_view = None;
        self.inline_rename = false;
        self.rename_pending = false;
        self.rename_error = None;
        cx.notify();
    }

    pub(crate) fn open_text_context_menu(
        &mut self,
        input: Entity<TextInput>,
        has_selection: bool,
        position: Point<Pixels>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        use crate::ui::components::dropdown::{DropdownItem, IconName};

        let Some(handle) = self.handle.clone() else {
            return;
        };
        let can_paste = cx
            .read_from_clipboard()
            .and_then(|clipboard| clipboard.text())
            .is_some();

        let mut items = Vec::new();
        for (label, command, is_dimmed) in [
            (Message::Cut, 0u8, !has_selection),
            (Message::Copy, 1u8, !has_selection),
            (Message::Paste, 2u8, !can_paste),
            (Message::SelectAll, 3u8, false),
        ] {
            if command == 3 {
                items.push(DropdownItem::separator());
            }
            let h = handle.clone();
            let target_input = input.clone();
            let mut item =
                DropdownItem::new(cx.i18n().text(label).to_string(), move |window, app| {
                    target_input.update(app, |input, cx| match command {
                        0 => input.cut_selected_text(window, cx),
                        1 => input.copy_selected_text(cx),
                        2 => input.paste_from_clipboard(window, cx),
                        _ => input.select_all_text(cx),
                    });
                    h.update(app, |_this, cx| {
                        cx.notify();
                    });
                });
            item.icon = match command {
                1 => Some(IconName::Copy),
                3 => Some(IconName::ALargeSmall),
                _ => None,
            };
            item.is_dimmed = is_dimmed;
            items.push(item);
        }

        self.context_menus.clear();
        self.context_menus.push(ContextMenuInstance {
            position,
            content: ContextMenuContent::Items(items, None),
            scroll_handle: UniformListScrollHandle::new(),
            width: px(0.0),
            bottom_padding: 0.0,
            open_left: false,
            hovered_item: None,
            native_request_id: None,
        });
        cx.notify();
    }

    pub fn execute_file_rename(
        &mut self,
        old_path: String,
        new_name: String,
        cx: &mut Context<Self>,
    ) {
        let parent = std::path::Path::new(&old_path)
            .parent()
            .unwrap_or(std::path::Path::new(""));
        let new_path = parent.join(new_name).to_string_lossy().to_string();

        let Some(handle) = self.handle.clone() else {
            self.rename_pending = false;
            return;
        };
        let quality = self.archive_quality_for_path(std::path::Path::new(&old_path));
        let cancel = self.rename_cancel.clone();
        let generation = self.rename_generation;
        let path_for_check = old_path.clone();
        cx.spawn(move |_, cx: &mut AsyncApp| {
            let mut cx = cx.clone();
            async move {
                let runtime = tokio::runtime::Handle::current();
                let result = tokio::task::spawn_blocking(move || {
                    runtime.block_on(oxyde_lib::commands::io::rename_item_controlled(
                        old_path, new_path, quality, cancel,
                    ))
                })
                .await;
                handle.update(&mut cx, |this, cx| {
                    let current_attempt = this.rename_generation == generation
                        && this.editing_path.as_deref() == Some(path_for_check.as_str());
                    if let Ok(Ok(())) = &result {
                        this.reconcile_watchers(cx);
                    }
                    if !current_attempt {
                        return;
                    }
                    this.rename_pending = false;
                    match result {
                        Ok(Ok(())) => {
                            this.editing_path = None;
                            this.editing_view = None;
                            this.inline_rename = false;
                            this.rename_error = None;
                            this.refresh_all_tabs(cx);
                            cx.notify();
                        }
                        Ok(Err(error)) => {
                            log::warn!("Rename failed: {error:?}");
                            this.rename_error = Some(crate::ui::errors::rename_message(&error));
                            this.rename_shake_id = this.rename_shake_id.wrapping_add(1);
                            cx.notify();
                        }
                        Err(error) => {
                            log::warn!("Rename task failed: {error}");
                            this.rename_error = Some(Message::RenameTaskFailed);
                            this.rename_shake_id = this.rename_shake_id.wrapping_add(1);
                            cx.notify();
                        }
                    }
                });
            }
        })
        .detach();
    }

    pub fn execute_copier(&mut self, _cx: &mut Context<Self>) {
        let pane_id = self.state.active_pane_id.clone();
        if let Some(pane) = self.state.layout.find_pane(&pane_id) {
            let paths: Vec<String> = pane.active_tab().selected_paths.iter().cloned().collect();
            log::info!("Action 'Copier': paths={:?}", paths);
            if !paths.is_empty() {
                if let Err(e) = oxyde_lib::platform::platform().set_clipboard_files(paths, false) {
                    log::error!("Action 'Copier' failed: {:?}", e);
                }
            }
        }
    }

    pub fn on_action_couper(&mut self, _: &Couper, _: &mut Window, cx: &mut Context<Self>) {
        self.execute_couper(cx);
    }

    pub fn execute_couper(&mut self, _cx: &mut Context<Self>) {
        let pane_id = self.state.active_pane_id.clone();
        if let Some(pane) = self.state.layout.find_pane(&pane_id) {
            let paths: Vec<String> = pane.active_tab().selected_paths.iter().cloned().collect();
            log::info!("Action 'Couper': paths={:?}", paths);
            if !paths.is_empty() {
                if let Err(e) = oxyde_lib::platform::platform().set_clipboard_files(paths, true) {
                    log::error!("Action 'Couper' failed: {:?}", e);
                }
            }
        }
    }

    pub fn on_action_coller(&mut self, _: &Coller, _: &mut Window, cx: &mut Context<Self>) {
        self.execute_coller(cx);
    }

    pub fn execute_coller(&mut self, cx: &mut Context<Self>) {
        let pane_id = self.state.active_pane_id.clone();
        if let Some(pane) = self.state.layout.find_pane(&pane_id) {
            let target_dir = pane.active_tab().path.clone();
            if target_dir.starts_with("search://") { return; }
            if let Ok((paths, is_cut)) = oxyde_lib::platform::platform().get_clipboard_files() {
                if !paths.is_empty() {
                    if is_cut {
                        self.execute_file_move(paths, target_dir, cx);
                    } else {
                        self.execute_file_copy(paths, target_dir, cx);
                    }
                }
            }
        }
    }

    pub fn on_action_ouvrir(&mut self, _: &Ouvrir, _: &mut Window, cx: &mut Context<Self>) {
        self.execute_ouvrir(cx);
    }

    /// Shell handlers may pump Windows messages. Never invoke them while GPUI
    /// holds the App/Entity borrow for a mouse or keyboard callback.
    pub(crate) fn open_external_item(&mut self, path: String, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            let result = tokio::task::spawn_blocking(move || {
                let path = oxyde_lib::commands::archive::prepare_archive_file_for_open(
                    std::path::Path::new(&path),
                )
                .map_err(|e| e.to_string())?;
                oxyde_lib::platform::platform().open_item(&path.to_string_lossy())
            })
            .await;
            let error = match result {
                Ok(Ok(())) => return,
                Ok(Err(error)) => error,
                Err(error) => error.to_string(),
            };
            let _ = this.update(cx, |this, cx| {
                this.file_operation_notice = Some((Message::OpenItemFailed, error));
                cx.notify();
            });
        }).detach();
    }

    fn archive_qualities(&self) -> std::collections::HashMap<String, String> {
        std::collections::HashMap::from([
            ("zip".into(), self.config.zip_quality.clone()),
            ("7z".into(), self.config.seven_zip_quality.clone()),
            ("zst".into(), self.config.zstd_quality.clone()),
        ])
    }

    fn archive_quality_for_path(&self, path: &std::path::Path) -> String {
        let archive = oxyde_lib::utils::archive::split_virtual_path(&path.to_string_lossy())
            .map(|(archive, _)| archive)
            .unwrap_or_else(|| path.to_path_buf());
        match archive
            .extension()
            .and_then(|e| e.to_str())
            .map(str::to_ascii_lowercase)
            .as_deref()
        {
            Some("7z") => self.config.seven_zip_quality.clone(),
            Some("zst") => self.config.zstd_quality.clone(),
            _ => self.config.zip_quality.clone(),
        }
    }

    pub(crate) fn open_file_item(
        &mut self,
        pane_id: &str,
        path: String,
        is_dir: bool,
        cx: &mut Context<Self>,
    ) {
        let navigable_archive =
            oxyde_lib::utils::archive::is_navigable_archive(std::path::Path::new(&path));
        if oxyde_lib::utils::disk_image::needs_mount_for_browsing(std::path::Path::new(&path)) {
            self.open_disk_image(pane_id.to_string(), path, false, cx);
        } else if is_dir || navigable_archive {
            self.open_directory_item(pane_id, path, false, cx);
        } else {
            self.open_external_item(path, cx);
        }
    }

    pub fn execute_ouvrir(&mut self, cx: &mut Context<Self>) {
        let pane_id = self.state.active_pane_id.clone();
        if let Some(pane) = self.state.layout.find_pane(&pane_id) {
            let tab = pane.active_tab();
            let paths: Vec<(String, bool)> = tab.files.iter()
                .filter(|entry| tab.selected_paths.contains(&entry.path))
                .map(|entry| (entry.path.clone(), entry.is_dir)).collect();
            for (path, is_dir) in paths {
                self.open_file_item(&pane_id, path, is_dir, cx);
            }
        }
    }

    pub fn on_action_supprimer(&mut self, _: &Supprimer, _: &mut Window, cx: &mut Context<Self>) {
        self.execute_supprimer(cx);
    }

    pub fn execute_supprimer(&mut self, cx: &mut Context<Self>) {
        let pane_id = self.state.active_pane_id.clone();
        if let Some(pane) = self.state.layout.find_pane(&pane_id) {
            let paths: Vec<String> = pane.active_tab().selected_paths.iter().cloned().collect();
            if paths.is_empty() {
                return;
            }
            let Some(handle) = self.handle.clone() else {
                return;
            };
            cx.spawn(move |_, cx: &mut AsyncApp| {
                let mut cx = cx.clone();
                async move {
                    let runtime = tokio::runtime::Handle::current();
                    handle.update(&mut cx, |this, cx| {
                        let mut op = oxyde_lib::systems::file_ops::FileOperation::new(
                            oxyde_lib::systems::file_ops::FileOpType::Trash,
                            paths.into_iter().map(std::path::PathBuf::from).collect(),
                            None,
                            None,
                        );
                        op.archive_qualities = this.archive_qualities();
                        let id =
                            this.file_ops
                                .queue_operation_on(&runtime, this.history.clone(), op);
                        this.watch_file_operation(id, cx);
                    });
                }
            })
            .detach();
        }
    }

    pub fn execute_compress_selected(
        &mut self,
        format: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(pane) = self.state.layout.find_pane(&self.state.active_pane_id) else {
            return;
        };
        let paths: Vec<std::path::PathBuf> = pane
            .active_tab()
            .selected_paths
            .iter()
            .map(std::path::PathBuf::from)
            .filter(|path| {
                oxyde_lib::utils::archive::split_virtual_path(&path.to_string_lossy()).is_none()
            })
            .collect();
        let Some(first) = paths.first() else {
            return;
        };
        let extension = if format == "zst" { "tar.zst" } else { format };
        let destination = first.with_extension(extension);
        self.begin_rename(
            destination.to_string_lossy().into_owned(),
            false,
            false,
            window,
            cx,
        );
        self.archive_creation = Some((paths, extension.to_string()));
    }

    pub fn execute_extract_archive(&mut self, path: String, cx: &mut Context<Self>) {
        let source = std::path::PathBuf::from(path);
        let Some(parent) = source.parent() else {
            return;
        };
        let name = source.file_name().unwrap_or_default().to_string_lossy();
        let stem = [".tar.gz", ".tar.xz", ".tar.zst", ".tar.bz2"]
            .iter()
            .find_map(|suffix| name.strip_suffix(suffix))
            .or_else(|| source.file_stem().and_then(|stem| stem.to_str()))
            .unwrap_or("archive");
        let destination = parent.join(stem);
        self.queue_archive_operation(
            oxyde_lib::systems::file_ops::FileOpType::Extract,
            vec![source],
            destination,
            cx,
        );
    }

    fn queue_archive_operation(
        &mut self,
        kind: oxyde_lib::systems::file_ops::FileOpType,
        sources: Vec<std::path::PathBuf>,
        destination: std::path::PathBuf,
        cx: &mut Context<Self>,
    ) {
        let Some(handle) = self.handle.clone() else {
            return;
        };
        cx.spawn(move |_, cx: &mut AsyncApp| {
            let mut cx = cx.clone();
            async move {
                let runtime = tokio::runtime::Handle::current();
                handle.update(&mut cx, |this, cx| {
                    let mut operation = oxyde_lib::systems::file_ops::FileOperation::new(
                        kind,
                        sources,
                        Some(destination),
                        None,
                    );
                    operation.compression_quality =
                        this.archive_quality_for_path(operation.destination.as_ref().unwrap());
                    operation.archive_qualities = this.archive_qualities();
                    let id =
                        this.file_ops
                            .queue_operation_on(&runtime, this.history.clone(), operation);
                    this.watch_file_operation(id, cx);
                });
            }
        })
        .detach();
    }

    /// End pointer interactions, including releases missed outside the window.
    pub(crate) fn finish_pointer_drag(&mut self, cx: &mut Context<Self>) {
        let mut needs_notify = false;
        let workspace_layout_changed = matches!(
            self.drag_target.as_ref(),
            Some(DragTarget::PaneResize { .. } | DragTarget::SidebarResize)
        );
        if self.drag_target.is_some() {
            self.drag_target = None;
            needs_notify = true;
        }
        if self.dragging_tab.is_some() {
            self.dragging_tab = None;
            needs_notify = true;
        }
        if self.drop_target_idx.is_some() || self.drop_target_pane.is_some() {
            self.drop_target_idx = None;
            self.drop_target_pane = None;
            needs_notify = true;
        }
        if self.highlight_split_pane.is_some() {
            self.highlight_split_pane = None;
            needs_notify = true;
        }
        if workspace_layout_changed {
            self.save_workspace(cx);
        }
        if needs_notify {
            cx.notify();
        }
    }

    /// Recompute `drag_copy_mode` from modifier override and drive-based default.
    /// Notifies if the value changed.
    pub fn update_drag_copy_mode(&mut self, cx: &mut Context<Self>) {
        let new_mode = self
            .drag_modifier_copy
            .unwrap_or(self.drag_target_default_copy);
        if self.drag_copy_mode != new_mode {
            self.drag_copy_mode = new_mode;
            cx.notify();
        }
    }

    pub fn execute_file_move(
        &mut self,
        paths: Vec<String>,
        target_dir: String,
        cx: &mut Context<Self>,
    ) {
        self.prepare_file_operation(paths, target_dir, true, cx);
    }

    pub fn execute_file_copy(
        &mut self,
        paths: Vec<String>,
        target_dir: String,
        cx: &mut Context<Self>,
    ) {
        self.prepare_file_operation(paths, target_dir, false, cx);
    }

    fn prepare_file_operation(
        &mut self,
        paths: Vec<String>,
        target_dir: String,
        is_move: bool,
        cx: &mut Context<Self>,
    ) {
        if paths.is_empty() {
            return;
        }
        let Some(handle) = self.handle.clone() else {
            return;
        };
        cx.spawn(move |_, cx: &mut AsyncApp| {
            let mut cx = cx.clone();
            async move {
                let check_paths = paths.clone();
                let check_destination = target_dir.clone();
                let runtime = tokio::runtime::Handle::current();
                let runtime_for_check = runtime.clone();
                let checked = tokio::task::spawn_blocking(move || {
                    runtime_for_check.block_on(oxyde_lib::commands::ops::check_conflicts(
                        check_paths,
                        check_destination,
                    ))
                })
                .await;
                handle.update(&mut cx, |this, cx| match checked {
                    Ok(Ok(response)) if !response.conflicts.is_empty() => {
                        this.pending_file_operations
                            .push_back(PendingFileOperation {
                                paths,
                                target_dir,
                                is_move,
                                conflict_paths: response
                                    .conflicts
                                    .into_iter()
                                    .map(|conflict| conflict.source.path)
                                    .collect(),
                                runtime,
                            });
                        cx.notify();
                    }
                    Ok(Ok(_)) => {
                        this.queue_file_operation(paths, target_dir, is_move, None, runtime, cx)
                    }
                    Ok(Err(error)) => {
                        this.file_operation_notice =
                            Some((Message::PrepareFailed, format!("{error:?}")));
                        cx.notify();
                    }
                    Err(error) => {
                        this.file_operation_notice =
                            Some((Message::PrepareFailed, format!("{error}")));
                        cx.notify();
                    }
                });
            }
        })
        .detach();
    }

    fn queue_file_operation(
        &mut self,
        paths: Vec<String>,
        target_dir: String,
        is_move: bool,
        resolutions: Option<std::collections::HashMap<String, oxyde_lib::models::ConflictAction>>,
        runtime: tokio::runtime::Handle,
        cx: &mut Context<Self>,
    ) {
        use oxyde_lib::systems::file_ops::{FileOpType, FileOperation};
        let mut op = FileOperation::new(
            if is_move {
                FileOpType::Move
            } else {
                FileOpType::Copy
            },
            paths.into_iter().map(std::path::PathBuf::from).collect(),
            Some(std::path::PathBuf::from(target_dir)),
            resolutions,
        );
        op.compression_quality = self.archive_quality_for_path(op.destination.as_ref().unwrap());
        op.archive_qualities = self.archive_qualities();
        let op_id = self
            .file_ops
            .queue_operation_on(&runtime, self.history.clone(), op);
        self.watch_file_operation(op_id, cx);
    }

    fn watch_file_operation(&mut self, op_id: String, cx: &mut Context<Self>) {
        use oxyde_lib::systems::file_ops::OpStatus;
        let operations = self.file_ops.clone();
        let Some(handle) = self.handle.clone() else {
            return;
        };
        cx.spawn(move |_, cx: &mut AsyncApp| {
            let mut cx = cx.clone();
            async move {
                let opened_at = std::time::Instant::now();
                let mut automatic_open_checked = false;
                loop {
                    cx.background_executor()
                        .timer(std::time::Duration::from_millis(200))
                        .await;
                    let Some(operation) = operations.get_operation(&op_id) else {
                        break;
                    };
                    if !automatic_open_checked
                        && opened_at.elapsed() >= std::time::Duration::from_millis(700)
                    {
                        automatic_open_checked = true;
                        if !matches!(
                            operation.status,
                            OpStatus::Completed | OpStatus::Cancelled | OpStatus::Error(_)
                        ) {
                            handle.update(&mut cx, |this, cx| {
                                this.operations_panel_open = true;
                                cx.notify();
                            });
                        }
                    }
                    match operation.status {
                        OpStatus::Queued
                        | OpStatus::Calculating
                        | OpStatus::Running
                        | OpStatus::Paused => {
                            handle.update(&mut cx, |_, cx| cx.notify());
                        }
                        OpStatus::Error(error) => {
                            handle.update(&mut cx, |this, cx| {
                                log::warn!("File operation failed: {error}");
                                this.operations_panel_open = true;
                                this.file_operation_notice = Some((Message::TransferFailed, error));
                                cx.notify();
                            });
                            break;
                        }
                        OpStatus::Completed => {
                            handle.update(&mut cx, |this, cx| {
                                this.refresh_all_tabs(cx);
                                this.state.refresh_sidebar();
                                this.reconcile_watchers(cx);
                                let operations = this.file_ops.list_operations();
                                if !this.operations_panel_opened_manually
                                    && !operations.is_empty()
                                    && operations.iter().all(|operation| {
                                        matches!(operation.status, OpStatus::Completed)
                                    })
                                {
                                    this.operations_panel_open = false;
                                }
                                cx.notify();
                            });
                            break;
                        }
                        OpStatus::Cancelled => {
                            handle.update(&mut cx, |_, cx| cx.notify());
                            break;
                        }
                    }
                }
            }
        })
        .detach();
    }

    pub(crate) fn cancel_pending_file_operation(&mut self, cx: &mut Context<Self>) {
        self.pending_file_operations.pop_front();
        cx.notify();
    }

    pub(crate) fn resolve_pending_file_operation(&mut self, replace: bool, cx: &mut Context<Self>) {
        use oxyde_lib::models::ConflictAction;
        let Some(pending) = self.pending_file_operations.pop_front() else {
            return;
        };
        let action = if replace {
            ConflictAction::Replace
        } else {
            ConflictAction::Skip
        };
        let resolutions = pending
            .conflict_paths
            .into_iter()
            .map(|path| (path, action))
            .collect();
        self.queue_file_operation(
            pending.paths,
            pending.target_dir,
            pending.is_move,
            Some(resolutions),
            pending.runtime,
            cx,
        );
        cx.notify();
    }

    pub fn create_new_folder(&mut self, pane_id: &str, cx: &mut Context<Self>) {
        use oxyde_lib::models::transaction::{Transaction, TransactionDetails, TransactionType};

        if let Some(pane) = self.state.layout.find_pane(pane_id) {
            let current_path = pane.active_tab().path.clone();
            if current_path.starts_with("search://") { return; }
            let language = cx.i18n().language();
            let quality = self.archive_quality_for_path(std::path::Path::new(&current_path));
            let tab_id = pane.active_tab().id.clone();
            let Some(handle) = self.handle.clone() else {
                return;
            };
            cx.spawn(move |_, cx: &mut AsyncApp| {
                let mut cx = cx.clone();
                async move {
                    let created = tokio::task::spawn_blocking({
                        let current_path = current_path.clone();
                        move || {
                            let i18n = I18n::for_language(language);
                            for counter in 0.. {
                                let name = if counter == 0 {
                                    i18n.text(Message::NewFolderName).to_string()
                                } else {
                                    let mut args = FluentArgs::new();
                                    args.set("number", counter);
                                    i18n.text_with_args(Message::NewFolderNumbered, &args)
                                        .replace(['\u{2068}', '\u{2069}'], "")
                                };
                                let path = std::path::Path::new(&current_path).join(name);
                                match oxyde_lib::commands::archive::create_archive_directory_with_quality(&path, &quality) {
                                    Ok(()) => return Ok(path),
                                    Err(error)
                                        if error.kind() == std::io::ErrorKind::AlreadyExists =>
                                    {
                                        continue;
                                    }
                                    Err(error) => return Err(error),
                                }
                            }
                            unreachable!()
                        }
                    })
                    .await;
                    handle.update(&mut cx, |this, cx| match created {
                        Ok(Ok(folder_path)) => {
                            this.queue_created_item_rename(
                                tab_id,
                                folder_path.to_string_lossy().into_owned(),
                                true,
                                cx,
                            );
                            if oxyde_lib::utils::archive::split_virtual_path(&current_path).is_none() {
                            this.history.push(Transaction::new(
                                TransactionType::NewFolder,
                                TransactionDetails {
                                    paths: vec![folder_path.to_string_lossy().into_owned()],
                                    target_dir: Some(current_path),
                                    old_path: None,
                                    new_path: None,
                                    created_files: None,
                                    created_dirs: None,
                                    replaced_files: None,
                                    overwritten_files: None,
                                    moved_files: None,
                                    moved_dirs: None,
                                    copied_files: None,
                                    copied_file_hashes: None,
                                },
                            ));
                            }
                            this.refresh_all_tabs(cx);
                            this.state.refresh_sidebar();
                            this.reconcile_watchers(cx);
                            cx.notify();
                        }
                        Ok(Err(error)) => log::warn!("Create folder failed: {error}"),
                        Err(error) => log::warn!("Create folder task failed: {error}"),
                    });
                }
            })
            .detach();
        }
    }

    fn queue_created_item_rename(
        &mut self,
        tab_id: String,
        path: String,
        is_dir: bool,
        cx: &mut Context<Self>,
    ) {
        let parent = std::path::Path::new(&path)
            .parent()
            .map(|path| path.to_string_lossy().into_owned());
        let mut target = None;
        self.state.layout.visit_panes(&mut |pane| {
            if let Some((index, tab)) = pane
                .tabs
                .iter()
                .enumerate()
                .find(|(_, tab)| tab.id == tab_id)
            {
                if Some(&tab.path) == parent.as_ref() && pane.active_tab().id == tab_id {
                    target = Some((pane.id.clone(), index, tab.path.clone()));
                }
            }
        });
        let Some((pane_id, index, directory)) = target else {
            return;
        };
        self.state.active_pane_id = pane_id.clone();
        self.created_item_rename = Some((tab_id, path, is_dir));
        self.request_tab_files(&pane_id, index, directory, false, cx);
    }

    /// Refresh the file list of a specific tab without disturbing its scroll / selection / filters.
    pub fn refresh_tab_files(
        &mut self,
        pane_id: &str,
        tab_idx: usize,
        path: String,
        cx: &mut Context<Self>,
    ) {
        self.request_tab_files(pane_id, tab_idx, path, false, cx);
    }

    pub(crate) fn eject_usb_drive(&mut self, path: String, cx: &mut Context<Self>) {
        self.context_menus.clear();
        if self.ejecting_drive || !self.state.drives.iter()
            .any(|drive| drive.path == path && drive.is_usb()) {
            cx.notify();
            return;
        }
        self.ejecting_drive = true;
        self.drive_refresh_epoch = self.drive_refresh_epoch.wrapping_add(1);
        self.drive_details_task = None;
        self.sidebar_follow_task = None;
        self.pending_sidebar_requests.clear();
        self.pending_watchers.clear();
        let watchers = std::mem::take(&mut self._path_watchers);
        self.reconcile_watchers(cx);
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = tokio::task::spawn_blocking(move || {
                // Release watches and wait for their handles to close before PnP.
                for (watched_path, watcher) in watchers {
                    watcher.release(&watched_path)?;
                }
                oxyde_lib::platform::platform().eject_drive(&path)?;
                Ok::<_, String>(oxyde_lib::commands::system::get_drives(Some(false)))
            }).await;
            let _ = this.update(cx, |this, cx| {
                this.ejecting_drive = false;
                let error = match result {
                    Ok(Ok(drives)) => {
                        this.apply_drive_snapshot(drives, cx);
                        None
                    },
                    Ok(Err(error)) => Some(error),
                    Err(error) => Some(error.to_string()),
                };
                if let Some(error) = error {
                    this.file_operation_notice = Some((Message::EjectDriveFailed, error));
                }
                this.reconcile_watchers(cx);
                cx.notify();
            });
        }).detach();
    }

    fn start_drive_refresh_task(&mut self, cx: &mut Context<Self>) {
        let h = self.handle.clone().expect("handle set");
        self._drive_refresh_task = Some(cx.spawn(move |_, cx: &mut AsyncApp| {
            let mut cx = cx.clone();
            async move {
                use std::sync::atomic::{AtomicU64, Ordering};
                let revision = std::sync::Arc::new(AtomicU64::new(0));
                let registration_revision = revision.clone();
                let registration = tokio::task::spawn_blocking(move || {
                    oxyde_lib::platform::platform().watch_drive_changes(registration_revision)
                }).await;
                let _registration = match registration {
                    Ok(Ok(guard)) => guard,
                    Ok(Err(error)) => { log::warn!("Drive notifications unavailable: {error}"); None }
                    Err(error) => { log::warn!("Drive notification worker failed: {error}"); None }
                };
                let mut schedule = drive_watchers::DriveRefreshSchedule::new(
                    _registration.is_some(), std::time::Instant::now());
                loop {
                    let paused = h.update(&mut cx, |this, _| this.ejecting_drive);
                    let observed = revision.load(Ordering::Acquire);
                    if !paused && schedule.due(observed, std::time::Instant::now()) {
                        let details_due = schedule.details_due;
                        let epoch = h.update(&mut cx, |this, _| this.drive_refresh_epoch);
                        if let Ok(roots) = tokio::task::spawn_blocking(|| {
                            oxyde_lib::platform::platform().list_drive_roots()
                        }).await {
                            h.update(&mut cx, |this, cx| {
                                if this.ejecting_drive || this.drive_refresh_epoch != epoch
                                    || revision.load(Ordering::Acquire) != observed { return; }
                                let changed = roots.len() != this.state.drives.len()
                                    || roots.iter().any(|root| !this.state.drives.iter()
                                        .any(|old| old.path == root.path && old.drive_type == root.drive_type));
                                if changed {
                                    this.drive_refresh_epoch = this.drive_refresh_epoch.wrapping_add(1);
                                }
                                this.apply_drive_roots(roots, cx);
                                if changed || details_due {
                                    this.refresh_disk_images(cx);
                                }
                                if changed || details_due || this.drive_details_task.is_none() {
                                    this.request_drive_details(cx);
                                }
                                let roots: Vec<_> = this.state.drives.iter()
                                    .map(|drive| drive.path.clone()).collect();
                                for root in roots {
                                    this.request_sidebar_nodes(root, false, cx);
                                }
                                this.reconcile_watchers(cx);
                            });
                        }
                    }
                    cx.background_executor().timer(std::time::Duration::from_millis(100)).await;
                }
            }
        }));
    }
    pub fn start_watcher_task(&mut self, cx: &mut Context<Self>) {
        self.start_drive_refresh_task(cx);
        self.refresh_disk_images(cx);
        let changes = self.watcher_changes.clone();
        let h = self.handle.clone().expect("handle set");
        self._watcher_task = Some(cx.spawn(move |_, cx: &mut AsyncApp| {
            let mut cx = cx.clone();
            async move {
                loop {
                    cx.background_executor()
                        .timer(std::time::Duration::from_millis(500))
                        .await;
                    let changed: Vec<String> = if let Ok(mut q) = changes.lock() {
                        q.drain(..).collect()
                    } else {
                        vec![]
                    };
                    if !changed.is_empty() {
                        h.update(&mut cx, |this, cx| {
                            if this.ejecting_drive { return; }
                            // Refresh only affected expanded paths off the UI thread.
                            for path in &changed {
                                if this.state.expanded_paths.contains(path)
                                    || this.state.drives.iter().any(|drive| &drive.path == path) {
                                    this.request_sidebar_nodes(path.clone(), true, cx);
                                }
                            }

                            // Reload every affected tab (active = full refresh, inactive = files only).
                            let mut reloads: Vec<(String, usize, String)> = Vec::new();
                            this.state.layout.visit_panes(&mut |p| {
                                for (idx, tab) in p.tabs.iter().enumerate() {
                                    let watched =
                                        oxyde_lib::utils::archive::split_virtual_path(&tab.path)
                                            .and_then(|(archive, _)| {
                                                archive.parent().map(|parent| {
                                                    parent.to_string_lossy().into_owned()
                                                })
                                            })
                                            .unwrap_or_else(|| tab.path.clone());
                                    if changed.contains(&watched) {
                                        reloads.push((p.id.clone(), idx, tab.path.clone()));
                                    }
                                }
                            });
                            for (pane_id, tab_idx, path) in reloads {
                                this.refresh_tab_files(&pane_id, tab_idx, path, cx);
                            }
                        });
                    }
                }
            }
        }));
    }
}

impl Render for OxydeApp {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let reference = crate::ui::scale::normalized_reference(self.config.ui_rem_size);
        crate::ui::scale::set_reference(reference);
        _window.set_rem_size(px(reference as f32));
        let zoom_handle = cx.entity();
        let theme = *cx.theme();
        use crate::ui::components::sidebar::render_sidebar;
        use crate::ui::style::TNUM;

        let handle = self.handle.clone().expect("handle set");
        let window_handle = _window.window_handle();
        if let Some((tab_id, path, is_dir)) = self.created_item_rename.clone() {
            let pane_id = self.state.active_pane_id.clone();
            let width = self
                .pane_sizes
                .get(&pane_id)
                .map(|size| size.width)
                .unwrap_or(self.list_width);
            if let Some(pane) = self.state.find_pane_mut(&pane_id) {
                let tab = pane.active_tab_mut();
                if tab.id != tab_id
                    || std::path::Path::new(&path).parent() != Some(std::path::Path::new(&tab.path))
                {
                    self.created_item_rename = None;
                } else if tab.files.iter().any(|file| file.path == path) {
                    tab.filter_name.clear();
                    tab.filter_types.clear();
                    tab.filter_sizes.clear();
                    tab.filter_dates.clear();
                    tab.update_cache();
                    tab.selected_paths.clear();
                    tab.selected_paths.insert(path.clone());
                    tab.selection_anchor = Some(path.clone());
                    if let Some(index) = tab
                        .filtered_files()
                        .iter()
                        .position(|file| file.path == path)
                    {
                        let grid = matches!(tab.view_mode, crate::ui::workspace::ViewMode::Grid);
                        let columns = if grid { crate::ui::style::GridGeometry::for_viewport(width, crate::ui::components::scrollbar::zed_track_width()).columns } else { 1 };
                        let file = &tab.filtered_files()[index];
                        tab.collapsed_date_groups.remove(&crate::ui::date_groups::DateGroup::for_timestamp(file.modified, chrono::Local::now().date_naive()));
                        let rows = crate::ui::date_groups::display_rows(tab.filtered_files(), tab.group_by_date, &tab.collapsed_date_groups, columns);
                        if let Some(row) = rows.iter().position(|row| matches!(row, crate::ui::date_groups::DisplayRow::Files(indices) if indices.contains(&index))) {
                            if grid && tab.group_by_date {
                                let y = crate::ui::date_groups::content_height(&rows[..row], true);
                                tab.list_scroll.0.borrow().base_handle.set_offset(point(px(0.0), px(-y)));
                            } else {
                                tab.list_scroll.scroll_to_item(row, gpui::ScrollStrategy::Top);
                            }
                        }
                    }
                    self.created_item_rename = None;
                    self.begin_rename(path, is_dir, true, _window, cx);
                }
            } else {
                self.created_item_rename = None;
            }
        }

        let size = _window.viewport_size();
        if self.network_force_focus_pending {
            self.network_force_focus_pending = false;
            if let Some(confirmation) = &self.network_force_confirmation {
                _window.focus(&confirmation.focus, cx);
            }
        }
        if self.network_focus_restore {
            self.network_focus_restore = false;
            _window.focus(&self.focus_handle, cx);
        }
        if let Some(dialog) = self.network_dialog.as_ref().filter(|dialog| dialog.busy) {
            if !dialog.focus.is_focused(_window) { _window.focus(&dialog.focus, cx); }
        }
        let win_w = size.width.as_f32();
        let win_h = size.height.as_f32();

        self.list_width = (win_w - self.sidebar_display_width() - logical(3.0)).max(1.0);
        self.list_height = (win_h - crate::ui::components::title_bar::titlebar_h()).max(1.0);

        if let Some(DragTarget::SelectionRectangle {
            ref pane_id,
            current_pos,
            ..
        }) = self.drag_target
        {
            let p_id = pane_id.clone();
            let pos = current_pos;
            self.handle_selection_drag(p_id, pos, cx);
        }
        self.measure_metadata_columns(_window, cx);
        self.clamp_scrolls();
        self.follow_active_path(cx);

        let settings_keyboard = handle.clone();
        let mut root = div()
            .on_key_down(move |event, _, cx| {
                if event.keystroke.key == "escape" {
                    settings_keyboard.update(cx, |this, cx| {
                        if !this.context_menus.is_empty() {
                            this.context_menus.clear();
                            cx.stop_propagation();
                            cx.notify();
                            return;
                        }
                        if this.settings_open {
                            if this.settings_picker.is_some() {
                                this.settings_picker = None;
                                this.settings_submenu_bounds = None;
                            } else {
                                this.settings_open = false;
                            }
                            cx.stop_propagation();
                            cx.notify();
                        }
                    });
                }
            })
            .track_focus(&self.focus_handle)
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().text)
            .font_family("Inter")
            .flex()
            .flex_col()
            .text_size(rems(0.8125))
            .font_weight(crate::ui::style::DEFAULT_FONT_WEIGHT)
            .child(canvas(|_, _, _| (), move |_, _, window, _| {
                // Low-level capture listeners must be installed during paint.
                let zoom_handle = zoom_handle.clone();
                window.on_mouse_event(move |event: &ScrollWheelEvent, phase, window, cx| {
                    if phase == DispatchPhase::Capture && event.modifiers.control {
                        let delta = event.delta.pixel_delta(window.line_height());
                        if delta.y != px(0.0) {
                            zoom_handle.update(cx, |this, cx| {
                                let current = this.ui_rem_size();
                                let next = if delta.y > px(0.0) { current + 1 } else { current.saturating_sub(1) };
                                this.set_ui_rem_size(next, window, cx);
                            });
                            cx.stop_propagation();
                        }
                    }
                });
            }).absolute().size_full())
            .on_action(cx.listener(Self::on_action_tout_selectionner))
            .on_action(cx.listener(Self::on_action_annuler))
            .on_action(cx.listener(Self::on_action_refaire))
            .on_action(cx.listener(|this, _: &Actualiser, _, cx| this.refresh_all(cx)))
            .on_action(cx.listener(Self::on_action_copier))
            .on_action(cx.listener(Self::on_action_couper))
            .on_action(cx.listener(Self::on_action_coller))
            .on_action(cx.listener(Self::on_action_ouvrir))
            .on_action(cx.listener(Self::on_action_renommer))
            .on_action(cx.listener(Self::on_action_supprimer));

        root = root.on_modifiers_changed({
            let h = handle.clone();
            move |e: &ModifiersChangedEvent, _, cx| {
                let modifier = if e.modifiers.control {
                    Some(true) // Ctrl → always copy
                } else if e.modifiers.shift {
                    Some(false) // Shift → always move
                } else {
                    None // auto → determined by drive context
                };
                h.update(cx, |this, cx| {
                    if this.drag_modifier_copy != modifier {
                        this.drag_modifier_copy = modifier;
                        this.update_drag_copy_mode(cx);
                    }
                });
            }
        });

        root.text_style().font_features = Some(TNUM.clone());

        root = root
            .on_mouse_exit({
                let h = handle.clone();
                move |_, _, cx| {
                    if let Some(panel) = h.read(cx).operations_panel_view.clone() {
                        panel.update(cx, |panel, cx| panel.finish_drag(cx));
                    }
                    h.update(cx, |this, cx| this.finish_pointer_drag(cx));
                }
            })
            .on_mouse_move({
                let h = handle.clone();
                move |e, window, cx| {
                    if !e.dragging() {
                        if let Some(panel) = h.read(cx).operations_panel_view.clone() {
                            panel.update(cx, |panel, cx| panel.finish_drag(cx));
                        }
                    }
                    h.update(cx, |this, cx| {
                        this.mouse_pos = e.position;
                        if !e.dragging() {
                            this.finish_pointer_drag(cx);
                            return;
                        }
                        if let Some(target) = this.drag_target.clone() {
                            let pos = e.position;
                            match target {
                                DragTarget::SidebarV {
                                    viewport_h,
                                    content_h,
                                    track_id,
                                } => {
                                    if let Some(origin) = this.scrollbar_origins.get(&track_id) {
                                        let mouse_pos_in_track = pos.y.as_f32() - origin.y;
                                        let thumb_size = (viewport_h
                                            * (viewport_h / content_h.max(1.0)).min(1.0))
                                        .max(logical(40.0));
                                        let travel_distance = (viewport_h - thumb_size).max(1.0);
                                        let content_range = content_h - viewport_h;
                                        let target_thumb_pos = (mouse_pos_in_track
                                            - this.grab_offset)
                                            .clamp(0.0, travel_distance);
                                        let target_px =
                                            (target_thumb_pos / travel_distance) * content_range;
                                        this.sidebar_scroll
                                            .0
                                            .borrow()
                                            .base_handle
                                            .set_offset(Point::new(px(0.0), px(-target_px)));
                                        cx.notify();
                                    }
                                }
                                DragTarget::SidebarH {
                                    viewport_w,
                                    content_w,
                                    track_id,
                                } => {
                                    if let Some(origin) = this.scrollbar_origins.get(&track_id) {
                                        let mouse_pos_in_track = pos.x.as_f32() - origin.x;
                                        let thumb_size = (viewport_w
                                            * (viewport_w / content_w.max(1.0)).min(1.0))
                                        .max(logical(40.0));
                                        let travel_distance = (viewport_w - thumb_size).max(1.0);
                                        let content_range = content_w - viewport_w;
                                        let target_thumb_pos = (mouse_pos_in_track
                                            - this.grab_offset)
                                            .clamp(0.0, travel_distance);
                                        let target_px =
                                            (target_thumb_pos / travel_distance) * content_range;
                                        this.sidebar_scroll_h
                                            .set_offset(Point::new(px(-target_px), px(0.0)));
                                        cx.notify();
                                    }
                                }
                                DragTarget::SidebarResize => {
                                    this.resize_sidebar(pos.x.as_f32() - this.grab_offset);
                                    cx.notify();
                                }
                                DragTarget::FileListV {
                                    pane_id,
                                    viewport_h,
                                    content_h,
                                    track_id,
                                } => {
                                    if let Some(origin) = this.scrollbar_origins.get(&track_id) {
                                        if let Some(pane) =
                                            this.state.layout.find_pane_mut(&pane_id)
                                        {
                                            let mouse_pos_in_track = pos.y.as_f32() - origin.y;
                                            let thumb_size = (viewport_h
                                                * (viewport_h / content_h.max(1.0)).min(1.0))
                                            .max(logical(40.0));
                                            let travel_distance =
                                                (viewport_h - thumb_size).max(1.0);
                                            let content_range = content_h - viewport_h;
                                            let target_thumb_pos = (mouse_pos_in_track
                                                - this.grab_offset)
                                                .clamp(0.0, travel_distance);
                                            let target_px = (target_thumb_pos / travel_distance)
                                                * content_range;
                                            pane.active_tab_mut()
                                                .list_scroll
                                                .0
                                                .borrow()
                                                .base_handle
                                                .set_offset(Point::new(px(0.0), px(-target_px)));
                                            cx.notify();
                                        }
                                    }
                                }
                                DragTarget::FileListH {
                                    pane_id,
                                    viewport_w,
                                    content_w,
                                    track_id,
                                } => {
                                    if let Some(origin) = this.scrollbar_origins.get(&track_id) {
                                        if let Some(pane) =
                                            this.state.layout.find_pane_mut(&pane_id)
                                        {
                                            let mouse_pos_in_track = pos.x.as_f32() - origin.x;
                                            let thumb_size = (viewport_w
                                                * (viewport_w / content_w.max(1.0)).min(1.0))
                                            .max(logical(40.0));
                                            let travel_distance =
                                                (viewport_w - thumb_size).max(1.0);
                                            let content_range = content_w - viewport_w;
                                            let target_thumb_pos = (mouse_pos_in_track
                                                - this.grab_offset)
                                                .clamp(0.0, travel_distance);
                                            let target_px = (target_thumb_pos / travel_distance)
                                                * content_range;
                                            pane.active_tab_mut()
                                                .list_scroll_h
                                                .set_offset(Point::new(px(-target_px), px(0.0)));
                                            cx.notify();
                                        }
                                    }
                                }
                                DragTarget::BreadcrumbV {
                                    viewport_h,
                                    content_h,
                                    track_id,
                                } => {
                                    if let Some(origin) = this.scrollbar_origins.get(&track_id) {
                                        let mouse_pos_in_track = pos.y.as_f32() - origin.y;
                                        let thumb_size = (viewport_h
                                            * (viewport_h / content_h.max(1.0)).min(1.0))
                                        .max(logical(40.0));
                                        let travel_distance = (viewport_h - thumb_size).max(1.0);
                                        let content_range = content_h - viewport_h;
                                        let target_thumb_pos = (mouse_pos_in_track
                                            - this.grab_offset)
                                            .clamp(0.0, travel_distance);
                                        let target_px =
                                            (target_thumb_pos / travel_distance) * content_range;
                                        this.breadcrumb_scroll
                                            .0
                                            .borrow()
                                            .base_handle
                                            .set_offset(Point::new(px(0.0), px(-target_px)));
                                        cx.notify();
                                    }
                                }
                                DragTarget::ContextMenuV {
                                    menu_idx,
                                    viewport_h,
                                    content_h,
                                    track_id,
                                } => {
                                    if let Some(origin) = this.scrollbar_origins.get(&track_id) {
                                        if let Some(menu) = this.context_menus.get(menu_idx) {
                                            let mouse_pos_in_track = pos.y.as_f32() - origin.y;
                                            let thumb_size = (viewport_h
                                                * (viewport_h / content_h.max(1.0)).min(1.0))
                                            .max(logical(40.0));
                                            let travel_distance =
                                                (viewport_h - thumb_size).max(1.0);
                                            let content_range = content_h - viewport_h;
                                            let target_thumb_pos = (mouse_pos_in_track
                                                - this.grab_offset)
                                                .clamp(0.0, travel_distance);
                                            let target_px = (target_thumb_pos / travel_distance)
                                                * content_range;
                                            menu.scroll_handle
                                                .0
                                                .borrow()
                                                .base_handle
                                                .set_offset(Point::new(px(0.0), px(-target_px)));
                                            cx.notify();
                                        }
                                    }
                                }
                                DragTarget::OperationsPanelV {
                                    viewport_h,
                                    content_h,
                                    track_id,
                                } => {
                                    if let Some(origin) = this.scrollbar_origins.get(&track_id) {
                                        let mouse_pos_in_track = pos.y.as_f32() - origin.y;
                                        let thumb_size = (viewport_h
                                            * (viewport_h / content_h.max(1.0)).min(1.0))
                                        .max(logical(40.0));
                                        let travel_distance = (viewport_h - thumb_size).max(1.0);
                                        let content_range = content_h - viewport_h;
                                        let target_thumb_pos = (mouse_pos_in_track
                                            - this.grab_offset)
                                            .clamp(0.0, travel_distance);
                                        let target_px =
                                            (target_thumb_pos / travel_distance) * content_range;
                                        this.operations_panel_scroll
                                            .set_offset(point(px(0.0), px(-target_px)));
                                        cx.notify();
                                    }
                                }
                                DragTarget::PaneResize {
                                    split_id,
                                    child_idx,
                                    initial_weights,
                                } => {
                                    if let Some(total_size) =
                                        this.split_sizes.get(&split_id).cloned()
                                    {
                                        if let Some((grid_handle, weights)) =
                                            this.state.layout.find_split_mut(&split_id)
                                        {
                                            let current_pos = match grid_handle {
                                                crate::ui::workspace::LayoutAxis::Horizontal => {
                                                    pos.x.as_f32()
                                                }
                                                crate::ui::workspace::LayoutAxis::Vertical => {
                                                    pos.y.as_f32()
                                                }
                                            };
                                            let delta = current_pos - this.grab_offset;
                                            let total_weight: f32 = initial_weights.iter().sum();
                                            let weight_per_pixel =
                                                total_weight / total_size.max(1.0);
                                            let delta_weight = delta * weight_per_pixel;

                                            let i = child_idx;
                                            let j = i + 1;

                                            if i < weights.len() && j < weights.len() {
                                                let total_pair_weight =
                                                    initial_weights[i] + initial_weights[j];
                                                let new_wi = (initial_weights[i] + delta_weight)
                                                    .clamp(0.1, total_pair_weight - 0.1);
                                                let new_wj = total_pair_weight - new_wi;

                                                weights[i] = new_wi;
                                                weights[j] = new_wj;
                                                cx.notify();
                                            }
                                        }
                                    }
                                }
                                DragTarget::SelectionRectangle {
                                    ref pane_id,
                                    start_pos,
                                    modifiers,
                                    ref initial_selection,
                                    ..
                                } => {
                                    let current_pos = pos;
                                    let initial_selection_cap = initial_selection.clone();
                                    this.drag_target = Some(DragTarget::SelectionRectangle {
                                        pane_id: pane_id.clone(),
                                        start_pos,
                                        current_pos,
                                        modifiers,
                                        initial_selection: initial_selection_cap,
                                    });
                                    cx.notify();
                                }
                            }
                        }
                        if let Some(panel) = this.operations_panel_view.clone() {
                            let pointer = e.position;
                            let viewport = window.viewport_size();
                            panel.update(cx, |panel, cx| {
                                panel.update_drag(pointer, viewport, cx);
                            });
                        }
                    });
                }
            })
            .on_mouse_up(MouseButton::Left, {
                let h = handle.clone();
                move |_, _window, cx| {
                    h.update(cx, |this, cx| {
                        this.finish_pointer_drag(cx);
                    });
                }
            })
            .on_mouse_down(MouseButton::Navigate(NavigationDirection::Back), {
                let h = handle.clone();
                move |_, _, cx| {
                    h.update(cx, |v, cx| {
                        let p_id = v.state.active_pane_id.clone();
                        v.go_back(&p_id, cx)
                    });
                }
            })
            .on_mouse_down(MouseButton::Navigate(NavigationDirection::Forward), {
                let h = handle.clone();
                move |_, _, cx| {
                    h.update(cx, |v, cx| {
                        let p_id = v.state.active_pane_id.clone();
                        v.go_forward(&p_id, cx)
                    });
                }
            })
            .child({
                // ── Custom title bar ──────────────────────────────────────
                use crate::ui::components::title_bar::render_title_bar;
                render_title_bar(self, handle.clone(), _window.is_maximized(), cx)
            })
            .child({
                // ── Horizontal content area (sidebar + panes) ─────────────
                let layout_tree = self.state.layout.clone();
                div()
                    .flex()
                    .flex_1()
                    .w_full()
                    .overflow_hidden()
                    .child(
                        div()
                            .w(px(self.sidebar_display_width()))
                            .h_full()
                            .flex_none()
                            .child(render_sidebar(self, handle.clone(), cx)),
                    )
                    .child(
                        div()
                            .id("sidebar-resize-handle")
                            .group("sidebar-resize-handle")
                            .relative()
                            .w(rems(0.0625))
                            .bg(theme.border)
                            .cursor(CursorStyle::ResizeLeftRight)
                            .h_full()
                            .flex_none()
                            .flex()
                            .justify_center()
                            .child(
                                deferred(div()
                                    .group("sidebar-resize-hit-area")
                                    .absolute()
                                    .left(rems(-0.0625))
                                    .w(rems(0.1875))
                                    .h_full()
                                    .occlude()
                                    .cursor(CursorStyle::ResizeLeftRight)
                                    .child(
                                        div()
                                            .absolute()
                                            .left(rems(0.0625))
                                            .top_0()
                                            .bottom_0()
                                            .w(rems(0.0625))
                                            .group_hover("sidebar-resize-hit-area", |s| {
                                                s.bg(theme.accent.opacity(0.6))
                                            }),
                                    )
                                    .on_mouse_down(MouseButton::Left, {
                                        let h = handle.clone();
                                        move |e, _, cx| {
                                            cx.stop_propagation();
                                            h.update(cx, |this, cx| {
                                                this.drag_target = Some(DragTarget::SidebarResize);
                                                this.grab_offset = e.position.x.as_f32() - this.sidebar_display_width();
                                                cx.notify();
                                            });
                                        }
                                    })),
                            )
                            .child(
                                div()
                                    .w(rems(0.0625))
                                    .h_full()
                                    .bg(theme.border)
                                    .group_hover("sidebar-resize-handle", |s| {
                                        s.bg(theme.accent.opacity(0.6))
                                    }),
                            )
                            .on_mouse_down(MouseButton::Left, {
                                let h = handle.clone();
                                move |e, _, cx| {
                                    h.update(cx, |this, _| {
                                        this.drag_target = Some(DragTarget::SidebarResize);
                                        this.grab_offset =
                                            e.position.x.as_f32() - this.sidebar_display_width();
                                    });
                                }
                            }),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .h_full()
                            .overflow_hidden()
                            .child(self.render_layout_node(&layout_tree, handle.clone(), cx)),
                    )
            });

        // Keep modal content below transient context menus in the render tree:
        // a text field inside the rename dialog can open its own context menu.
        if self.network_force_confirmation.is_some() {
            root = root.child(deferred(crate::ui::components::network_dialog::render_force_confirmation(self, handle.clone(), _window, cx)));
        } else if self.network_dialog.is_some() {
            root = root.child(deferred(crate::ui::components::network_dialog::render_network_dialog(self, handle.clone(), _window, cx)));
        }
        if self.editing_path.is_some() && !self.inline_rename {
            root = root.child(deferred(
                crate::ui::components::rename_dialog::render_rename_dialog(
                    self,
                    handle.clone(),
                    cx,
                ),
            ));
        }

        // CONTEXT MENU OVERLAY
        for (m_idx, instance) in self.context_menus.iter().enumerate() {
            use crate::ui::components::dropdown::GenericDropdown;

            let h = handle.clone();
            let pos = instance.position;
            let scroll_handle = instance.scroll_handle.clone();
            let is_v_dragging = matches!(self.drag_target, Some(DragTarget::ContextMenuV { menu_idx, .. }) if menu_idx == m_idx);
            let _menu_width = instance.width;

            let content_element = match &instance.content {
                ContextMenuContent::Items(items, header_type) => {
                    let header = header_type.as_ref().map(|ht| match ht {
                        ContextMenuHeader::TypeSelectAll { pane_id } => {
                            let pid_cap = pane_id.clone();
                            let h_inner = h.clone();

                            let (label, should_clear) =
                                if let Some(pane) = self.state.find_pane(&pid_cap) {
                                    let tab = pane.active_tab();
                                    let mut all_present = std::collections::HashSet::new();
                                    for f in &tab.files {
                                        if f.is_dir {
                                            all_present.insert("Dossier".to_string());
                                        } else if let Some(ext) = std::path::Path::new(&f.name)
                                            .extension()
                                            .and_then(|e| e.to_str())
                                        {
                                            all_present.insert(ext.to_uppercase());
                                        }
                                    }
                                    let is_full = !all_present.is_empty()
                                        && all_present.iter().all(|t| tab.filter_types.contains(t));
                                    if is_full {
                                        (Message::DeselectAllChecks, true)
                                    } else {
                                        (Message::SelectAllChecks, false)
                                    }
                                } else {
                                    (Message::SelectAllChecks, false)
                                };

                            crate::ui::elements::button::Button::new(SharedString::from(format!(
                                "select-all-type-{}",
                                pid_cap
                            )))
                            .label(cx.i18n().text(label))
                            .label_size(13.0)
                            .variant(crate::ui::elements::button::ButtonVariant::Subtle)
                            .on_click(move |_, cx| {
                                let pid_inner = pid_cap.clone();
                                h_inner.update(cx, |this, cx| {
                                    if should_clear {
                                        this.clear_filter_types(&pid_inner, cx);
                                    } else {
                                        if let Some(pane) = this.state.find_pane_mut(&pid_inner) {
                                            let tab = pane.active_tab_mut();
                                            let all_files = tab.files.clone();
                                            for f in &all_files {
                                                if f.is_dir {
                                                    tab.filter_types.insert("Dossier".to_string());
                                                } else if let Some(ext) =
                                                    std::path::Path::new(&f.name)
                                                        .extension()
                                                        .and_then(|e| e.to_str())
                                                {
                                                    tab.filter_types.insert(ext.to_uppercase());
                                                }
                                            }
                                            tab.update_cache();
                                        }
                                    }
                                    cx.notify();
                                    this.context_menus.truncate(m_idx + 1);
                                    this.open_type_filter_menu(&pid_inner, pos, cx);
                                });
                            })
                            .into_any_element()
                        }
                        ContextMenuHeader::SizeSelectAll { pane_id } => {
                            let pid_cap = pane_id.clone();
                            let h_inner = h.clone();

                            let (label, should_clear) =
                                if let Some(p) = self.state.find_pane(&pid_cap) {
                                    use crate::ui::workspace::SizeCategory;
                                    let tab = p.active_tab();
                                    let categories = [
                                        SizeCategory::Minuscule,
                                        SizeCategory::Petit,
                                        SizeCategory::Moyen,
                                        SizeCategory::Grand,
                                        SizeCategory::Huge,
                                    ];
                                    let mut present = std::collections::HashSet::new();
                                    for f in &tab.files {
                                        for cat in &categories {
                                            if cat.matches(f.size) {
                                                present.insert(*cat);
                                                break;
                                            }
                                        }
                                    }
                                    let is_full = !present.is_empty()
                                        && present.iter().all(|c| tab.filter_sizes.contains(c));
                                    if is_full {
                                        (Message::DeselectAllChecks, true)
                                    } else {
                                        (Message::SelectAllChecks, false)
                                    }
                                } else {
                                    (Message::SelectAllChecks, false)
                                };

                            crate::ui::elements::button::Button::new(SharedString::from(format!(
                                "select-all-size-{}",
                                pid_cap
                            )))
                            .label(cx.i18n().text(label))
                            .label_size(13.0)
                            .variant(crate::ui::elements::button::ButtonVariant::Subtle)
                            .on_click(move |_, cx| {
                                let pid_inner = pid_cap.clone();
                                h_inner.update(cx, |this, cx| {
                                    if should_clear {
                                        this.clear_filter_sizes(&pid_inner, cx);
                                    } else {
                                        if let Some(p) = this.state.find_pane_mut(&pid_inner) {
                                            use crate::ui::workspace::SizeCategory;
                                            let tab = p.active_tab_mut();
                                            tab.filter_sizes.insert(SizeCategory::Minuscule);
                                            tab.filter_sizes.insert(SizeCategory::Petit);
                                            tab.filter_sizes.insert(SizeCategory::Moyen);
                                            tab.filter_sizes.insert(SizeCategory::Grand);
                                            tab.filter_sizes.insert(SizeCategory::Huge);
                                            tab.update_cache();
                                        }
                                    }
                                    cx.notify();
                                    this.context_menus.truncate(m_idx + 1);
                                    this.open_size_filter_menu(&pid_inner, pos, cx);
                                });
                            })
                            .into_any_element()
                        }
                        ContextMenuHeader::DateSelectAll { pane_id } => {
                            let pid_cap = pane_id.clone();
                            let h_inner = h.clone();

                            let (label, should_clear) =
                                if let Some(p) = self.state.find_pane(&pid_cap) {
                                    use crate::ui::workspace::DateCategory;
                                    let tab = p.active_tab();
                                    let categories = [
                                        DateCategory::Aujourdhui,
                                        DateCategory::Hier,
                                        DateCategory::CetteSemaine,
                                        DateCategory::LaSemaineDerniere,
                                        DateCategory::PlusAncien,
                                    ];
                                    let mut present = std::collections::HashSet::new();
                                    for f in &tab.files {
                                        for cat in &categories {
                                            if cat.matches(f.modified) {
                                                present.insert(*cat);
                                                break;
                                            }
                                        }
                                    }
                                    let is_full = !present.is_empty()
                                        && present.iter().all(|c| tab.filter_dates.contains(c));
                                    if is_full {
                                        (Message::DeselectAllChecks, true)
                                    } else {
                                        (Message::SelectAllChecks, false)
                                    }
                                } else {
                                    (Message::SelectAllChecks, false)
                                };

                            crate::ui::elements::button::Button::new(SharedString::from(format!(
                                "select-all-date-{}",
                                pid_cap
                            )))
                            .label(cx.i18n().text(label))
                            .label_size(13.0)
                            .variant(crate::ui::elements::button::ButtonVariant::Subtle)
                            .on_click(move |_, cx| {
                                let pid_inner = pid_cap.clone();
                                h_inner.update(cx, |this, cx| {
                                    if should_clear {
                                        this.clear_filter_dates(&pid_inner, cx);
                                    } else {
                                        if let Some(p) = this.state.find_pane_mut(&pid_inner) {
                                            use crate::ui::workspace::DateCategory;
                                            let tab = p.active_tab_mut();
                                            tab.filter_dates.insert(DateCategory::Aujourdhui);
                                            tab.filter_dates.insert(DateCategory::Hier);
                                            tab.filter_dates.insert(DateCategory::CetteSemaine);
                                            tab.filter_dates
                                                .insert(DateCategory::LaSemaineDerniere);
                                            tab.filter_dates.insert(DateCategory::PlusAncien);
                                            tab.update_cache();
                                        }
                                    }
                                    cx.notify();
                                    this.context_menus.truncate(m_idx + 1);
                                    this.open_date_filter_menu(&pid_inner, pos, cx);
                                });
                            })
                            .into_any_element()
                        }
                    });

                    let dropdown_items = items
                        .iter()
                        .map(|it| {
                            let on_click = it.on_click.clone();
                            crate::ui::components::dropdown::DropdownItem {
                                label: it.label.clone(),
                                icon: it.icon.clone(),
                                stay_open: it.stay_open,
                                has_submenu: it.has_submenu,
                                is_dimmed: it.is_dimmed,
                                on_click: std::rc::Rc::new(move |window, app| {
                                    (on_click)(window, app);
                                }),
                                on_hover: it.on_hover.clone(),
                            }
                        })
                        .collect();

                    let items_cap = items.clone();
                    GenericDropdown {
                        id: ("context-menu", m_idx).into(),
                        h_app: h.clone(),
                        items: dropdown_items,
                        reserve_icon_space: true,
                        is_context_menu: true,
                        bottom_padding: instance.bottom_padding,
                        scroll_handle,
                        width: None,
                        max_height: logical(480.0),
                        on_close: {
                            let h = h.clone();
                            std::rc::Rc::new(move |_, cx| {
                                h.update(cx, |this, cx| {
                                    this.context_menus.clear();
                                    this.submenu_timer = None;
                                    this.submenu_close_timer = None;
                                    cx.notify();
                                });
                            })
                        },
                        on_outside: Some({
                            let h = h.clone();
                            std::rc::Rc::new(move |position, _, cx| {
                                h.update(cx, |this, cx| {
                                    let inside_sibling = this.context_menus.iter().enumerate().any(
                                        |(index, menu)| {
                                            index != m_idx
                                                && context_menu_contains_point(
                                                    menu,
                                                    position,
                                                    px(win_h),
                                                )
                                        },
                                    );
                                    if !inside_sibling {
                                        this.context_menus.clear();
                                        this.submenu_timer = None;
                                        this.submenu_close_timer = None;
                                        cx.notify();
                                    }
                                });
                            })
                        }),
                        on_drag: {
                            std::rc::Rc::new(move |this, tid, vh, ch, _| {
                                this.drag_target = Some(DragTarget::ContextMenuV {
                                    menu_idx: m_idx,
                                    viewport_h: vh,
                                    content_h: ch,
                                    track_id: tid,
                                });
                            })
                        },
                        is_dragging: is_v_dragging,
                        header,
                        on_item_hover: Some(std::rc::Rc::new({
                            let h_app = h.clone();
                            let items_val = items_cap.clone();
                            move |ix, item_pos, cx| {
                                let should_handle = h_app.update(cx, |this, cx| {
                                    let Some(menu) = this.context_menus.get_mut(m_idx) else {
                                        return false;
                                    };
                                    if menu.hovered_item == Some(ix) {
                                        return false;
                                    }
                                    menu.hovered_item = Some(ix);
                                    cx.notify();
                                    true
                                });
                                if !should_handle {
                                    return;
                                }
                                if let Some(it) = items_val.get(ix) {
                                    let has_submenu = it.on_hover.is_some();
                                    let h_inner = h_app.clone();
                                    let items_inner = items_val.clone();
                                    let ix_inner = ix;
                                    let pos_inner = item_pos;
                                    let window_handle_inner = window_handle;

                                    if has_submenu {
                                        cx.defer(move |cx| {
                                            let _h_inner_spawn = h_inner.clone();
                                            h_inner.update(cx, |this, cx| {
                                                let parent_left = this
                                                    .context_menus
                                                    .get(m_idx)
                                                    .map(|m| m.position.x)
                                                    .unwrap_or_default();
                                                let parent_right = parent_left
                                                    + this
                                                        .context_menus
                                                        .get(m_idx)
                                                        .map(|m| m.width)
                                                        .unwrap_or_default();
                                                // Compute Y from parent menu position + item index (reliable, no canvas-bounds dependency)
                                                let parent_pos_y = this
                                                    .context_menus
                                                    .get(m_idx)
                                                    .map(|m| m.position.y)
                                                    .unwrap_or_default();
                                                let parent_menu_height = this
                                                    .context_menus
                                                    .get(m_idx)
                                                    .map(|menu| {
                                                        context_menu_height(menu, px(win_h))
                                                    })
                                                    .unwrap_or_default();
                                                this.submenu_close_timer = None;
                                                this.submenu_timer =
                                                    Some(cx.spawn(move |_, cx: &mut AsyncApp| {
                                                        let mut cx_val = cx.clone();
                                                        async move {
                                                            cx_val
                                                                .background_executor()
                                                                .timer(std::time::Duration::ZERO)
                                                                .await;
                                                            let _ = window_handle_inner.update(
                                                                &mut cx_val,
                                                                |_, _window, app| {
                                                                    let viewport_w = _window
                                                                        .viewport_size()
                                                                        .width;
                                                                    let parent_visible_y =
                                                                        context_menu_visible_top(
                                                                            parent_pos_y,
                                                                            parent_menu_height,
                                                                            _window
                                                                                .viewport_size()
                                                                                .height,
                                                                        );
                                                                    let open_left = parent_right
                                                                        + rem_pixels(rems(13.75))
                                                                        > viewport_w;
                                                                    if let Some(it) =
                                                                        items_inner.get(ix_inner)
                                                                    {
                                                                        if let Some(on_h) =
                                                                            it.on_hover.clone()
                                                                        {
                                                                            let mut sub_pos =
                                                                                pos_inner;
                                                                            sub_pos.x = if open_left
                                                                            {
                                                                                parent_left
                                                                                    + rem_pixels(rems(0.125))
                                                                            // TopRight anchor: right edge at parent left
                                                                            } else {
                                                                                parent_right
                                                                                    - rem_pixels(rems(0.125))
                                                                                // TopLeft anchor: left edge at parent right
                                                                            };
                                                                            // Y: parent menu top + border(1) + padding(4) + ix * item_h
                                                                            // minus submenu border(1) + padding(4) → nets to ix * item_h
                                                                            sub_pos.y =
                                                                                parent_visible_y
                                                                                    + px(items_inner.iter().take(ix_inner).filter(|item| !matches!(item.icon, Some(crate::ui::components::dropdown::IconName::Separator))).count()
                                                                                        as f32
                                                                                        * logical(26.0) + items_inner.iter().take(ix_inner).filter(|item| matches!(item.icon, Some(crate::ui::components::dropdown::IconName::Separator))).count() as f32 * logical(9.0));
                                                                            (on_h)(
                                                                                sub_pos, open_left,
                                                                                app,
                                                                            );
                                                                        }
                                                                    }
                                                                },
                                                            );
                                                        }
                                                    }));
                                            });
                                        });
                                    } else {
                                        // Item has no submenu → close any open children immediately
                                        cx.defer(move |cx| {
                                            h_inner.update(cx, |this, cx| {
                                                this.submenu_timer = None;
                                                this.submenu_close_timer = None;
                                                if this.context_menus.len() > m_idx + 1 {
                                                    this.context_menus.truncate(m_idx + 1);
                                                    cx.notify();
                                                }
                                            });
                                        });
                                    }
                                } else {
                                    let h_close = h_app.clone();
                                    cx.defer(move |cx| {
                                        h_close.update(cx, |this, cx| {
                                            this.submenu_timer = None;
                                            this.submenu_close_timer = None;
                                            if this.context_menus.len() > m_idx + 1 {
                                                this.context_menus.truncate(m_idx + 1);
                                                cx.notify();
                                            }
                                        });
                                    });
                                }
                            }
                        })),
                        on_width_calculated: Some(std::rc::Rc::new({
                            let h = h.clone();
                            move |w, cx| {
                                h.update(cx, |this, _| {
                                    if let Some(menu) = this.context_menus.get_mut(m_idx) {
                                        menu.width = w;
                                    }
                                });
                            }
                        })),
                        on_hover_change: Some(std::rc::Rc::new({
                            let h = h.clone();
                            move |is_hovering: bool, cx: &mut App| {
                                if is_hovering {
                                    h.update(cx, |this, _| {
                                        this.submenu_close_timer = None;
                                    });
                                } else {
                                    h.update(cx, |this, _| {
                                        if let Some(menu) = this.context_menus.get_mut(m_idx) {
                                            menu.hovered_item = None;
                                        }
                                    });
                                    let h_close = h.clone();
                                    let wh = window_handle;
                                    h.update(cx, |this, cx| {
                                        this.submenu_close_timer =
                                            Some(cx.spawn(move |_, cx: &mut AsyncApp| {
                                                let mut cx_val = cx.clone();
                                                async move {
                                                    cx_val
                                                        .background_executor()
                                                        .timer(std::time::Duration::from_millis(
                                                            250,
                                                        ))
                                                        .await;
                                                    let _ = wh.update(&mut cx_val, |_, _, cx| {
                                                        let h_inner = h_close.clone();
                                                        cx.defer(move |cx| {
                                                            h_inner.update(cx, |this, cx| {
                                                                if this.context_menus.len()
                                                                    > m_idx + 1
                                                                {
                                                                    this.context_menus
                                                                        .truncate(m_idx + 1);
                                                                    cx.notify();
                                                                }
                                                            });
                                                        });
                                                    });
                                                }
                                            }));
                                    });
                                }
                            }
                        })),
                    }
                }
                ContextMenuContent::NativeItems {
                    path,
                    is_background,
                    items,
                } => {
                    let path_cap = path.clone();
                    let is_bg_cap = *is_background;
                    let h_app = h.clone();
                    let items_cap = items.clone();

                    let dropdown_items = items
                        .iter()
                        .map(|it| {
                            let h = h_app.clone();
                            let p = path_cap.clone();
                            let id = it.id;
                            let has_sub = it.has_submenu;

                            crate::ui::components::dropdown::DropdownItem {
                                label: it.label.clone(),
                                icon: if m_idx == 1 {
                                    crate::ui::components::dropdown::native_menu_icon(
                                        it.verb.as_deref(),
                                        &it.label,
                                    )
                                } else {
                                    None
                                },
                                stay_open: has_sub,
                                has_submenu: has_sub,
                                // Windows returns -1 for popup entries that own a submenu.
                                // That is not a disabled command; only dim actual leaf entries
                                // whose IDs are invalid.
                                is_dimmed: id < 0 && !has_sub,
                                on_click: std::rc::Rc::new({
                                    move |_, app| {
                                        let h_inner = h.clone();
                                        let p_inner = p.clone();
                                        let id_inner = id;

                                        h_inner.update(app, |this, cx| {
                                            if !has_sub && id_inner >= 0 {
                                                fn is_creation(
                                                    items: &[oxyde_lib::models::WinMenuItem],
                                                    id: i32,
                                                    in_new: bool,
                                                ) -> bool
                                                {
                                                    items.iter().any(|item| {
                                                        let in_new = in_new
                                                            || item.verb.as_deref() == Some("new");
                                                        (item.id == id && in_new)
                                                            || is_creation(
                                                                &item.children,
                                                                id,
                                                                in_new,
                                                            )
                                                    })
                                                }
                                                let creates_item = is_bg_cap
                                                    && this.context_menus.iter().any(|menu| {
                                                        if let ContextMenuContent::NativeItems {
                                                            items,
                                                            ..
                                                        } = &menu.content
                                                        {
                                                            is_creation(items, id_inner, false)
                                                        } else {
                                                            false
                                                        }
                                                    });
                                                this.context_menus.clear();
                                                this.execute_native_context_menu_item(
                                                    p_inner,
                                                    id_inner,
                                                    is_bg_cap,
                                                    creates_item,
                                                    cx,
                                                );
                                            }
                                            cx.notify();
                                        });
                                    }
                                }),
                                on_hover: None,
                            }
                        })
                        .collect();

                    GenericDropdown {
                        id: ("context-menu", m_idx).into(),
                        h_app: h.clone(),
                        items: dropdown_items,
                        reserve_icon_space: m_idx == 1,
                        is_context_menu: true,
                        bottom_padding: instance.bottom_padding,
                        scroll_handle,
                        width: None,
                        max_height: native_menu_max_height(px(win_h)),
                        on_close: {
                            let h = h.clone();
                            std::rc::Rc::new(move |_, cx| {
                                h.update(cx, |this, cx| {
                                    this.context_menus.clear();
                                    this.submenu_timer = None;
                                    this.submenu_close_timer = None;
                                    cx.notify();
                                });
                            })
                        },
                        on_outside: Some({
                            let h = h.clone();
                            std::rc::Rc::new(move |position, _, cx| {
                                h.update(cx, |this, cx| {
                                    let inside_sibling = this.context_menus.iter().enumerate().any(
                                        |(index, menu)| {
                                            index != m_idx
                                                && context_menu_contains_point(
                                                    menu,
                                                    position,
                                                    px(win_h),
                                                )
                                        },
                                    );
                                    if !inside_sibling {
                                        this.context_menus.clear();
                                        this.submenu_timer = None;
                                        this.submenu_close_timer = None;
                                        cx.notify();
                                    }
                                });
                            })
                        }),
                        on_drag: {
                            std::rc::Rc::new(move |this, tid, vh, ch, _| {
                                this.drag_target = Some(DragTarget::ContextMenuV {
                                    menu_idx: m_idx,
                                    viewport_h: vh,
                                    content_h: ch,
                                    track_id: tid,
                                });
                            })
                        },
                        is_dragging: is_v_dragging,
                        header: None,
                        on_item_hover: Some(std::rc::Rc::new({
                            let h_app = h.clone();
                            let items_val = items_cap.clone();
                            move |ix, item_pos, cx| {
                                let should_handle = h_app.update(cx, |this, cx| {
                                    let Some(menu) = this.context_menus.get_mut(m_idx) else {
                                        return false;
                                    };
                                    if menu.hovered_item == Some(ix) {
                                        return false;
                                    }
                                    menu.hovered_item = Some(ix);
                                    cx.notify();
                                    true
                                });
                                if !should_handle {
                                    return;
                                }
                                if let Some(it) = items_val.get(ix) {
                                    let h_inner = h_app.clone();
                                    let path_inner = path_cap.clone();
                                    let sub_items = it.children.clone();
                                    let has_sub = it.has_submenu;
                                    let window_handle_inner = window_handle;

                                    cx.defer(move |cx| {
                                        let h_inner_spawn = h_inner.clone();
                                        h_inner.update(cx, |this, cx| {
                                            if has_sub && !sub_items.is_empty() {
                                                let path_cap = path_inner.clone();
                                                let sub_items_cap = sub_items.clone();
                                                let pos_inner = item_pos;
                                                let parent_left = this.context_menus.get(m_idx)
                                                    .map(|m| m.position.x)
                                                    .unwrap_or_default();
                                                let parent_right = parent_left + this.context_menus.get(m_idx)
                                                    .map(|m| m.width)
                                                    .unwrap_or_default();
                                                let parent_pos_y = this.context_menus.get(m_idx)
                                                    .map(|m| m.position.y)
                                                    .unwrap_or_default();
                                                let parent_menu_height = this.context_menus.get(m_idx)
                                                    .map(|menu| context_menu_height(menu, px(win_h)))
                                                    .unwrap_or_default();
                                                let ix_cap = ix;

                                                this.submenu_close_timer = None;
                                                this.submenu_timer = Some(cx.spawn(move |_, cx: &mut AsyncApp| {
                                                    let mut cx_val = cx.clone();
                                                    async move {
                                                        cx_val.background_executor().timer(std::time::Duration::ZERO).await;
                                                        let _ = window_handle_inner.update(&mut cx_val, |_, _window, cx| {
                                                            let viewport_w = _window.viewport_size().width;
                                                            let parent_visible_y = context_menu_visible_top(
                                                                parent_pos_y,
                                                                parent_menu_height,
                                                                _window.viewport_size().height,
                                                            );
                                                            let open_left = parent_right + rem_pixels(rems(13.75)) > viewport_w;
                                                            let app_handle = h_inner_spawn.clone();
                                                            let path_cap = path_cap.clone();
                                                            let sub_items_cap = sub_items_cap.clone();

                                                            cx.defer(move |cx| {
                                                                app_handle.update(cx, |this, cx| {
                                                                    let is_already_open = this.context_menus.get(m_idx + 1)
                                                                        .map(|m| matches!(&m.content, ContextMenuContent::NativeItems { items, .. } if items.first().map(|it| it.id) == sub_items_cap.first().map(|it| it.id)))
                                                                        .unwrap_or(false);

                                                                    if !is_already_open {
                                                                        this.context_menus.truncate(m_idx + 1);
                                                                        let mut new_pos = pos_inner;
                                                                        new_pos.x = if open_left {
                                                                            parent_left + rem_pixels(rems(0.125)) // TopRight: right edge at parent left
                                                                        } else {
                                                                            parent_right - rem_pixels(rems(0.125)) // TopLeft: left edge at parent right
                                                                        };
                                                                        new_pos.y = parent_visible_y + px(ix_cap as f32 * logical(26.0));

                                                                        this.context_menus.push(ContextMenuInstance {
                                                                            position: new_pos,
                                                                            open_left,
                                                                            hovered_item: None,
                                                                            native_request_id: None,
                                                                            content: ContextMenuContent::NativeItems {
                                                                                path: path_cap,
                                                                                is_background: is_bg_cap,
                                                                                items: sub_items_cap
                                                                            },
                                                                            scroll_handle: UniformListScrollHandle::new(),
                                    width: px(0.0),
                                    bottom_padding: 0.0,
                                });
                                                                        cx.notify();
                                                                    }
                                                                });
                                                            });
                                                        });
                                                    }
                                                }));
                                            } else {
                                                this.submenu_timer = None;
                                                this.submenu_close_timer = None;
                                                if this.context_menus.len() > m_idx + 1 {
                                                    this.context_menus.truncate(m_idx + 1);
                                                    cx.notify();
                                                }
                                            }
                                        });
                                    });
                                }
                            }
                        })),
                        on_width_calculated: Some(std::rc::Rc::new({
                            let h = h.clone();
                            move |w, cx| {
                                h.update(cx, |this, _| {
                                    if let Some(menu) = this.context_menus.get_mut(m_idx) {
                                        menu.width = w;
                                    }
                                });
                            }
                        })),
                        on_hover_change: Some(std::rc::Rc::new({
                            let h = h.clone();
                            move |is_hovering: bool, cx: &mut App| {
                                if is_hovering {
                                    h.update(cx, |this, _| {
                                        this.submenu_close_timer = None;
                                    });
                                } else {
                                    h.update(cx, |this, _| {
                                        if let Some(menu) = this.context_menus.get_mut(m_idx) {
                                            menu.hovered_item = None;
                                        }
                                    });
                                    let h_close = h.clone();
                                    let wh = window_handle;
                                    h.update(cx, |this, cx| {
                                        this.submenu_close_timer =
                                            Some(cx.spawn(move |_, cx: &mut AsyncApp| {
                                                let mut cx_val = cx.clone();
                                                async move {
                                                    cx_val
                                                        .background_executor()
                                                        .timer(std::time::Duration::from_millis(
                                                            250,
                                                        ))
                                                        .await;
                                                    let _ = wh.update(&mut cx_val, |_, _, cx| {
                                                        let h_inner = h_close.clone();
                                                        cx.defer(move |cx| {
                                                            h_inner.update(cx, |this, cx| {
                                                                if this.context_menus.len()
                                                                    > m_idx + 1
                                                                {
                                                                    this.context_menus
                                                                        .truncate(m_idx + 1);
                                                                    cx.notify();
                                                                }
                                                            });
                                                        });
                                                    });
                                                }
                                            }));
                                    });
                                }
                            }
                        })),
                    }
                }
                ContextMenuContent::NameFilter { input } => GenericDropdown {
                    id: ("context-menu", m_idx).into(),
                    h_app: h.clone(),
                    items: vec![],
                    reserve_icon_space: true,
                    is_context_menu: true,
                    bottom_padding: instance.bottom_padding,
                    scroll_handle,
                    width: Some(logical(200.0)),
                    max_height: logical(480.0),
                    on_close: {
                        let h = h.clone();
                        std::rc::Rc::new(move |_, cx| {
                            h.update(cx, |this, cx| {
                                this.context_menus.clear();
                                this.submenu_timer = None;
                                this.submenu_close_timer = None;
                                cx.notify();
                            });
                        })
                    },
                    on_outside: Some({
                        let h = h.clone();
                        std::rc::Rc::new(move |position, _, cx| {
                            h.update(cx, |this, cx| {
                                let inside_sibling =
                                    this.context_menus.iter().enumerate().any(|(index, menu)| {
                                        index != m_idx
                                            && context_menu_contains_point(
                                                menu,
                                                position,
                                                px(win_h),
                                            )
                                    });
                                if !inside_sibling {
                                    this.context_menus.clear();
                                    this.submenu_timer = None;
                                    this.submenu_close_timer = None;
                                    cx.notify();
                                }
                            });
                        })
                    }),
                    on_drag: {
                        std::rc::Rc::new(move |this, tid, vh, ch, _| {
                            this.drag_target = Some(DragTarget::ContextMenuV {
                                menu_idx: m_idx,
                                viewport_h: vh,
                                content_h: ch,
                                track_id: tid,
                            });
                        })
                    },
                    is_dragging: false,
                    header: Some(
                        div()
                            .w(rems(12.5))
                            .h(px(crate::ui::style::row_height()))
                            .child(input.clone())
                            .into_any_element(),
                    ),
                    on_item_hover: None,
                    on_width_calculated: Some(std::rc::Rc::new({
                        let h = h.clone();
                        move |w, cx| {
                            h.update(cx, |this, _| {
                                if let Some(menu) = this.context_menus.get_mut(m_idx) {
                                    menu.width = w;
                                }
                            });
                        }
                    })),
                    on_hover_change: None,
                },
            };

            root = root.child(deferred(if instance.open_left {
                // Snap (no corner-switch) so the submenu extends left from parent_left
                anchored()
                    .position(pos)
                    .anchor(Anchor::TopRight)
                    .snap_to_window()
                    .child(content_element)
            } else {
                anchored()
                    .position(pos)
                    .snap_to_window()
                    .child(content_element)
            }));
        }

        // SETTINGS OVERLAY
        if self.operations_panel_open {
            if let Some(panel) = self.operations_panel_view.clone() {
                root = root.child(deferred(panel));
            }
        }

        if self.settings_open {
            use crate::ui::components::settings::render_settings_panel;
            root = root.child(deferred(render_settings_panel(self, handle.clone(), _window, cx)));
        }

        if let Some(pending) = self.pending_file_operations.front() {
            root = root.child(deferred(
                crate::ui::components::conflict_dialog::render_conflict_dialog(
                    pending.conflict_paths.len(),
                    handle.clone(),
                    cx,
                ),
            ));
        }

        if let Some((key, detail)) = self.file_operation_notice.clone() {
            let mut args = FluentArgs::new();
            args.set("detail", detail);
            let message = cx.i18n().text_with_args(key, &args);
            let close_handle = handle.clone();
            let theme = *cx.theme();
            root = root.child(deferred(
                crate::ui::components::modal::Modal::new(
                    "operation-notice",
                    cx.i18n().text(Message::OperationFailed),
                    div().text_color(theme.text_muted).child(message),
                )
                .title_icon("icons/circle-x.svg")
                .action(
                    crate::ui::components::modal::action_button("operation-notice-close")
                        .hover(|d| d.bg(theme.control_hover))
                        .child(cx.i18n().text(Message::Close))
                        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                            close_handle.update(cx, |this, cx| {
                                this.file_operation_notice = None;
                                cx.notify();
                            });
                        }),
                ),
            ));
        }

        if let Some((message, _)) = self.network_notice {
            let close = handle.clone();
            let theme = *cx.theme();
            root = root.child(deferred(div().id("network-success-notice").absolute().bottom(rems(1.0)).right(rems(1.0))
                .max_w(_window.viewport_size().width - rem_pixels(rems(2.0)))
                .rounded_md().border(rems(0.0625)).border_color(theme.accent).bg(theme.surface)
                .px_3().py_2().flex().items_center().gap(rems(0.5)).text_size(rems(0.8125)).text_color(theme.text).occlude()
                .child(svg().path("icons/circle-check.svg").size(rems(1.0)).text_color(theme.accent))
                .child(cx.i18n().text(message))
                .child(div().id("network-notice-close").p_1().on_click(move |_, _, cx| close.update(cx, |view, cx| { view.network_notice = None; cx.notify(); }))
                    .child(svg().path("icons/x.svg").size(rems(0.875)).text_color(theme.text_muted)))));
        }
        root
    }
}
