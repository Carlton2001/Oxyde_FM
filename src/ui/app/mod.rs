use gpui::*;
pub use gpui::prelude::*;
pub use crate::ui::theme::{PixelsExt, ThemeContext, OxydeTheme};
pub use crate::ui::workspace::WorkspaceState;
pub use oxyde_lib::models::FileEntry;

pub mod navigation;
pub mod selection;
pub mod filters;
pub mod layout;

#[allow(unused_imports)]
pub use navigation::*;
#[allow(unused_imports)]
pub use selection::*;
#[allow(unused_imports)]
pub use filters::*;
#[allow(unused_imports)]
pub use layout::*;

use crate::ui::components::tooltip::Tooltip;

actions!(oxyde, [Ouvrir, Copier, Couper, Coller, Renommer, Supprimer, ToutSelectionner, Annuler, Refaire]);

#[derive(Debug, PartialEq, Clone)]
pub enum DragTarget {
    SidebarV { viewport_h: f32, content_h: f32, track_id: String },
    SidebarH { viewport_w: f32, content_w: f32, track_id: String },
    SidebarResize,
    FileListV { pane_id: String, viewport_h: f32, content_h: f32, track_id: String },
    FileListH { pane_id: String, viewport_w: f32, content_w: f32, track_id: String },
    BreadcrumbV { viewport_h: f32, content_h: f32, track_id: String },
    ContextMenuV { menu_idx: usize, viewport_h: f32, content_h: f32, track_id: String },
    PaneResize { split_id: String, child_idx: usize, initial_weights: Vec<f32> },
    SelectionRectangle { pane_id: String, start_pos: Point<Pixels>, current_pos: Point<Pixels>, modifiers: Modifiers, initial_selection: std::collections::HashSet<String> },
}

#[derive(Clone)]
pub enum ContextMenuHeader {
    TypeSelectAll { pane_id: String },
    SizeSelectAll { pane_id: String },
    DateSelectAll { pane_id: String },
}

#[derive(Clone)]
pub enum ContextMenuContent {
    Items(Vec<crate::ui::components::dropdown::DropdownItem>, Option<ContextMenuHeader>),
    NativeItems { path: String, is_background: bool, items: Vec<oxyde_lib::models::WinMenuItem> },
    NameFilter { input: Entity<crate::ui::components::input::TextInput> },
}

#[derive(Clone)]
pub struct ContextMenuInstance {
    pub position: Point<Pixels>,
    pub content: ContextMenuContent,
    pub scroll_handle: UniformListScrollHandle,
    pub width: Pixels,
    /// If true, the menu opens to the LEFT of its anchor (use Corner::TopRight).
    pub open_left: bool,
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
        Self { paths, cursor_offset, app_handle, _subscription: sub }
    }
}

fn drag_op_pill(key_hint: &str, label: &str, active: bool, theme: &OxydeTheme) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .gap(px(4.0))
        .px(px(6.0))
        .py(px(3.0))
        .rounded_md()
        .bg(if active { theme.accent } else { theme.header })
        .when(!key_hint.is_empty(), |d| {
            d.child(
                div()
                    .text_size(px(9.0))
                    .font_weight(FontWeight::BOLD)
                    .text_color(if active { theme.background } else { theme.text_muted })
                    .child(key_hint.to_string())
            )
        })
        .child(
            div()
                .text_size(px(11.0))
                .font_weight(FontWeight::MEDIUM)
                .text_color(if active { theme.background } else { theme.text_muted })
                .child(label.to_string())
        )
}

impl Render for FileDragPreview {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let is_copy = self.app_handle.read(cx).drag_copy_mode;
        let theme = *cx.theme();

        let (icon_path, name_label) = if self.paths.len() == 1 {
            let p = std::path::Path::new(&self.paths[0]);
            let is_dir = p.is_dir();
            let name = p.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("Fichier")
                .to_string();
            (if is_dir { "icons/folder.svg" } else { "icons/file.svg" }, name)
        } else {
            ("icons/file.svg", format!("{} éléments", self.paths.len()))
        };

        // Root is 0×0; the visible ghost is absolutely positioned at cursor + (12, 4).
        div()
            .relative()
            .child(
                div()
                    .absolute()
                    .left(self.cursor_offset.x + px(12.0))
                    .top(self.cursor_offset.y + px(4.0))
                    .px(px(10.0))
                    .py(px(8.0))
                    .rounded_md()
                    .bg(theme.surface)
                    .border_1()
                    .border_color(theme.accent)
                    .flex()
                    .flex_col()
                    .gap(px(6.0))
                    .whitespace_nowrap()
                    .child(
                        // File icon + name row
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6.0))
                            .child(
                                svg()
                                    .path(icon_path)
                                    .size(px(14.0))
                                    .flex_none()
                                    .text_color(theme.accent)
                            )
                            .child(
                                div()
                                    .text_color(theme.text)
                                    .text_size(px(12.0))
                                    .font_weight(FontWeight::MEDIUM)
                                    .child(name_label)
                            )
                    )
                    .child(
                        // Operation pills row
                        div()
                            .flex()
                            .gap(px(4.0))
                            .child(drag_op_pill("+Ctrl", "Copier", is_copy, &theme))
                            .child(drag_op_pill("+Shift", "Déplacer", !is_copy, &theme))
                    )
            )
    }
}

#[derive(Clone)]
pub struct TooltipData {
    pub name: String,
    pub type_str: String,
    pub size_str: String,
    pub date_str: String,
    pub full_path: Option<String>,
    pub position: Point<Pixels>,
    pub view: Option<AnyView>,
}

struct ManualTooltipElement {
    data: TooltipData,
}

impl IntoElement for ManualTooltipElement {
    type Element = Self;
    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for ManualTooltipElement {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        (window.request_layout(Style::default(), None, cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let view = if let Some(view) = self.data.view.as_ref() {
            view.clone()
        } else if let Some(path) = self.data.full_path.as_ref() {
            Tooltip::for_file_full(
                self.data.name.clone(),
                path.clone(),
                self.data.type_str.clone(),
                self.data.size_str.clone(),
                self.data.date_str.clone(),
            )(window, cx)
        } else {
            Tooltip::for_file(
                self.data.name.clone(),
                self.data.type_str.clone(),
                self.data.size_str.clone(),
                self.data.date_str.clone(),
            )(window, cx)
        };
        window.set_tooltip(AnyTooltip {
            view,
            mouse_position: window.mouse_position(),
            check_visible_and_update: std::rc::Rc::new(|_, _, _| true),
        });
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        _prepaint: &mut Self::PrepaintState,
        _window: &mut Window,
        _cx: &mut App,
    ) {
    }
}

pub struct OxydeApp {
    pub state: WorkspaceState,
    pub handle: Option<Entity<Self>>,
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
    pub sidebar_content_width: f32,
    pub list_height: f32,
    pub list_width: f32,
    pub split_sizes: std::collections::HashMap<String, f32>,
    pub pane_sizes: std::collections::HashMap<String, Size<f32>>,
    pub pane_origins: std::collections::HashMap<String, Point<f32>>,
    pub list_bounds: std::collections::HashMap<String, Bounds<f32>>,
    pub scrollbar_origins: std::collections::HashMap<String, Point<f32>>,
    pub breadcrumb_menu: Option<(String, String, usize, Vec<FileEntry>)>, // (pane_id, path, index, dirs)
    pub context_menus: Vec<ContextMenuInstance>,
    pub submenu_timer: Option<Task<()>>,
    pub submenu_close_timer: Option<Task<()>>,
    pub dragging_tab: Option<(String, usize)>,
    pub tooltip_request: Option<TooltipData>,
    pub tooltip_timer: Option<Task<()>>,
    pub mouse_pos: Point<Pixels>,
    #[allow(dead_code)]
    pub file_ops: std::sync::Arc<oxyde_lib::systems::file_ops::FileOperationManager>,
    pub watcher_changes: std::sync::Arc<std::sync::Mutex<std::collections::VecDeque<String>>>,
    pub _path_watchers: std::collections::HashMap<String, Box<dyn std::any::Any>>,
    pub _watcher_task: Option<Task<()>>,
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
}

impl OxydeApp {
    pub fn bind_keys(cx: &mut App) {
        cx.bind_keys([
            KeyBinding::new("ctrl-a", ToutSelectionner, None),
            KeyBinding::new("ctrl-z", Annuler, None),
            KeyBinding::new("ctrl-y", Refaire, None),
            KeyBinding::new("ctrl-shift-z", Refaire, None),
        ]);
    }

    pub fn execute_undo(&mut self, cx: &mut Context<Self>) {
        let history = self.history.clone();
        let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
        let _ = rt.block_on(oxyde_lib::commands::ops::undo_last_action(&history));
        cx.notify();
    }

    pub fn execute_redo(&mut self, cx: &mut Context<Self>) {
        let history = self.history.clone();
        let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
        let _ = rt.block_on(oxyde_lib::commands::ops::redo_last_action(&history));
        cx.notify();
    }

    pub fn empty(cx: &mut Context<Self>) -> Self {
        Self {
            state: WorkspaceState::new(),
            handle: None,
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
            sidebar_width: 260.0,
            sidebar_content_width: 260.0,
            list_height: 800.0,
            list_width: 1000.0,
            split_sizes: std::collections::HashMap::new(),
            pane_sizes: std::collections::HashMap::new(),
            pane_origins: std::collections::HashMap::new(),
            list_bounds: std::collections::HashMap::new(),
            scrollbar_origins: std::collections::HashMap::new(),
            breadcrumb_menu: None,
            context_menus: Vec::new(),
            submenu_timer: None,
            submenu_close_timer: None,
            dragging_tab: None,
            tooltip_request: None,
            tooltip_timer: None,
            mouse_pos: Point::default(),
            file_ops: std::sync::Arc::new(oxyde_lib::systems::file_ops::FileOperationManager::new()),
            watcher_changes: std::sync::Arc::new(std::sync::Mutex::new(std::collections::VecDeque::new())),
            _path_watchers: std::collections::HashMap::new(),
            _watcher_task: None,
            _subscriptions: Vec::new(),
            drag_copy_mode: false,
            drag_modifier_copy: None,
            drag_target_default_copy: false,
        }
    }

    /// Recompute `drag_copy_mode` from modifier override and drive-based default.
    /// Notifies if the value changed.
    pub fn update_drag_copy_mode(&mut self, cx: &mut Context<Self>) {
        let new_mode = self.drag_modifier_copy.unwrap_or(self.drag_target_default_copy);
        if self.drag_copy_mode != new_mode {
            self.drag_copy_mode = new_mode;
            cx.notify();
        }
    }

    pub fn execute_file_move(&mut self, paths: Vec<String>, target_dir: String, cx: &mut Context<Self>) {
        use oxyde_lib::models::transaction::{Transaction, TransactionType, TransactionDetails};

        // Skip if source and destination are the same directory
        let all_same = paths.iter().all(|p| {
            std::path::Path::new(p).parent()
                .and_then(|par| par.to_str())
                .map(|par| {
                    let norm_par = par.replace('\\', "/").trim_end_matches('/').to_string();
                    let norm_dst = target_dir.replace('\\', "/").trim_end_matches('/').to_string();
                    norm_par == norm_dst
                })
                .unwrap_or(false)
        });
        if all_same {
            return;
        }

        // Prevent moving a folder into itself or one of its own descendants.
        // Path::starts_with does component-level matching so "C:\foo" won't
        // accidentally match "C:\foobar".
        let dst = std::path::Path::new(&target_dir);
        if paths.iter().any(|p| dst.starts_with(std::path::Path::new(p))) {
            return;
        }

        let mut moved: Vec<String> = Vec::new();
        for path in &paths {
            let src = std::path::Path::new(path);
            if let Some(file_name) = src.file_name() {
                let dst = std::path::Path::new(&target_dir).join(file_name);
                let ok = if std::fs::rename(src, &dst).is_ok() {
                    true
                } else if src.is_dir() {
                    let opts = fs_extra::dir::CopyOptions::new();
                    fs_extra::dir::move_dir(src, std::path::Path::new(&target_dir), &opts).is_ok()
                } else {
                    let opts = fs_extra::file::CopyOptions::new();
                    fs_extra::file::move_file(src, &dst, &opts).is_ok()
                };
                if ok {
                    moved.push(path.clone());
                }
            }
        }

        if !moved.is_empty() {
            self.history.push(Transaction::new(
                TransactionType::Move,
                TransactionDetails {
                    paths: moved,
                    target_dir: Some(target_dir),
                    old_path: None,
                    new_path: None,
                    created_files: None,
                },
            ));
            // Refresh sidebar so has_subdirs and directory tree reflect the new state.
            self.state.refresh_sidebar();
            self.reconcile_watchers();
        }

        cx.notify();
    }

    pub fn execute_file_copy(&mut self, paths: Vec<String>, target_dir: String, cx: &mut Context<Self>) {
        use oxyde_lib::models::transaction::{Transaction, TransactionType, TransactionDetails};

        let all_same = paths.iter().all(|p| {
            std::path::Path::new(p).parent()
                .and_then(|par| par.to_str())
                .map(|par| {
                    let norm_par = par.replace('\\', "/").trim_end_matches('/').to_string();
                    let norm_dst = target_dir.replace('\\', "/").trim_end_matches('/').to_string();
                    norm_par == norm_dst
                })
                .unwrap_or(false)
        });
        if all_same {
            return;
        }

        // Prevent copying a folder into itself or one of its own descendants.
        let dst = std::path::Path::new(&target_dir);
        if paths.iter().any(|p| dst.starts_with(std::path::Path::new(p))) {
            return;
        }

        let mut copied: Vec<String> = Vec::new();
        for path in &paths {
            let src = std::path::Path::new(path);
            if let Some(file_name) = src.file_name() {
                let dst = std::path::Path::new(&target_dir).join(file_name);
                let ok = if src.is_dir() {
                    let opts = fs_extra::dir::CopyOptions::new();
                    fs_extra::dir::copy(src, std::path::Path::new(&target_dir), &opts).is_ok()
                } else {
                    let opts = fs_extra::file::CopyOptions::new();
                    fs_extra::file::copy(src, &dst, &opts).is_ok()
                };
                if ok {
                    copied.push(path.clone());
                }
            }
        }

        if !copied.is_empty() {
            self.history.push(Transaction::new(
                TransactionType::Copy,
                TransactionDetails {
                    paths: copied,
                    target_dir: Some(target_dir),
                    old_path: None,
                    new_path: None,
                    created_files: None,
                },
            ));
            self.state.refresh_sidebar();
            self.reconcile_watchers();
        }

        cx.notify();
    }

    pub fn create_new_folder(&mut self, pane_id: &str, cx: &mut Context<Self>) {
        use oxyde_lib::models::transaction::{Transaction, TransactionType, TransactionDetails};
        
        if let Some(pane) = self.state.layout.find_pane(pane_id) {
            let current_path = pane.active_tab().path.clone();
            let base_name = "Nouveau dossier";
            let mut folder_path = std::path::PathBuf::from(&current_path).join(base_name);
            let mut counter = 1;

            while folder_path.exists() {
                counter += 1;
                folder_path = std::path::PathBuf::from(&current_path).join(format!("{} ({})", base_name, counter));
            }

            if std::fs::create_dir_all(&folder_path).is_ok() {
                let path_str = folder_path.to_string_lossy().to_string();
                self.history.push(Transaction::new(
                    TransactionType::NewFolder,
                    TransactionDetails {
                        paths: vec![path_str.clone()],
                        target_dir: Some(current_path),
                        old_path: None,
                        new_path: None,
                        created_files: None,
                    },
                ));
                self.state.refresh_sidebar();
                self.reconcile_watchers();
            }
        }
        cx.notify();
    }

    /// Rebuild the set of directory watchers to cover every open tab across all panes,
    /// plus the expanded directories visible in the sidebar tree.
    /// Watchers are keyed by path so identical paths across multiple tabs share one handle.
    pub fn reconcile_watchers(&mut self) {
        use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};
        use crate::ui::workspace::SidebarItem;

        // Collect every unique path that should be watched.
        let mut needed: std::collections::HashSet<String> = std::collections::HashSet::new();
        self.state.layout.visit_panes(&mut |p| {
            for tab in &p.tabs {
                if !tab.path.is_empty() {
                    needed.insert(tab.path.clone());
                }
            }
        });
        for item in &self.state.sidebar_items {
            if let SidebarItem::Directory { node, .. } = item {
                needed.insert(node.path.clone());
            }
        }

        // Drop watchers whose path is no longer needed.
        self._path_watchers.retain(|p, _| needed.contains(p));

        // Create watchers for newly-needed paths.
        let changes = self.watcher_changes.clone();
        for path in needed {
            if self._path_watchers.contains_key(&path) {
                continue;
            }
            let path_for_cb = path.clone();
            let ch = changes.clone();
            let watcher = RecommendedWatcher::new(
                move |res: notify::Result<Event>| {
                    if res.is_ok() {
                        if let Ok(mut q) = ch.lock() {
                            if !q.contains(&path_for_cb) {
                                q.push_back(path_for_cb.clone());
                            }
                        }
                    }
                },
                Config::default(),
            );
            if let Ok(mut w) = watcher {
                let p = std::path::PathBuf::from(&path);
                if p.exists() && w.watch(&p, RecursiveMode::NonRecursive).is_ok() {
                    self._path_watchers.insert(path, Box::new(w));
                }
            }
        }
    }

    /// Refresh the file list of a specific tab without disturbing its scroll / selection / filters.
    pub fn refresh_tab_files(&mut self, pane_id: &str, tab_idx: usize, path: String, cx: &mut Context<Self>) {
        let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
        if let Ok(response) = rt.block_on(oxyde_lib::commands::io::list_dir(path, None, None, None, None)) {
            if let Some(pane) = self.state.find_pane_mut(pane_id) {
                if let Some(tab) = pane.tabs.get_mut(tab_idx) {
                    tab.files = response.entries;
                    tab.sort_files();
                    tab.update_cache();
                }
            }
            cx.notify();
        }
    }

    pub fn start_watcher_task(&mut self, cx: &mut Context<Self>) {
        let changes = self.watcher_changes.clone();
        let h = self.handle.clone().expect("handle set");
        self._watcher_task = Some(cx.spawn(move |_, cx: &mut AsyncApp| {
            let mut cx = cx.clone();
            async move {
                loop {
                    cx.background_executor().timer(std::time::Duration::from_millis(500)).await;
                    let changed: Vec<String> = if let Ok(mut q) = changes.lock() {
                        q.drain(..).collect()
                    } else {
                        vec![]
                    };
                    if !changed.is_empty() {
                        let _ = h.update(&mut cx, |this, cx| {
                            use crate::ui::workspace::SidebarItem;

                            // Check if any watched sidebar directory changed → rebuild the tree.
                            let sidebar_changed = this.state.sidebar_items.iter().any(|item| {
                                if let SidebarItem::Directory { node, .. } = item {
                                    changed.contains(&node.path)
                                } else {
                                    false
                                }
                            });
                            if sidebar_changed {
                                this.state.refresh_sidebar();
                                this.reconcile_watchers();
                            }

                            // Reload every affected tab (active = full refresh, inactive = files only).
                            let mut reloads: Vec<(String, usize, String, bool)> = Vec::new();
                            this.state.layout.visit_panes(&mut |p| {
                                for (idx, tab) in p.tabs.iter().enumerate() {
                                    if changed.contains(&tab.path) {
                                        reloads.push((p.id.clone(), idx, tab.path.clone(), idx == p.active_tab_index));
                                    }
                                }
                            });
                            for (pane_id, tab_idx, path, is_active) in reloads {
                                if is_active {
                                    this.load_path_sync(&pane_id, path, cx);
                                } else {
                                    this.refresh_tab_files(&pane_id, tab_idx, path, cx);
                                }
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
        use crate::ui::components::sidebar::render_sidebar;
        use crate::ui::style::TNUM;

        let handle = self.handle.clone().expect("handle set");
        let window_handle = _window.window_handle();

        let size = _window.viewport_size();
        let win_w = size.width.as_f32();
        let win_h = size.height.as_f32();

        self.list_width = (win_w - self.sidebar_width - 4.0).max(1.0);
        self.list_height = win_h.max(1.0);

        if let Some(DragTarget::SelectionRectangle { ref pane_id, current_pos, .. }) = self.drag_target {
            let p_id = pane_id.clone();
            let pos = current_pos;
            self.handle_selection_drag(p_id, pos, cx);
        }

        self.clamp_scrolls();

        let mut root = div()
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().text)
            .font_family("Inter")
            .flex()
            .text_size(px(13.0))
            .font_weight(FontWeight::LIGHT)
            .on_action({
                let h = handle.clone();
                move |_: &ToutSelectionner, _, cx| {
                    let _ = h.update(cx, |this, cx| {
                        let pane_id = this.state.active_pane_id.clone();
                        this.select_all(&pane_id, cx);
                    });
                }
            })
            .on_action({
                let h = handle.clone();
                move |_: &Annuler, _, cx| {
                    let _ = h.update(cx, |this, cx| this.execute_undo(cx));
                }
            })
            .on_action({
                let h = handle.clone();
                move |_: &Refaire, _, cx| {
                    let _ = h.update(cx, |this, cx| this.execute_redo(cx));
                }
            })
            .on_modifiers_changed({
                let h = handle.clone();
                move |e: &ModifiersChangedEvent, _, cx| {
                    let modifier = if e.modifiers.control {
                        Some(true)   // Ctrl → always copy
                    } else if e.modifiers.shift {
                        Some(false)  // Shift → always move
                    } else {
                        None         // auto → determined by drive context
                    };
                    let _ = h.update(cx, |this, cx| {
                        if this.drag_modifier_copy != modifier {
                            this.drag_modifier_copy = modifier;
                            this.update_drag_copy_mode(cx);
                        }
                    });
                }
            });
        
        root.text_style().get_or_insert_with(Default::default).font_features = Some(TNUM.clone());

        root = root
            .on_mouse_move({
                let h = handle.clone();
                move |e, _window, cx| {
                    let _ = h.update(cx, |this, cx| {
                        this.mouse_pos = e.position;
                        if let Some(target) = this.drag_target.clone() {
                            let pos = e.position;
                            match target {
                                DragTarget::SidebarV { viewport_h, content_h, track_id } => {
                                      if let Some(origin) = this.scrollbar_origins.get(&track_id) {
                                          let mouse_pos_in_track = pos.y.as_f32() - origin.y;
                                          let thumb_size = (viewport_h * (viewport_h / content_h.max(1.0)).min(1.0)).max(40.0);
                                          let travel_distance = (viewport_h - thumb_size).max(1.0);
                                          let content_range = content_h - viewport_h;
                                          let target_thumb_pos = (mouse_pos_in_track - this.grab_offset).clamp(0.0, travel_distance);
                                          let target_px = (target_thumb_pos / travel_distance) * content_range;
                                          this.sidebar_scroll.0.borrow().base_handle.set_offset(Point::new(px(0.0), px(-target_px)));
                                          cx.notify();
                                      }
                                }
                                DragTarget::SidebarH { viewport_w, content_w, track_id } => {
                                      if let Some(origin) = this.scrollbar_origins.get(&track_id) {
                                          let mouse_pos_in_track = pos.x.as_f32() - origin.x;
                                          let thumb_size = (viewport_w * (viewport_w / content_w.max(1.0)).min(1.0)).max(40.0);
                                          let travel_distance = (viewport_w - thumb_size).max(1.0);
                                          let content_range = content_w - viewport_w;
                                          let target_thumb_pos = (mouse_pos_in_track - this.grab_offset).clamp(0.0, travel_distance);
                                          let target_px = (target_thumb_pos / travel_distance) * content_range;
                                          this.sidebar_scroll_h.set_offset(Point::new(px(-target_px), px(0.0)));
                                          cx.notify();
                                      }
                                }
                                DragTarget::SidebarResize => {
                                      this.sidebar_width = (pos.x.as_f32() - this.grab_offset).clamp(150.0, 600.0);
                                      cx.notify();
                                }
                                DragTarget::FileListV { pane_id, viewport_h, content_h, track_id } => {
                                      if let Some(origin) = this.scrollbar_origins.get(&track_id) {
                                          if let Some(pane) = this.state.layout.find_pane_mut(&pane_id) {
                                              let mouse_pos_in_track = pos.y.as_f32() - origin.y;
                                              let thumb_size = (viewport_h * (viewport_h / content_h.max(1.0)).min(1.0)).max(40.0);
                                              let travel_distance = (viewport_h - thumb_size).max(1.0);
                                              let content_range = content_h - viewport_h;
                                              let target_thumb_pos = (mouse_pos_in_track - this.grab_offset).clamp(0.0, travel_distance);
                                              let target_px = (target_thumb_pos / travel_distance) * content_range;
                                              pane.active_tab_mut().list_scroll.0.borrow().base_handle.set_offset(Point::new(px(0.0), px(-target_px)));
                                              cx.notify();
                                          }
                                      }
                                }
                                DragTarget::FileListH { pane_id, viewport_w, content_w, track_id } => {
                                         if let Some(origin) = this.scrollbar_origins.get(&track_id) {
                                             if let Some(pane) = this.state.layout.find_pane_mut(&pane_id) {
                                                 let mouse_pos_in_track = pos.x.as_f32() - origin.x;
                                                 let thumb_size = (viewport_w * (viewport_w / content_w.max(1.0)).min(1.0)).max(40.0);
                                                 let travel_distance = (viewport_w - thumb_size).max(1.0);
                                                 let content_range = content_w - viewport_w;
                                                 let target_thumb_pos = (mouse_pos_in_track - this.grab_offset).clamp(0.0, travel_distance);
                                                 let target_px = (target_thumb_pos / travel_distance) * content_range;
                                                 pane.active_tab_mut().list_scroll_h.set_offset(Point::new(px(-target_px), px(0.0)));
                                                 cx.notify();
                                             }
                                         }
                                }
                                DragTarget::BreadcrumbV { viewport_h, content_h, track_id } => {
                                      if let Some(origin) = this.scrollbar_origins.get(&track_id) {
                                          let mouse_pos_in_track = pos.y.as_f32() - origin.y;
                                          let thumb_size = (viewport_h * (viewport_h / content_h.max(1.0)).min(1.0)).max(40.0);
                                          let travel_distance = (viewport_h - thumb_size).max(1.0);
                                          let content_range = content_h - viewport_h;
                                          let target_thumb_pos = (mouse_pos_in_track - this.grab_offset).clamp(0.0, travel_distance);
                                          let target_px = (target_thumb_pos / travel_distance) * content_range;
                                          this.breadcrumb_scroll.0.borrow().base_handle.set_offset(Point::new(px(0.0), px(-target_px)));
                                          cx.notify();
                                      }
                                }
                                 DragTarget::ContextMenuV { menu_idx, viewport_h, content_h, track_id } => {
                                       if let Some(origin) = this.scrollbar_origins.get(&track_id) {
                                           if let Some(menu) = this.context_menus.get(menu_idx) {
                                               let mouse_pos_in_track = pos.y.as_f32() - origin.y;
                                               let thumb_size = (viewport_h * (viewport_h / content_h.max(1.0)).min(1.0)).max(40.0);
                                               let travel_distance = (viewport_h - thumb_size).max(1.0);
                                               let content_range = content_h - viewport_h;
                                               let target_thumb_pos = (mouse_pos_in_track - this.grab_offset).clamp(0.0, travel_distance);
                                               let target_px = (target_thumb_pos / travel_distance) * content_range;
                                               menu.scroll_handle.0.borrow().base_handle.set_offset(Point::new(px(0.0), px(-target_px)));
                                               cx.notify();
                                           }
                                       }
                                 }
                                DragTarget::PaneResize { split_id, child_idx, initial_weights } => {
                                     if let Some(total_size) = this.split_sizes.get(&split_id).cloned() {
                                          if let Some((grid_handle, weights)) = this.state.layout.find_split_mut(&split_id) {
                                              let current_pos = match grid_handle {
                                                  crate::ui::workspace::LayoutAxis::Horizontal => pos.x.as_f32(),
                                                  crate::ui::workspace::LayoutAxis::Vertical => pos.y.as_f32(),
                                              };
                                              let delta = current_pos - this.grab_offset;
                                              let total_weight: f32 = initial_weights.iter().sum();
                                              let weight_per_pixel = total_weight / total_size.max(1.0);
                                              let delta_weight = delta * weight_per_pixel;
                                              
                                              let i = child_idx;
                                              let j = i + 1;
                                              
                                              if i < weights.len() && j < weights.len() {
                                                  let total_pair_weight = initial_weights[i] + initial_weights[j];
                                                  let new_wi = (initial_weights[i] + delta_weight).clamp(0.1, total_pair_weight - 0.1);
                                                  let new_wj = total_pair_weight - new_wi;
                                                  
                                                  weights[i] = new_wi;
                                                  weights[j] = new_wj;
                                                  cx.notify();
                                              }
                                          }
                                     }
                                }
                                DragTarget::SelectionRectangle { ref pane_id, start_pos, modifiers, ref initial_selection, .. } => {
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
                    });
                }
            })
            .on_mouse_up(MouseButton::Left, {
                let h = handle.clone();
                move |_, _window, cx| {
                    let _ = h.update(cx, |this, cx| {
                        let mut needs_notify = false;
                        if this.drag_target.is_some() {
                            this.drag_target = None;
                            needs_notify = true;
                        }
                        if this.dragging_tab.is_some() {
                            this.dragging_tab = None;
                            needs_notify = true;
                        }
                        if this.drop_target_idx.is_some() || this.drop_target_pane.is_some() {
                            this.drop_target_idx = None;
                            this.drop_target_pane = None;
                            needs_notify = true;
                        }
                        if this.highlight_split_pane.is_some() {
                            this.highlight_split_pane = None;
                            needs_notify = true;
                        }
                        if needs_notify {
                            cx.notify();
                        }
                    });
                }
            })
            .on_mouse_down(MouseButton::Navigate(NavigationDirection::Back), {
                let h = handle.clone();
                move |_, _, cx| { let _ = h.update(cx, |v, cx| {
                    let p_id = v.state.active_pane_id.clone();
                    v.go_back(&p_id, cx)
                }); }
            })
            .on_mouse_down(MouseButton::Navigate(NavigationDirection::Forward), {
                let h = handle.clone();
                move |_, _, cx| { let _ = h.update(cx, |v, cx| {
                    let p_id = v.state.active_pane_id.clone();
                    v.go_forward(&p_id, cx)
                }); }
            })
            .child(
                div()
                    .w(px(self.sidebar_width))
                    .h_full()
                    .flex_none()
                    .child(render_sidebar(self, handle.clone(), cx))
            )
            .child(
                div()
                    .id("sidebar-resize-handle")
                    .w(px(4.0))
                    .h_full()
                    .bg(rgb(0x1a1a1a))
                    .cursor_col_resize()
                    .hover(|s| s.bg(rgb(0x0078d4)))
                    .active(|s| s.bg(rgb(0x0078d4)))
                    .on_mouse_down(MouseButton::Left, {
                        let h = handle.clone();
                        move |e, _, cx| {
                            let _ = h.update(cx, |this, _| {
                                this.drag_target = Some(DragTarget::SidebarResize);
                                this.grab_offset = e.position.x.as_f32() - this.sidebar_width;
                            });
                        }
                    })
            );
        
        let layout_tree = self.state.layout.clone();
        root = root.child(div().flex_1().h_full().child(self.render_layout_node(&layout_tree, handle.clone(), cx)));
        
        // CONTEXT MENU OVERLAY
        for (m_idx, instance) in self.context_menus.iter().enumerate() {
            use crate::ui::components::dropdown::{GenericDropdown};

            let h = handle.clone();
            let pos = instance.position;
            let scroll_handle = instance.scroll_handle.clone();
            let is_v_dragging = matches!(self.drag_target, Some(DragTarget::ContextMenuV { menu_idx, .. }) if menu_idx == m_idx);
            let _menu_width = instance.width;

            let content_element = match &instance.content {
                ContextMenuContent::Items(items, header_type) => {
                    let header = header_type.as_ref().map(|ht| {
                        match ht {
                            ContextMenuHeader::TypeSelectAll { pane_id } => {
                                let pid_cap = pane_id.clone();
                                let h_inner = h.clone();
                                
                                let (label, should_clear) = if let Some(pane) = self.state.find_pane(&pid_cap) {
                                    let tab = pane.active_tab();
                                    let mut all_present = std::collections::HashSet::new();
                                    for f in &tab.files {
                                        if f.is_dir { all_present.insert("Dossier".to_string()); }
                                        else if let Some(ext) = std::path::Path::new(&f.name).extension().and_then(|e| e.to_str()) {
                                            all_present.insert(ext.to_uppercase());
                                        }
                                    }
                                    let is_full = !all_present.is_empty() && all_present.iter().all(|t| tab.filter_types.contains(t));
                                    if is_full || tab.filter_types.is_empty() { ("Tout cocher", false) } else { ("Tout décocher", true) }
                                } else { ("Tout cocher", false) };

                                crate::ui::elements::button::Button::new(SharedString::from(format!("select-all-type-{}", pid_cap)))
                                    .label(label)
                                    .variant(crate::ui::elements::button::ButtonVariant::Subtle)
                                    .on_click(move |_, cx| {
                                        let pid_inner = pid_cap.clone();
                                        let _ = h_inner.update(cx, |this, cx| {
                                            if should_clear {
                                                this.clear_filter_types(&pid_inner, cx);
                                            } else {
                                                if let Some(pane) = this.state.find_pane_mut(&pid_inner) {
                                                    let tab = pane.active_tab_mut();
                                                    let all_files = tab.files.clone();
                                                    for f in &all_files {
                                                        if f.is_dir { tab.filter_types.insert("Dossier".to_string()); }
                                                        else if let Some(ext) = std::path::Path::new(&f.name).extension().and_then(|e| e.to_str()) {
                                                            tab.filter_types.insert(ext.to_uppercase());
                                                        }
                                                    }
                                                }
                                            }
                                            this.context_menus.truncate(m_idx + 1);
                                            this.open_type_filter_menu(&pid_inner, pos, cx);
                                        });
                                    })
                                    .into_any_element()
                            }
                            ContextMenuHeader::SizeSelectAll { pane_id } => {
                                let pid_cap = pane_id.clone();
                                let h_inner = h.clone();
                                
                                let (label, should_clear) = if let Some(p) = self.state.find_pane(&pid_cap) {
                                    use crate::ui::workspace::SizeCategory;
                                    let tab = p.active_tab();
                                    let categories = [SizeCategory::Minuscule, SizeCategory::Petit, SizeCategory::Moyen, SizeCategory::Grand, SizeCategory::Huge];
                                    let mut present = std::collections::HashSet::new();
                                    for f in &tab.files {
                                        for cat in &categories {
                                            if cat.matches(f.size) { present.insert(*cat); break; }
                                        }
                                    }
                                    let is_full = !present.is_empty() && present.iter().all(|c| tab.filter_sizes.contains(c));
                                    if is_full || tab.filter_sizes.is_empty() { ("Tout cocher", false) } else { ("Tout décocher", true) }
                                } else { ("Tout cocher", false) };

                                crate::ui::elements::button::Button::new(SharedString::from(format!("select-all-size-{}", pid_cap)))
                                    .label(label)
                                    .variant(crate::ui::elements::button::ButtonVariant::Subtle)
                                    .on_click(move |_, cx| {
                                        let pid_inner = pid_cap.clone();
                                        let _ = h_inner.update(cx, |this, cx| {
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
                                                }
                                            }
                                            this.context_menus.truncate(m_idx + 1);
                                            this.open_size_filter_menu(&pid_inner, pos, cx);
                                        });
                                    })
                                    .into_any_element()
                            }
                            ContextMenuHeader::DateSelectAll { pane_id } => {
                                let pid_cap = pane_id.clone();
                                let h_inner = h.clone();
                                
                                let (label, should_clear) = if let Some(p) = self.state.find_pane(&pid_cap) {
                                    use crate::ui::workspace::DateCategory;
                                    let tab = p.active_tab();
                                    let categories = [DateCategory::Aujourdhui, DateCategory::Hier, DateCategory::CetteSemaine, DateCategory::LaSemaineDerniere, DateCategory::PlusAncien];
                                    let mut present = std::collections::HashSet::new();
                                    for f in &tab.files {
                                        for cat in &categories {
                                            if cat.matches(f.modified) { present.insert(*cat); break; }
                                        }
                                    }
                                    let is_full = !present.is_empty() && present.iter().all(|c| tab.filter_dates.contains(c));
                                    if is_full || tab.filter_dates.is_empty() { ("Tout cocher", false) } else { ("Tout décocher", true) }
                                } else { ("Tout cocher", false) };

                                crate::ui::elements::button::Button::new(SharedString::from(format!("select-all-date-{}", pid_cap)))
                                    .label(label)
                                    .variant(crate::ui::elements::button::ButtonVariant::Subtle)
                                    .on_click(move |_, cx| {
                                        let pid_inner = pid_cap.clone();
                                        let _ = h_inner.update(cx, |this, cx| {
                                            if should_clear {
                                                this.clear_filter_dates(&pid_inner, cx);
                                            } else {
                                                if let Some(p) = this.state.find_pane_mut(&pid_inner) {
                                                    use crate::ui::workspace::DateCategory;
                                                    let tab = p.active_tab_mut();
                                                    tab.filter_dates.insert(DateCategory::Aujourdhui);
                                                    tab.filter_dates.insert(DateCategory::Hier);
                                                    tab.filter_dates.insert(DateCategory::CetteSemaine);
                                                    tab.filter_dates.insert(DateCategory::LaSemaineDerniere);
                                                    tab.filter_dates.insert(DateCategory::PlusAncien);
                                                }
                                            }
                                            this.context_menus.truncate(m_idx + 1);
                                            this.open_date_filter_menu(&pid_inner, pos, cx);
                                        });
                                    })
                                    .into_any_element()
                            }
                        }
                    });
                    
                    let dropdown_items = items.iter().map(|it| {
                        let on_click = it.on_click.clone();
                        crate::ui::components::dropdown::DropdownItem {
                            label: it.label.clone(),
                            icon: it.icon.clone(),
                            stay_open: it.stay_open,
                            has_submenu: it.has_submenu,
                            on_click: std::rc::Rc::new(move |window, app| { (on_click)(window, app); }),
                            on_hover: it.on_hover.clone(),
                        }
                    }).collect();

                    let items_cap = items.clone();
                    cx.new(|_| GenericDropdown {
                        h_app: h.clone(),
                        items: dropdown_items,
                        scroll_handle,
                        width: None,
                        max_height: 480.0,
                        on_close: {
                            let h = h.clone();
                            std::rc::Rc::new(move |_, cx| {
                                let _ = h.update(cx, |this, _| { this.context_menus.clear(); });
                            })
                        },
                        on_drag: {
                            std::rc::Rc::new(move |this, tid, vh, ch, _| {
                                this.drag_target = Some(DragTarget::ContextMenuV { menu_idx: m_idx, viewport_h: vh, content_h: ch, track_id: tid });
                            })
                        },
                        is_dragging: is_v_dragging,
                        header,
                        on_item_hover: Some(std::rc::Rc::new({
                            let h_app = h.clone();
                            let items_val = items_cap.clone();
                            let hovered_idx = std::rc::Rc::new(std::cell::Cell::new(None));
                            let window_handle = window_handle.clone();
                            move |ix, item_pos, cx| {
                                if hovered_idx.get() == Some(ix) { return; }
                                hovered_idx.set(Some(ix));

                                if let Some(it) = items_val.get(ix) {
                                    let has_submenu = it.on_hover.is_some();
                                    let h_inner = h_app.clone();
                                    let items_inner = items_val.clone();
                                    let ix_inner = ix;
                                    let pos_inner = item_pos;
                                    let window_handle_inner = window_handle.clone();

                                    if has_submenu {
                                        cx.defer(move |cx| {
                                            let _h_inner_spawn = h_inner.clone();
                                            let _ = h_inner.update(cx, |this, cx| {
                                                let parent_left = this.context_menus.get(m_idx)
                                                    .map(|m| m.position.x)
                                                    .unwrap_or_default();
                                                let parent_right = parent_left + this.context_menus.get(m_idx)
                                                    .map(|m| m.width)
                                                    .unwrap_or_default();
                                                this.submenu_close_timer = None;
                                                this.submenu_timer = Some(cx.spawn(move |_, cx: &mut AsyncApp| {
                                                    let mut cx_val = cx.clone();
                                                    async move {
                                                        cx_val.background_executor().timer(std::time::Duration::from_millis(150)).await;
                                                        let _ = window_handle_inner.update(&mut cx_val, |_, _window, app| {
                                                            let viewport_w = _window.viewport_size().width;
                                                            let open_left = parent_right + px(220.0) > viewport_w;
                                                            if let Some(it) = items_inner.get(ix_inner) {
                                                                if let Some(on_h) = it.on_hover.clone() {
                                                                    let mut sub_pos = pos_inner;
                                                                    sub_pos.x = if open_left {
                                                                        parent_left + px(2.0) // TopRight anchor: right edge at parent left
                                                                    } else {
                                                                        parent_right - px(2.0) // TopLeft anchor: left edge at parent right
                                                                    };
                                                                    sub_pos.y -= px(8.0);
                                                                    (on_h)(sub_pos, open_left, app);
                                                                }
                                                            }
                                                        });
                                                    }
                                                }));
                                            });
                                        });
                                    } else {
                                        // Item has no submenu → close any open children immediately
                                        cx.defer(move |cx| {
                                            let _ = h_inner.update(cx, |this, cx| {
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
                                        let _ = h_close.update(cx, |this, cx| {
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
                                let _ = h.update(cx, |this, _| {
                                    if let Some(menu) = this.context_menus.get_mut(m_idx) {
                                        menu.width = w;
                                    }
                                });
                            }
                        })),
                        on_hover_change: Some(std::rc::Rc::new({
                            let h = h.clone();
                            let window_handle = window_handle.clone();
                            move |is_hovering: bool, cx: &mut App| {
                                if is_hovering {
                                    let _ = h.update(cx, |this, _| { this.submenu_close_timer = None; });
                                } else {
                                    let h_close = h.clone();
                                    let wh = window_handle.clone();
                                    let _ = h.update(cx, |this, cx| {
                                        this.submenu_close_timer = Some(cx.spawn(move |_, cx: &mut AsyncApp| {
                                            let mut cx_val = cx.clone();
                                            async move {
                                                cx_val.background_executor().timer(std::time::Duration::from_millis(250)).await;
                                                let _ = wh.update(&mut cx_val, |_, _, cx| {
                                                    let h_inner = h_close.clone();
                                                    cx.defer(move |cx| {
                                                        let _ = h_inner.update(cx, |this, cx| {
                                                            if this.context_menus.len() > m_idx + 1 {
                                                                this.context_menus.truncate(m_idx + 1);
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
                    })
                },
                ContextMenuContent::NativeItems { path, is_background, items } => {
                    let path_cap = path.clone();
                    let is_bg_cap = *is_background;
                    let h_app = h.clone();
                    let items_cap = items.clone();

                    let dropdown_items = items.iter().map(|it| {
                        let h = h_app.clone();
                        let p = path_cap.clone();
                        let id = it.id;
                        let has_sub = it.has_submenu;
                        
                        crate::ui::components::dropdown::DropdownItem {
                            label: it.label.clone(),
                            icon: None,
                            stay_open: has_sub,
                            has_submenu: has_sub,
                            on_click: std::rc::Rc::new({
                                move |_, app| {
                                    let h_inner = h.clone();
                                    let p_inner = p.clone();
                                    let id_inner = id;
                                    
                                    let _ = h_inner.update(app, |this, cx| {
                                        if !has_sub {
                                            this.context_menus.clear();
                                            let _ = oxyde_lib::commands::system::execute_native_menu_item(p_inner, id_inner, is_bg_cap);
                                        }
                                        cx.notify();
                                    });
                                }
                            }),
                            on_hover: None,
                        }
                    }).collect();

                    cx.new(|_| GenericDropdown {
                        h_app: h.clone(),
                        items: dropdown_items,
                        scroll_handle,
                        width: None,
                        max_height: 480.0,
                        on_close: {
                            let h = h.clone();
                            std::rc::Rc::new(move |_, cx| {
                                let _ = h.update(cx, |this, _| { this.context_menus.clear(); });
                            })
                        },
                        on_drag: {
                            std::rc::Rc::new(move |this, tid, vh, ch, _| {
                                this.drag_target = Some(DragTarget::ContextMenuV { menu_idx: m_idx, viewport_h: vh, content_h: ch, track_id: tid });
                            })
                        },
                        is_dragging: is_v_dragging,
                        header: None,
                        on_item_hover: Some(std::rc::Rc::new({
                            let h_app = h.clone();
                            let items_val = items_cap.clone();
                            let hovered_idx = std::rc::Rc::new(std::cell::Cell::new(None));
                            let window_handle = window_handle.clone();
                            move |ix, item_pos, cx| {
                                if hovered_idx.get() == Some(ix) { return; }
                                hovered_idx.set(Some(ix));

                                if let Some(it) = items_val.get(ix) {
                                    let h_inner = h_app.clone();
                                    let path_inner = path_cap.clone();
                                    let sub_items = it.children.clone();
                                    let has_sub = it.has_submenu;
                                    let window_handle_inner = window_handle.clone();
                                    
                                    cx.defer(move |cx| {
                                        let h_inner_spawn = h_inner.clone();
                                        let _ = h_inner.update(cx, |this, cx| {
                                            if has_sub && !sub_items.is_empty() {
                                                let path_cap = path_inner.clone();
                                                let is_bg_cap = is_bg_cap;
                                                let sub_items_cap = sub_items.clone();
                                                let pos_inner = item_pos;
                                                let parent_left = this.context_menus.get(m_idx)
                                                    .map(|m| m.position.x)
                                                    .unwrap_or_default();
                                                let parent_right = parent_left + this.context_menus.get(m_idx)
                                                    .map(|m| m.width)
                                                    .unwrap_or_default();

                                                this.submenu_close_timer = None;
                                                this.submenu_timer = Some(cx.spawn(move |_, cx: &mut AsyncApp| {
                                                    let mut cx_val = cx.clone();
                                                    async move {
                                                        cx_val.background_executor().timer(std::time::Duration::from_millis(150)).await;
                                                        let _ = window_handle_inner.update(&mut cx_val, |_, _window, cx| {
                                                            let viewport_w = _window.viewport_size().width;
                                                            let open_left = parent_right + px(220.0) > viewport_w;
                                                            let app_handle = h_inner_spawn.clone();
                                                            let path_cap = path_cap.clone();
                                                            let sub_items_cap = sub_items_cap.clone();
                                                            let pos_inner = pos_inner;

                                                            cx.defer(move |cx| {
                                                                let _ = app_handle.update(cx, |this, cx| {
                                                                    let is_already_open = this.context_menus.get(m_idx + 1)
                                                                        .map(|m| matches!(&m.content, ContextMenuContent::NativeItems { items, .. } if items.first().map(|it| it.id) == sub_items_cap.first().map(|it| it.id)))
                                                                        .unwrap_or(false);

                                                                    if !is_already_open {
                                                                        this.context_menus.truncate(m_idx + 1);
                                                                        let mut new_pos = pos_inner;
                                                                        new_pos.x = if open_left {
                                                                            parent_left + px(2.0) // TopRight: right edge at parent left
                                                                        } else {
                                                                            parent_right - px(2.0) // TopLeft: left edge at parent right
                                                                        };
                                                                        new_pos.y -= px(8.0);

                                                                        this.context_menus.push(ContextMenuInstance {
                                                                            position: new_pos,
                                                                            open_left,
                                                                            content: ContextMenuContent::NativeItems {
                                                                                path: path_cap,
                                                                                is_background: is_bg_cap,
                                                                                items: sub_items_cap
                                                                            },
                                                                            scroll_handle: UniformListScrollHandle::new(),
                                                                            width: px(0.0),
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
                                let _ = h.update(cx, |this, _| {
                                    if let Some(menu) = this.context_menus.get_mut(m_idx) {
                                        menu.width = w;
                                    }
                                });
                            }
                        })),
                        on_hover_change: Some(std::rc::Rc::new({
                            let h = h.clone();
                            let window_handle = window_handle.clone();
                            move |is_hovering: bool, cx: &mut App| {
                                if is_hovering {
                                    let _ = h.update(cx, |this, _| { this.submenu_close_timer = None; });
                                } else {
                                    let h_close = h.clone();
                                    let wh = window_handle.clone();
                                    let _ = h.update(cx, |this, cx| {
                                        this.submenu_close_timer = Some(cx.spawn(move |_, cx: &mut AsyncApp| {
                                            let mut cx_val = cx.clone();
                                            async move {
                                                cx_val.background_executor().timer(std::time::Duration::from_millis(250)).await;
                                                let _ = wh.update(&mut cx_val, |_, _, cx| {
                                                    let h_inner = h_close.clone();
                                                    cx.defer(move |cx| {
                                                        let _ = h_inner.update(cx, |this, cx| {
                                                            if this.context_menus.len() > m_idx + 1 {
                                                                this.context_menus.truncate(m_idx + 1);
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
                    })
                },
                ContextMenuContent::NameFilter { input } => {
                    cx.new(|_| GenericDropdown {
                        h_app: h.clone(),
                        items: vec![],
                        scroll_handle,
                        width: None,
                        max_height: 480.0,
                        on_close: {
                            let h = h.clone();
                            std::rc::Rc::new(move |_, cx| {
                                let _ = h.update(cx, |this, _| { this.context_menus.clear(); });
                            })
                        },
                        on_drag: {
                            std::rc::Rc::new(move |this, tid, vh, ch, _| {
                                this.drag_target = Some(DragTarget::ContextMenuV { menu_idx: m_idx, viewport_h: vh, content_h: ch, track_id: tid });
                            })
                        },
                        is_dragging: false,
                        header: Some(input.clone().into_any_element()),
                        on_item_hover: None,
                        on_width_calculated: Some(std::rc::Rc::new({
                            let h = h.clone();
                            move |w, cx| {
                                let _ = h.update(cx, |this, _| {
                                    if let Some(menu) = this.context_menus.get_mut(m_idx) {
                                        menu.width = w;
                                    }
                                });
                            }
                        })),
                        on_hover_change: None,
                    })
                }
            };

            root = root.child(
                deferred(
                    if instance.open_left {
                        // Snap (no corner-switch) so the submenu extends left from parent_left
                        anchored()
                            .position(pos)
                            .anchor(Corner::TopRight)
                            .snap_to_window()
                            .child(content_element)
                    } else {
                        anchored()
                            .position(pos)
                            .child(content_element)
                    }
                )
            );
        }

        if let Some(req) = self.tooltip_request.as_ref() {
            root = root.child(ManualTooltipElement { data: req.clone() });
        }

        root
    }
}
