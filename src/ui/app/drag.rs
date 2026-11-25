use super::{DragTarget, FileDragPreview, OxydeApp, OxydeTheme, ThemeContext};
use crate::ui::i18n::{FluentArgs, I18nContext, Message};
use crate::ui::scale::rem_pixels;
use crate::ui::components::scrollbar::drag_scrolled;
use gpui::prelude::*;
use gpui::*;
mod popup;
pub(super) mod external;
pub(crate) mod outgoing;
#[cfg(target_os = "windows")]
pub(super) mod native;
pub(super) mod volumes;

fn actionable_drop_paths(mut paths: Vec<String>, target: &str) -> Vec<String> {
    use crate::ui::paths::paths_match;
    paths.retain(|path| {
        !paths_match(path, target)
            && !std::path::Path::new(path).parent().is_some_and(|parent| {
                paths_match(&parent.to_string_lossy(), target)
            })
    });
    paths
}

#[cfg(test)]
#[path = "drag/tests/drop_paths.rs"]
mod drop_paths_tests;

/// New weight of the left pane of a pair, keeping both at 0.1 or more.
/// A pair lighter than 0.2 is split in half instead of panicking in `clamp`.
fn resized_pair_weight(initial: f32, pair_total: f32, delta: f32) -> f32 {
    const MIN: f32 = 0.1;
    if pair_total <= 2.0 * MIN {
        return pair_total / 2.0;
    }
    (initial + delta).clamp(MIN, pair_total - MIN)
}

#[cfg(test)]
#[path = "drag/tests/pane_weights.rs"]
mod pane_weight_tests;

fn drag_copy_override(modifiers: Modifiers) -> Option<bool> {
    if modifiers.control {
        Some(true)
    } else if modifiers.shift {
        Some(false)
    } else {
        None
    }
}

#[cfg(test)]
#[path = "drag/tests/modifiers.rs"]
mod modifier_tests;

#[cfg(all(test, target_os = "windows"))]
#[path = "drag/tests/native_input.rs"]
mod native_input_tests;

/// Returns true if `src` and `dst` are on different drives/volumes.
/// On Windows this compares the drive or share (`\\?\` forms included), then the
/// volumes mounted in folders.
#[cfg(target_os = "windows")]
pub fn drives_differ(src: &str, dst: &str) -> bool {
    volumes::roots_differ(src, dst) || volumes::mounts_differ(src, dst)
}

/// Every absolute Linux path starts with `/`: compare mount points instead.
#[cfg(not(target_os = "windows"))]
pub fn drives_differ(src: &str, dst: &str) -> bool {
    volumes::mounts_differ(src, dst)
}

impl FileDragPreview {
    pub fn new(
        paths: Vec<String>,
        cursor_offset: Point<Pixels>,
        single_is_dir: bool,
        app_handle: Entity<OxydeApp>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        crate::ui::components::tooltip::hover::dismiss(window, cx);
        app_handle.update(cx, |app, cx| {
            // Modifier events follow the focus path, including with a stationary pointer.
            window.focus(&app.focus_handle, cx);
            app.sync_pointer_modifiers(window.modifiers(), cx);
            // A completed drag must never become a filename rename click.
            app.suppress_item_selection_click = true;
            app.rename_click_candidate = None;
            app.file_drag_origin = app.state.find_pane(&app.state.active_pane_id)
                .map(|pane| (pane.id.clone(), pane.active_tab().id.clone()));
        });
        let sub = cx.observe(&app_handle, |_, _entity, cx| cx.notify());
        #[cfg(target_os = "windows")]
        native::begin(paths.clone(), single_is_dir, app_handle.clone(), window, cx);
        Self {
            paths,
            single_is_dir,
            cursor_offset,
            app_handle,
            _subscription: sub,
            #[cfg(test)]
            rendered_copy_mode: None,
        }
    }
}

/// One action of the ghost, set as a shortcut in a tooltip: the active one takes the
/// selected segment's fill and accent, its key stays muted.
fn drag_op_pill(key_hint: &str, label: &str, active: bool, theme: &OxydeTheme) -> impl IntoElement {
    use crate::ui::components::tooltip::DETAIL_SIZE;
    div()
        .flex()
        .items_center()
        .gap(rems(0.375))
        .px(rems(0.375))
        .py(rems(0.125))
        .rounded(rems(0.25))
        .text_size(DETAIL_SIZE)
        .when(active, |pill| pill.bg(theme.selected))
        .child(
            div()
                .text_color(if active { theme.accent } else { theme.text_muted })
                .when(active, |label| label.font_weight(FontWeight::MEDIUM))
                .child(label.to_string()),
        )
        .when(!key_hint.is_empty(), |pill| {
            pill.child(div().text_color(theme.text_muted).child(key_hint.to_string()))
        })
}

impl Render for FileDragPreview {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let is_copy = self.app_handle.read(cx).drag_copy_mode;
        #[cfg(test)]
        { self.rendered_copy_mode = Some(is_copy); }
        let new_tab = self.app_handle.read(cx).drag_new_tab;
        let refused = self.app_handle.read(cx).drag_target_refused;
        #[cfg(target_os = "windows")]
        if native::has_ghost(cx) { return div().into_any_element(); }
        let offset = self.cursor_offset;
        div().relative().child(ghost_content(&self.paths, self.single_is_dir, is_copy, new_tab, refused, cx)
            .absolute().left(offset.x + rem_pixels(rems(0.75))).top(offset.y + rem_pixels(rems(0.25))))
            .into_any_element()
    }
}

fn ghost_content(paths: &[String], single_is_dir: bool, is_copy: bool, new_tab: bool, unavailable: bool, cx: &mut App) -> Div {
        let theme = *cx.theme();

        let (icon_path, name_label) = if paths.len() == 1 {
            let p = std::path::Path::new(&paths[0]);
            let is_dir = single_is_dir;
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
                args.set("count", paths.len());
                args.set(
                    "number",
                    crate::ui::format::format_number(paths.len() as u64, cx.i18n().language()),
                );
                cx.i18n()
                    .text_with_args(Message::DragItems, &args)
                    .to_string()
            })
        };

    // The tooltips' frame and type, lifted by the same shadow.
    use crate::ui::components::tooltip::{frame, TITLE_SIZE};
    frame(theme)
        .font_family("Inter")
        .font_weight(crate::ui::style::DEFAULT_FONT_WEIGHT)
        .line_height(relative(1.25))
        .px(rems(0.75))
        .py(rems(0.5))
        .flex()
        .flex_col()
        .gap(rems(0.375))
        .whitespace_nowrap()
        .child(
            div()
                .flex()
                .items_center()
                .gap(rems(0.5))
                .child(svg().path(icon_path).size(rems(1.0)).flex_none().text_color(theme.accent))
                .child(
                    div()
                        .max_w(rems(24.0))
                        .truncate()
                        .text_color(theme.text)
                        .text_size(TITLE_SIZE)
                        .font_weight(FontWeight::MEDIUM)
                        .child(name_label),
                ),
        )
        .child(
            div()
                .flex()
                .gap(rems(0.25))
                .ml(rems(-0.375))
                .when(unavailable, |row| row.child(drag_op_pill("", cx.i18n().text(Message::DragUnavailable).as_ref(), false, &theme)))
                .when(new_tab && !unavailable, |row| row.child(drag_op_pill("", cx.i18n().text(Message::DragNewTab).as_ref(), true, &theme)))
                .when(!new_tab && !unavailable, |row| row.child(drag_op_pill(
                    "Ctrl",
                    cx.i18n().text(Message::DragCopy).as_ref(),
                    is_copy,
                    &theme,
                )))
                .when(!unavailable, |row| row.child(drag_op_pill(
                    "Shift",
                    cx.i18n().text(Message::DragMove).as_ref(),
                    !is_copy,
                    &theme,
                ))),
        )
}

impl OxydeApp {
    pub(super) fn sync_pointer_modifiers(&mut self, modifiers: Modifiers, cx: &mut Context<Self>) {
        if self.shift_delete != modifiers.shift {
            self.shift_delete = modifiers.shift;
            cx.notify();
        }
        let copy_override = drag_copy_override(modifiers);
        if self.drag_modifier_copy != copy_override {
            self.drag_modifier_copy = copy_override;
            self.update_drag_copy_mode(cx);
        }
    }

    /// End pointer interactions, including releases missed outside the window.
    pub(crate) fn finish_pointer_drag(&mut self, cx: &mut Context<Self>) {
        #[cfg(target_os = "windows")]
        if !self.file_drag_session.as_ref().is_some_and(|session| session.native) {
            self.file_drag_session = None;
            native::close(cx);
        }
        self.item_press = None;
        self.item_selection_drag = false;
        self.selection_scroll_y = None;
        self.selection_autoscroll_at = None;
        self.selection_autoscroll_task = None;
        self.file_drag_origin = None;
        self.drag_target_refused = false;
        self.set_file_drag_new_tab(false, cx);
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

    /// What the hovered drop target decides for an internal drag: the default operation
    /// and whether it takes nothing. Returns true when it refuses, so it takes no highlight.
    /// Nothing is dropped where nothing can be written: a read-only volume or archive,
    /// the Trash, the network list, or search results.
    pub(crate) fn refuses_drop(&self, target: &str) -> bool {
        matches!(target, "trash://" | "__network_vincinity__")
            || target.starts_with("search://")
            || self.is_read_only_path(target)
    }

    pub(crate) fn note_drag_target(&mut self, paths: &[String], target: &str, cx: &mut Context<Self>) -> bool {
        let refused = self.refuses_drop(target);
        let default_copy = paths.first().is_some_and(|path| drives_differ(path, target));
        if self.drag_target_refused != refused {
            self.drag_target_refused = refused;
            cx.notify();
        }
        if self.drag_target_default_copy != default_copy {
            self.drag_target_default_copy = default_copy;
            self.update_drag_copy_mode(cx);
        }
        refused
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
}

impl OxydeApp {
    pub(crate) fn cancel_file_drag(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if self.file_drag_origin.is_none() || !cx.has_active_drag() {
            return false;
        }
        let origin = self.file_drag_origin.take();
        cx.stop_active_drag(window);
        self.finish_pointer_drag(cx);
        if let Some((pane_id, tab_id)) = origin {
            let index = self.state.find_pane(&pane_id)
                .and_then(|pane| pane.tabs.iter().position(|tab| tab.id == tab_id));
            if let Some(index) = index {
                self.switch_tab(&pane_id, index, cx);
                window.focus(&self.focus_handle, cx);
            }
        }
        cx.notify();
        true
    }

    pub(crate) fn set_file_drag_new_tab(&mut self, value: bool, cx: &mut Context<Self>) {
        if self.drag_new_tab != value {
            self.drag_new_tab = value;
            cx.notify();
        }
    }

    pub(crate) fn tab_drop_directories(&self, data: &super::FileDragData) -> Vec<String> {
        let dragged: std::collections::HashSet<&str> = data.paths.iter().map(String::as_str).collect();
        self.state.find_pane(&data.source_pane_id)
            .map(|pane| pane.tabs.iter().flat_map(|tab| tab.files.iter())
                .filter(|file| file.is_dir && dragged.contains(file.path.as_str()))
                .map(|file| file.path.clone()).collect::<std::collections::BTreeSet<_>>()
                .into_iter().collect()).unwrap_or_default()
    }

    pub(crate) fn execute_file_drop(&mut self, paths: Vec<String>, target: String, cx: &mut Context<Self>) {
        if self.refuses_drop(&target) {
            return;
        }
        let paths = actionable_drop_paths(paths, &target);
        if self.drag_copy_mode {
            self.execute_file_copy(paths, target, cx);
        } else {
            self.execute_file_move(paths, target, cx);
        }
    }

    /// Moves the list whose thumb `target` drags so the thumb follows `pos`, in owner
    /// coordinates. Shared by pointer moves and the press that starts the drag.
    pub(crate) fn move_scrollbar_thumb(&mut self, target: &DragTarget, pos: Point<Pixels>) -> bool {
        match target.clone() {
        DragTarget::SidebarV {
            viewport_h,
            content_h,
            track_id,
        } => {
            if let Some(origin) = self.scrollbar_origins.get(&track_id) {
                let target_px = drag_scrolled(viewport_h, content_h, viewport_h, pos.y.as_f32() - origin.y, self.grab_offset);
                self.sidebar_scroll
                    .0
                    .borrow()
                    .base_handle
                    .set_offset(Point::new(px(0.0), px(-target_px)));
                return true;
            }
        }
        DragTarget::SidebarH {
            viewport_w,
            content_w,
            track_id,
        } => {
            if let Some(origin) = self.scrollbar_origins.get(&track_id) {
                let target_px = drag_scrolled(viewport_w, content_w, viewport_w, pos.x.as_f32() - origin.x, self.grab_offset);
                self.sidebar_scroll_h
                    .set_offset(Point::new(px(-target_px), px(0.0)));
                return true;
            }
        }
        DragTarget::FileListV {
            pane_id,
            viewport_h,
            content_h,
            track_id,
        } => {
            if let Some(origin) = self.scrollbar_origins.get(&track_id) {
                if let Some(pane) =
                    self.state.layout.find_pane_mut(&pane_id)
                {
                    let target_px = drag_scrolled(viewport_h, content_h, viewport_h, pos.y.as_f32() - origin.y, self.grab_offset);
                    pane.active_tab_mut()
                        .list_scroll
                        .0
                        .borrow()
                        .base_handle
                        .set_offset(Point::new(px(0.0), px(-target_px)));
                    return true;
                }
            }
        }
        DragTarget::FileListH {
            pane_id,
            viewport_w,
            content_w,
            track_id,
        } => {
            if let Some(origin) = self.scrollbar_origins.get(&track_id) {
                if let Some(pane) =
                    self.state.layout.find_pane_mut(&pane_id)
                {
                    let target_px = drag_scrolled(viewport_w, content_w, viewport_w, pos.x.as_f32() - origin.x, self.grab_offset);
                    pane.active_tab_mut()
                        .list_scroll_h
                        .set_offset(Point::new(px(-target_px), px(0.0)));
                    return true;
                }
            }
        }
        DragTarget::BreadcrumbV {
            viewport_h,
            content_h,
            track_id,
        } => {
            if let Some(origin) = self.scrollbar_origins.get(&track_id) {
                let target_px = drag_scrolled(viewport_h, content_h, viewport_h, pos.y.as_f32() - origin.y, self.grab_offset);
                self.breadcrumb_scroll
                    .0
                    .borrow()
                    .base_handle
                    .set_offset(Point::new(px(0.0), px(-target_px)));
                return true;
            }
        }
        DragTarget::ContextMenuV {
            menu_idx,
            viewport_h,
            content_h,
            track_id,
        } => {
            if let Some(origin) = self.scrollbar_origins.get(&track_id) {
                if let Some(menu) = self.context_menus.get(menu_idx) {
                    let target_px = drag_scrolled(viewport_h, content_h, viewport_h, pos.y.as_f32() - origin.y, self.grab_offset);
                    menu.scroll_handle
                        .0
                        .borrow()
                        .base_handle
                        .set_offset(Point::new(px(0.0), px(-target_px)));
                    return true;
                }
            }
        }
            _ => {}
        }
        false
    }

    pub(super) fn handle_pointer_move(&mut self, e: &MouseMoveEvent, _window: &mut Window, cx: &mut Context<Self>) {
        // Mouse events also carry current modifiers when a key event was missed.
        self.sync_pointer_modifiers(e.modifiers, cx);
        if !e.dragging() {
            self.finish_pointer_drag(cx);
            return;
        }
        #[cfg(target_os = "windows")]
        self.promote_file_drag(_window, cx);
        self.move_item_press(e, cx);
        if let Some(target) = self.drag_target.clone() {
            let pos = e.position;
            match target {
                DragTarget::SidebarResize => {
                    self.resize_sidebar(pos.x.as_f32() - self.grab_offset);
                    cx.notify();
                }
                DragTarget::SidebarV { .. } | DragTarget::SidebarH { .. } | DragTarget::FileListV { .. }
                | DragTarget::FileListH { .. } | DragTarget::BreadcrumbV { .. }
                | DragTarget::ContextMenuV { .. } => {
                    if self.move_scrollbar_thumb(&target, pos) {
                        cx.notify();
                    }
                }
                DragTarget::PaneResize {
                    split_id,
                    child_idx,
                    initial_weights,
                } => {
                    if let Some(total_size) =
                        self.split_sizes.get(&split_id).cloned()
                    {
                        if let Some((grid_handle, weights)) =
                            self.state.layout.find_split_mut(&split_id)
                        {
                            let current_pos = match grid_handle {
                                crate::ui::workspace::LayoutAxis::Horizontal => {
                                    pos.x.as_f32()
                                }
                                crate::ui::workspace::LayoutAxis::Vertical => {
                                    pos.y.as_f32()
                                }
                            };
                            let delta = current_pos - self.grab_offset;
                            let total_weight: f32 = initial_weights.iter().sum();
                            let weight_per_pixel =
                                total_weight / total_size.max(1.0);
                            let delta_weight = delta * weight_per_pixel;

                            let i = child_idx;
                            let j = i + 1;

                            if i < weights.len() && j < weights.len() {
                                let total_pair_weight =
                                    initial_weights[i] + initial_weights[j];
                                let new_wi = resized_pair_weight(
                                    initial_weights[i],
                                    total_pair_weight,
                                    delta_weight,
                                );
                                let new_wj = total_pair_weight - new_wi;

                                weights[i] = new_wi;
                                weights[j] = new_wj;
                                cx.notify();
                            }
                        }
                    }
                }
                DragTarget::SelectionRectangle { .. } => {
                    if let Some(DragTarget::SelectionRectangle { current_pos, .. }) = &mut self.drag_target {
                        *current_pos = pos;
                    }
                    cx.notify();
                }
            }
        }
    }
}

/// Hover time over a tab before a drag activates it.
const DRAG_TAB_SWITCH_DELAY: std::time::Duration = std::time::Duration::from_millis(600);

/// A tab waiting for drag activation; dropping it cancels its timer.
pub(crate) struct DragTabSwitch {
    pane_id: String,
    tab_ix: usize,
    _timer: Task<()>,
}

impl DragTabSwitch {
    fn targets(&self, pane_id: &str, tab_ix: usize) -> bool {
        self.pane_id == pane_id && self.tab_ix == tab_ix
    }
}

impl OxydeApp {
    /// Activates the hovered tab once the drag pointer has rested on it long enough.
    pub(crate) fn hover_tab_during_drag(&mut self, pane_id: &str, tab_ix: usize, cx: &mut Context<Self>) {
        let already_active = self.state.active_pane_id == pane_id
            && self.state.find_pane(pane_id).is_some_and(|pane| pane.active_tab_index == tab_ix);
        if already_active {
            self.drag_tab_switch = None;
            return;
        }
        if self.drag_tab_switch.as_ref().is_some_and(|switch| switch.targets(pane_id, tab_ix)) {
            return;
        }
        let pane = pane_id.to_owned();
        let timer = cx.spawn(async move |this, cx| {
            cx.background_executor().timer(DRAG_TAB_SWITCH_DELAY).await;
            let _ = this.update(cx, |this, cx| {
                this.drag_tab_switch = None;
                if cx.has_active_drag() {
                    this.switch_tab(&pane, tab_ix, cx);
                }
            });
        });
        self.drag_tab_switch = Some(DragTabSwitch { pane_id: pane_id.to_owned(), tab_ix, _timer: timer });
    }

    /// Cancels a pending drag activation when the pointer leaves that tab.
    pub(crate) fn leave_tab_during_drag(&mut self, pane_id: &str, tab_ix: usize) {
        if self.drag_tab_switch.as_ref().is_some_and(|switch| switch.targets(pane_id, tab_ix)) {
            self.drag_tab_switch = None;
        }
    }
}
