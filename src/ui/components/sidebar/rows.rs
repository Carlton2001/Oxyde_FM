use crate::ui::components::dropdown::{DropdownItem, IconName};
use super::menus::{directory_context_menu, drive_context_menu, location_context_menu, pinned_context_menu};
use super::{pinned_display_name, sidebar_paths_equal, sidebar_item_height, sidebar_row_h};
use crate::ui::app::{ContextMenuContent, ContextMenuInstance, FileDragData, FileDragPreview, OxydeApp};
use crate::ui::components::tooltip::Tooltip;
use crate::ui::components::tooltip::hover::{HoverRegion, SharedTooltipExt};
use crate::ui::i18n::{I18nContext, Message};
use crate::ui::scale::logical;
use crate::ui::theme::OxydeTheme;
use crate::ui::workspace::SidebarItem;
use gpui::prelude::*;
use gpui::*;
use std::collections::HashSet;

pub(super) struct SidebarRowPresentation {
    pub(super) content_w: f32,
    pub(super) pinned_paths: HashSet<String>,
    pub(super) expandable_drives: HashSet<String>,
    pub(super) show_tooltips: bool,
    pub(super) use_system_icons: bool,
}

pub(super) fn render_sidebar_rows(
    range: std::ops::Range<usize>,
    cx: &mut App,
    items_for_list: &[SidebarItem],
    h: &Entity<OxydeApp>,
    sel: &Option<String>,
    presentation: &SidebarRowPresentation,
) -> Vec<AnyElement> {
    let SidebarRowPresentation {
        content_w, pinned_paths: pinned_paths_for_list, expandable_drives,
        show_tooltips, use_system_icons,
    } = presentation;
    let (content_w, show_tooltips, use_system_icons) = (*content_w, *show_tooltips, *use_system_icons);
    let items  = items_for_list.to_vec();
    let sel    = sel.clone();
    range.map(|i| {
        let theme    = *OxydeTheme::global(cx);
        let item     = &items[i];
        let h        = h.clone();

        match item {
            // ── Section header ─────────────────────────────
            SidebarItem::Header(_) => {
                div()
                    .w(px((content_w - logical(26.0)).max(1.0)))
                    .h(px(sidebar_row_h()))
                    .px_3()
                    .flex()
                    .items_center()
                    .child(div().flex_1().h(rems(0.0625)).flex_shrink_0().bg(theme.border))
                    .into_any_element()
            }

            // ── Quick-access (pinned) item ─────────────────
            SidebarItem::Pinned { item } => {
                let path      = item.path.clone();
                let is_sel    = sel.as_deref().is_some_and(|selected| sidebar_paths_equal(selected, &path));
                let sel_bg:   Hsla = theme.selected;
                let hov_bg:   Hsla = if is_sel { theme.selection_hover } else { theme.hover };
                let is_dimmed = item.name.starts_with('.');

                div()
                    .id(SharedString::from(format!("sb-pin-{}", path)))
                    .w(px((content_w - logical(26.0)).max(1.0)))
                    .h(px(sidebar_row_h()))
                    .px_3()
                    .flex()
                    .items_center()
                    .when(is_dimmed, |d| d.opacity(0.5))
                    .relative().when(is_sel, |d| d.bg(sel_bg).child(div().absolute().left_0().top_0().bottom_0().w(rems(0.1875)).bg(theme.accent)))
                    .hover(move |s| s.bg(hov_bg))
                    .when(show_tooltips, |d| {
                        let name = pinned_display_name(item, cx.i18n());
                        if path == "__network_vincinity__" || path == "trash://" {
                            d.shared_tooltip(h.clone(), HoverRegion::DirectoryTree, format!("pinned:{path}"), cx, Tooltip::text(name))
                        } else {
                            d.shared_tooltip(h.clone(), HoverRegion::DirectoryTree, format!("pinned:{path}"), cx, Tooltip::with_meta(name, path.clone()))
                        }
                    })
                    // Left click → select + navigate
                    .on_mouse_down(MouseButton::Left, {
                        let h = h.clone(); let p = path.clone();
                        move |_, _, cx| {
                            h.update(cx, |this, cx| {
                                this.sidebar_selected_path = Some(p.clone());
                                let id = this.state.active_pane_id.clone();
                                this.navigate_to(&id, p.clone(), cx);
                            });
                        }
                    })
                    // Middle click → open in new tab
                    .on_mouse_down(MouseButton::Middle, {
                        let h = h.clone(); let p = path.clone();
                        move |_, _, cx| {
                            h.update(cx, |this, cx| {
                                let id = this.state.active_pane_id.clone();
                                this.add_tab(&id, p.clone(), false, cx);
                            });
                        }
                    })
                    // Right click → select + context menu
                    .on_mouse_down(MouseButton::Right, {
                        let h = h.clone(); let p = path.clone();
                        let is_qa = pinned_paths_for_list.iter().any(|pinned| super::super::paths_match(pinned, &path));
                        move |e, _, cx| {
                            let pos   = e.position;
                            let items = if is_qa {
                                pinned_context_menu(cx.i18n(), h.clone(), p.clone())
                            } else {
                                location_context_menu(cx.i18n(), h.clone(), p.clone())
                            };
                            h.update(cx, |this, cx| {
                                this.sidebar_selected_path = Some(p.clone());
                                this.context_menus.clear();
                                this.context_menus.push(ContextMenuInstance {
            native_bounds: None,
                                    position: pos,
                                    content: ContextMenuContent::Items(items, None),
                                    scroll_handle: UniformListScrollHandle::new(),
                                    width: px(0.0),
                                    bottom_padding: 3.0,
                                    open_left: false,
                                    hovered_item: None,
                                    native_request_id: None,
                                });
                                cx.notify();
                            });
                        }
                    })
                    .child(
                        div().w_4().h_4().mr_2().flex().items_center().justify_center().flex_shrink_0()
                            .child({
                                let icon_path = match path.as_str() {
                                    "__network_vincinity__" => "icons/globe.svg",
                                    "trash://" => "icons/trash-sidebar.svg",
                                    _ => "icons/pin.svg",
                                };
                                svg().path(icon_path).size_4().text_color(if path == "__network_vincinity__" || path == "trash://" { theme.text } else { theme.accent })
                            })
                    )
                    .child(
                        div().flex_1().text_ellipsis().overflow_hidden()
                            .text_color(theme.text)
                            .when(item.path == "__network_vincinity__" || item.path == "trash://", |d| d.font_weight(FontWeight::SEMIBOLD))
                            .child(pinned_display_name(item, cx.i18n()))
                    )
                    .drag_over::<FileDragData>({
                        let h = h.clone(); let tgt = path.clone();
                        move |style, data, _, cx| {
                            let default_copy = data.paths.first()
                                .map(|s| crate::ui::app::drives_differ(s, &tgt))
                                .unwrap_or(false);
                            h.update(cx, |this, cx| {
                                if this.drag_target_default_copy != default_copy {
                                    this.drag_target_default_copy = default_copy;
                                    this.update_drag_copy_mode(cx);
                                }
                            });
                            style.bg(theme.drag_hover)
                        }
                    })
                    .on_drop({
                        let h = h.clone(); let tgt = path.clone();
                        move |data: &FileDragData, _, cx| {
                            let paths = data.paths.clone();
                            h.update(cx, |this, cx| {
                                this.execute_file_drop(paths, tgt.clone(), cx);
                            });
                        }
                    })
                    .into_any_element()
            }

            // ── Drive item ─────────────────────────────────
            SidebarItem::Drive { drive, is_expanded } => {
                let path       = drive.path.clone();
                let is_usb = drive.is_usb();
                let is_remote = drive.drive_type == "remote";
                let has_subs = expandable_drives.contains(&path);
                let is_sel     = sel.as_deref().is_some_and(|selected| sidebar_paths_equal(selected, &path));
                let is_exp     = *is_expanded;
                let sel_bg:    Hsla = theme.selected;
                let hov_bg:    Hsla = if is_sel { theme.selection_hover } else { theme.hover };
                let label      = crate::ui::format::drive_label(&drive.label, &drive.path, cx.i18n());

                div()
                    .id(SharedString::from(format!("sb-drv-{}", path)))
                    .w(px((content_w - logical(26.0)).max(1.0)))
                    .h(px(sidebar_item_height(item)))
                    .px_3()
                    .flex()
                    .items_center()
                    .relative().when(is_sel, |d| d.bg(sel_bg).child(div().absolute().left_0().top_0().bottom_0().w(rems(0.1875)).bg(theme.accent)))
                    .hover(move |s| s.bg(hov_bg))
                    .when(show_tooltips, |d| d.shared_tooltip(h.clone(), HoverRegion::DirectoryTree, format!("drive:{path}"), cx, Tooltip::for_drive(label.clone(), drive.clone())))
                    // Left click → select + navigate (single) / toggle (double)
                    .on_mouse_down(MouseButton::Left, {
                        let h = h.clone(); let p = path.clone();
                        move |e, _, cx| {
                            h.update(cx, |this, cx| {
                                this.sidebar_selected_path = Some(p.clone());
                                if e.click_count == 2 && has_subs {
                                    this.toggle_sidebar_path(p.clone(), cx);
                                } else {
                                    let id = this.state.active_pane_id.clone();
                                    this.navigate_to(&id, p.clone(), cx);
                                }
                            });
                        }
                    })
                    // Middle click → open in new tab
                    .on_mouse_down(MouseButton::Middle, {
                        let h = h.clone(); let p = path.clone();
                        move |_, _, cx| {
                            h.update(cx, |this, cx| {
                                let id = this.state.active_pane_id.clone();
                                this.add_tab(&id, p.clone(), false, cx);
                            });
                        }
                    })
                    // Right click → select + context menu
                    .on_mouse_down(MouseButton::Right, {
                        let h = h.clone(); let p = path.clone();
                        move |e, _, cx| {
                            let pos   = e.position;
                            let mut items = drive_context_menu(cx.i18n(), h.clone(), p.clone(), is_usb, is_remote);
                            if let Some(image) = h.read(cx).mounted_image_for_drive(&p) {
                                let image_handle = h.clone();
                                let action = DropdownItem::new(cx.i18n().text(Message::UnmountDiskImage).to_string(), move |_, cx| {
                                    image_handle.update(cx, |this, cx| this.change_disk_image(image.clone(), true, None, cx));
                                }).icon(IconName::ExternalLink);
                                items = super::super::context_menu_sections(super::super::network::insert_after_open(items, vec![action], cx.i18n()), cx.i18n());
                            }
                            h.update(cx, |this, cx| {
                                this.sidebar_selected_path = Some(p.clone());
                                this.context_menus.clear();
                                this.context_menus.push(ContextMenuInstance {
            native_bounds: None,
                                    position: pos,
                                    content: ContextMenuContent::Items(items, None),
                                    scroll_handle: UniformListScrollHandle::new(),
                                    width: px(0.0),
                                    bottom_padding: 3.0,
                                    open_left: false,
                                    hovered_item: None,
                                    native_request_id: None,
                                });
                                cx.notify();
                            });
                        }
                    })
                    // Chevron expand toggle
                    .child(
                        div()
                            .w_4().h_4().mr_1()
                            .flex().items_center().justify_center().flex_shrink_0()
                            .when(has_subs, |d| d
                            .on_mouse_down(MouseButton::Left, {
                                let h = h.clone(); let p = path.clone();
                                move |_, _, cx| {
                                    cx.stop_propagation();
                                    h.update(cx, |view, cx| {
                                        view.toggle_sidebar_path(p.clone(), cx);
                                    });
                                }
                            })
                            .child(svg()
                                .path(if is_exp { "icons/chevron-down.svg" } else { "icons/chevron-right.svg" })
                                .size_full()
                                .text_color(theme.text_muted)))
                    )
                    // Drive icon + label
                    .child(
                        div().w_4().h_4().mr_2().flex().items_center().justify_center().flex_shrink_0()
                            .child(svg().path(crate::ui::components::icons::drive_icon_path(drive)).size_4().text_color(theme.text))
                    )
                    .child(
                        div().flex_1().text_ellipsis().overflow_hidden()
                            .text_color(theme.text)
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(label)
                    )
                    .drag_over::<FileDragData>({
                        let h = h.clone(); let tgt = path.clone();
                        move |style, data, _, cx| {
                            let default_copy = data.paths.first()
                                .map(|s| crate::ui::app::drives_differ(s, &tgt))
                                .unwrap_or(false);
                            h.update(cx, |this, cx| {
                                if this.drag_target_default_copy != default_copy {
                                    this.drag_target_default_copy = default_copy;
                                    this.update_drag_copy_mode(cx);
                                }
                            });
                            style.bg(theme.drag_hover)
                        }
                    })
                    .on_drop({
                        let h = h.clone(); let tgt = path.clone();
                        move |data: &FileDragData, _, cx| {
                            let paths = data.paths.clone();
                            h.update(cx, |this, cx| {
                                this.execute_file_drop(paths, tgt.clone(), cx);
                            });
                        }
                    })
                    .into_any_element()
            }

            // ── Directory item ─────────────────────────────
            SidebarItem::Directory { node, depth, is_expanded } => {
                let path       = node.path.clone();
                let is_sel     = sel.as_deref().is_some_and(|selected| sidebar_paths_equal(selected, &path));
                let is_exp     = *is_expanded;
                let has_subs   = node.has_subdirs;
                let depth_px   = logical(8.0) + (*depth as f32 * logical(12.0));
                let sel_bg:    Hsla = theme.selected;
                let hov_bg:    Hsla = if is_sel { theme.selection_hover } else { theme.hover };
                let is_dimmed  = node.is_hidden || node.is_system;

                div()
                    .id(SharedString::from(format!("sb-dir-{}", path)))
                    .w(px((content_w - logical(26.0)).max(1.0)))
                    .h(px(sidebar_row_h()))
                    .pl(px(depth_px))
                    .pr_3()
                    .flex()
                    .items_center()
                    .when(is_dimmed, |d| d.opacity(0.5))
                    .relative().when(is_sel, |d| d.bg(sel_bg).child(div().absolute().left_0().top_0().bottom_0().w(rems(0.1875)).bg(theme.accent)))
                    .hover(move |s| s.bg(hov_bg))
                    .when(show_tooltips, |d| d.shared_tooltip(h.clone(), HoverRegion::DirectoryTree, format!("directory:{path}"), cx, Tooltip::with_meta(node.name.clone(), path.clone())))
                    // Left click → select + navigate / toggle on double
                    .on_mouse_down(MouseButton::Left, {
                        let h = h.clone(); let p = path.clone();
                        move |e, _, cx| {
                            h.update(cx, |this, cx| {
                                this.sidebar_selected_path = Some(p.clone());
                                if e.click_count == 2 && has_subs {
                                    this.toggle_sidebar_path(p.clone(), cx);
                                } else {
                                    let id = this.state.active_pane_id.clone();
                                    this.navigate_to(&id, p.clone(), cx);
                                }
                            });
                        }
                    })
                    // Middle click → open in new tab
                    .on_mouse_down(MouseButton::Middle, {
                        let h = h.clone(); let p = path.clone();
                        move |_, _, cx| {
                            h.update(cx, |this, cx| {
                                let id = this.state.active_pane_id.clone();
                                this.add_tab(&id, p.clone(), false, cx);
                            });
                        }
                    })
                    // Right click → select + context menu
                    .on_mouse_down(MouseButton::Right, {
                        let h = h.clone(); let p = path.clone();
                        move |e, _, cx| {
                            let pos   = e.position;
                            let items = directory_context_menu(cx.i18n(), h.clone(), p.clone());
                            h.update(cx, |this, cx| {
                                this.sidebar_selected_path = Some(p.clone());
                                this.context_menus.clear();
                                this.context_menus.push(ContextMenuInstance {
            native_bounds: None,
                                    position: pos,
                                    content: ContextMenuContent::Items(items, None),
                                    scroll_handle: UniformListScrollHandle::new(),
                                    width: px(0.0),
                                    bottom_padding: 3.0,
                                    open_left: false,
                                    hovered_item: None,
                                    native_request_id: None,
                                });
                                cx.notify();
                            });
                        }
                    })
                    // Chevron expand toggle
                    .child(
                        div()
                            .w_4().h_4().mr_1()
                            .flex().items_center().justify_center().flex_shrink_0()
                            .when(has_subs, |d| d
                                .on_mouse_down(MouseButton::Left, {
                                    let h = h.clone(); let p = path.clone();
                                    move |_, _, cx| {
                                        cx.stop_propagation();
                                        h.update(cx, |view, cx| {
                                            view.toggle_sidebar_path(p.clone(), cx);
                                        });
                                    }
                                })
                            )
                            .child(if has_subs {
                                svg().path(if is_exp { "icons/chevron-down.svg" } else { "icons/chevron-right.svg" })
                                    .size_full().text_color(theme.text_muted)
                            } else {
                                svg().path("").size_full()
                            })
                    )
                    // Folder icon
                    .child(
                        div().w_4().h_4().mr_2().flex().items_center().justify_center().flex_shrink_0()
                            .child(crate::ui::components::icons::render_file_icon(
                                path.clone(), if is_exp && !use_system_icons {
                                    crate::ui::components::icons::IconType::FolderOpen
                                } else {
                                    crate::ui::components::icons::IconType::Folder
                                },
                                rems(1.0), theme.accent, use_system_icons, false,
                            ))
                    )
                    // Name
                    .child(
                        div().flex_1().text_ellipsis().overflow_hidden()
                            .text_color(theme.text)
                            .child(node.name.clone())
                    )
                    // Drag source
                    .on_drag(
                        FileDragData { paths: vec![path.clone()], source_pane_id: "sidebar".to_string() },
                        {
                            let drag_path = path.clone();
                            let h_drag    = h.clone();
                            move |_, cursor_offset, _, cx| {
                                let app   = h_drag.clone();
                                let paths = vec![drag_path.clone()];
                                cx.new(|cx| FileDragPreview::new(paths, cursor_offset, app, cx))
                            }
                        }
                    )
                    // Drag target
                    .drag_over::<FileDragData>({
                        let h = h.clone(); let tgt = path.clone(); let self_path = path.clone();
                        move |style, data, _, cx| {
                            if data.paths == vec![self_path.clone()] { return style; }
                            let default_copy = data.paths.first()
                                .map(|s| crate::ui::app::drives_differ(s, &tgt))
                                .unwrap_or(false);
                            h.update(cx, |this, cx| {
                                if this.drag_target_default_copy != default_copy {
                                    this.drag_target_default_copy = default_copy;
                                    this.update_drag_copy_mode(cx);
                                }
                            });
                            style.bg(theme.drag_hover)
                        }
                    })
                    .on_drop({
                        let h = h.clone(); let tgt = path.clone(); let self_path = path.clone();
                        move |data: &FileDragData, _, cx| {
                            if data.paths == vec![self_path.clone()] { return; }
                            let paths = data.paths.clone();
                            h.update(cx, |this, cx| {
                                this.execute_file_drop(paths, tgt.clone(), cx);
                            });
                        }
                    })
                    .into_any_element()
            }
        }
    }).collect::<Vec<_>>()
}
