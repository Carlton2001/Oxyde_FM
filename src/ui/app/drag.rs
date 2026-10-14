use super::{DragTarget, FileDragPreview, OxydeApp, OxydeTheme, ThemeContext};
use crate::ui::i18n::{FluentArgs, I18nContext, Message};
use crate::ui::scale::{logical, rem_pixels};
use gpui::prelude::*;
use gpui::*;
#[cfg(target_os = "windows")]
mod popup;

fn actionable_drop_paths(mut paths: Vec<String>, target: &str) -> Vec<String> {
    use crate::ui::components::sidebar::sidebar_paths_equal;
    paths.retain(|path| {
        !sidebar_paths_equal(path, target)
            && !std::path::Path::new(path).parent().is_some_and(|parent| {
                sidebar_paths_equal(&parent.to_string_lossy(), target)
            })
    });
    paths
}

#[cfg(test)]
#[path = "drag/tests/drop_paths.rs"]
mod drop_paths_tests;

/// Returns true if `src` and `dst` are on different drives/volumes.
/// On Windows this compares the root prefix (e.g. "C:" vs "D:").
pub fn drives_differ(src: &str, dst: &str) -> bool {
    let src_root = std::path::Path::new(src).components().next();
    let dst_root = std::path::Path::new(dst).components().next();
    src_root != dst_root
}

impl FileDragPreview {
    pub fn new(
        paths: Vec<String>,
        cursor_offset: Point<Pixels>,
        app_handle: Entity<OxydeApp>,
        cx: &mut Context<Self>,
    ) -> Self {
        app_handle.update(cx, |app, _| {
            app.file_drag_origin = app.state.find_pane(&app.state.active_pane_id)
                .map(|pane| (pane.id.clone(), pane.active_tab().id.clone()));
        });
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
        let new_tab = self.app_handle.read(cx).drag_new_tab;
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
                        .when(new_tab, |row| row.child(drag_op_pill("", cx.i18n().text(Message::DragNewTab).as_ref(), true, &theme)))
                        .when(!new_tab, |row| row.child(drag_op_pill(
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
                        ))),
                ),
        )
    }
}

impl OxydeApp {
    /// End pointer interactions, including releases missed outside the window.
    pub(crate) fn finish_pointer_drag(&mut self, cx: &mut Context<Self>) {
        self.item_press = None;
        self.item_selection_drag = false;
        self.file_drag_origin = None;
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
        self.state.find_pane(&data.source_pane_id)
            .map(|pane| pane.tabs.iter().flat_map(|tab| tab.files.iter())
                .filter(|file| file.is_dir && data.paths.contains(&file.path))
                .map(|file| file.path.clone()).collect::<std::collections::BTreeSet<_>>()
                .into_iter().collect()).unwrap_or_default()
    }

    pub(crate) fn execute_file_drop(&mut self, paths: Vec<String>, target: String, cx: &mut Context<Self>) {
        let paths = actionable_drop_paths(paths, &target);
        if self.drag_copy_mode {
            self.execute_file_copy(paths, target, cx);
        } else {
            self.execute_file_move(paths, target, cx);
        }
    }

    pub(super) fn handle_pointer_move(&mut self, e: &MouseMoveEvent, _window: &mut Window, cx: &mut Context<Self>) {
        if !e.dragging() {
            self.finish_pointer_drag(cx);
            return;
        }
        self.move_item_press(e, cx);
        if let Some(target) = self.drag_target.clone() {
            let pos = e.position;
            match target {
                DragTarget::SidebarV {
                    viewport_h,
                    content_h,
                    track_id,
                } => {
                    if let Some(origin) = self.scrollbar_origins.get(&track_id) {
                        let mouse_pos_in_track = pos.y.as_f32() - origin.y;
                        let thumb_size = (viewport_h
                            * (viewport_h / content_h.max(1.0)).min(1.0))
                        .max(logical(40.0));
                        let travel_distance = (viewport_h - thumb_size).max(1.0);
                        let content_range = content_h - viewport_h;
                        let target_thumb_pos = (mouse_pos_in_track
                            - self.grab_offset)
                            .clamp(0.0, travel_distance);
                        let target_px =
                            (target_thumb_pos / travel_distance) * content_range;
                        self.sidebar_scroll
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
                    if let Some(origin) = self.scrollbar_origins.get(&track_id) {
                        let mouse_pos_in_track = pos.x.as_f32() - origin.x;
                        let thumb_size = (viewport_w
                            * (viewport_w / content_w.max(1.0)).min(1.0))
                        .max(logical(40.0));
                        let travel_distance = (viewport_w - thumb_size).max(1.0);
                        let content_range = content_w - viewport_w;
                        let target_thumb_pos = (mouse_pos_in_track
                            - self.grab_offset)
                            .clamp(0.0, travel_distance);
                        let target_px =
                            (target_thumb_pos / travel_distance) * content_range;
                        self.sidebar_scroll_h
                            .set_offset(Point::new(px(-target_px), px(0.0)));
                        cx.notify();
                    }
                }
                DragTarget::SidebarResize => {
                    self.resize_sidebar(pos.x.as_f32() - self.grab_offset);
                    cx.notify();
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
                            let mouse_pos_in_track = pos.y.as_f32() - origin.y;
                            let thumb_size = (viewport_h
                                * (viewport_h / content_h.max(1.0)).min(1.0))
                            .max(logical(40.0));
                            let travel_distance =
                                (viewport_h - thumb_size).max(1.0);
                            let content_range = content_h - viewport_h;
                            let target_thumb_pos = (mouse_pos_in_track
                                - self.grab_offset)
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
                    if let Some(origin) = self.scrollbar_origins.get(&track_id) {
                        if let Some(pane) =
                            self.state.layout.find_pane_mut(&pane_id)
                        {
                            let mouse_pos_in_track = pos.x.as_f32() - origin.x;
                            let thumb_size = (viewport_w
                                * (viewport_w / content_w.max(1.0)).min(1.0))
                            .max(logical(40.0));
                            let travel_distance =
                                (viewport_w - thumb_size).max(1.0);
                            let content_range = content_w - viewport_w;
                            let target_thumb_pos = (mouse_pos_in_track
                                - self.grab_offset)
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
                    if let Some(origin) = self.scrollbar_origins.get(&track_id) {
                        let mouse_pos_in_track = pos.y.as_f32() - origin.y;
                        let thumb_size = (viewport_h
                            * (viewport_h / content_h.max(1.0)).min(1.0))
                        .max(logical(40.0));
                        let travel_distance = (viewport_h - thumb_size).max(1.0);
                        let content_range = content_h - viewport_h;
                        let target_thumb_pos = (mouse_pos_in_track
                            - self.grab_offset)
                            .clamp(0.0, travel_distance);
                        let target_px =
                            (target_thumb_pos / travel_distance) * content_range;
                        self.breadcrumb_scroll
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
                    if let Some(origin) = self.scrollbar_origins.get(&track_id) {
                        if let Some(menu) = self.context_menus.get(menu_idx) {
                            let mouse_pos_in_track = pos.y.as_f32() - origin.y;
                            let thumb_size = (viewport_h
                                * (viewport_h / content_h.max(1.0)).min(1.0))
                            .max(logical(40.0));
                            let travel_distance =
                                (viewport_h - thumb_size).max(1.0);
                            let content_range = content_h - viewport_h;
                            let target_thumb_pos = (mouse_pos_in_track
                                - self.grab_offset)
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
                    if let Some(origin) = self.scrollbar_origins.get(&track_id) {
                        let mouse_pos_in_track = pos.y.as_f32() - origin.y;
                        let thumb_size = (viewport_h
                            * (viewport_h / content_h.max(1.0)).min(1.0))
                        .max(logical(40.0));
                        let travel_distance = (viewport_h - thumb_size).max(1.0);
                        let content_range = content_h - viewport_h;
                        let target_thumb_pos = (mouse_pos_in_track
                            - self.grab_offset)
                            .clamp(0.0, travel_distance);
                        let target_px =
                            (target_thumb_pos / travel_distance) * content_range;
                        self.operations_panel_scroll
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
                    self.drag_target = Some(DragTarget::SelectionRectangle {
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
    }
}
