use crate::ui::app::{
    ContextMenuContent, ContextMenuInstance, DragTarget, FileDragData, FileDragPreview, OxydeApp,
};
use crate::ui::components::dropdown::{DropdownItem, IconName};
use crate::ui::components::scrollbar::{render_scrollbar, ScrollbarAxis};
use crate::ui::components::tooltip::Tooltip;
use crate::ui::theme::{OxydeTheme, ThemeContext};
use crate::ui::workspace::SidebarItem;
use gpui::prelude::*;
use gpui::*;

const SIDEBAR_ROW_H: f32 = 28.0;
// Selection / hover colours
const SEL_BG: u32 = 0x0078d428; // blue ~16% opacity
const SEL_HOV: u32 = 0x0078d440; // blue ~25% on hover-while-selected
const ITEM_HOV: u32 = 0xffffff0d; // ~5% white for idle hover

// ── Per-item right-click menus ───────────────────────────────────────────────

fn pinned_context_menu(h: Entity<OxydeApp>, path: String) -> Vec<DropdownItem> {
    vec![
        DropdownItem {
            label: "Ouvrir".to_string(),
            icon: Some(IconName::FolderOpen),
            stay_open: false,
            has_submenu: false,
            is_dimmed: false,
            on_click: std::rc::Rc::new({
                let h = h.clone();
                let p = path.clone();
                move |_, app| {
                    h.update(app, |this, cx| {
                        this.context_menus.clear();
                        let id = this.state.active_pane_id.clone();
                        this.navigate_to(&id, p.clone(), cx);
                    });
                }
            }),
            on_hover: None,
        },
        DropdownItem {
            label: "Ouvrir dans un nouvel onglet".to_string(),
            icon: Some(IconName::Copy),
            stay_open: false,
            has_submenu: false,
            is_dimmed: false,
            on_click: std::rc::Rc::new({
                let h = h.clone();
                let p = path.clone();
                move |_, app| {
                    h.update(app, |this, cx| {
                        this.context_menus.clear();
                        let id = this.state.active_pane_id.clone();
                        this.add_tab(&id, p.clone(), false, cx);
                    });
                }
            }),
            on_hover: None,
        },
        DropdownItem {
            label: "Désépingler de l'accès rapide".to_string(),
            icon: Some(IconName::X),
            stay_open: false,
            has_submenu: false,
            is_dimmed: false,
            on_click: std::rc::Rc::new({
                let h = h.clone();
                let p = path.clone();
                move |_, app| {
                    h.update(app, |this, cx| {
                        this.context_menus.clear();
                        let _ = oxyde_lib::commands::system::remove_from_quick_access(p.clone());
                        this.state.reload_quick_access_and_refresh();
                        cx.notify();
                    });
                }
            }),
            on_hover: None,
        },
        DropdownItem {
            label: "Propriétés".to_string(),
            icon: None,
            stay_open: false,
            has_submenu: false,
            is_dimmed: false,
            on_click: std::rc::Rc::new({
                let h = h.clone();
                let p = path.clone();
                move |_, app| {
                    h.update(app, |this, _| {
                        this.context_menus.clear();
                        oxyde_lib::platform::platform().show_properties_dialog(&p);
                    });
                }
            }),
            on_hover: None,
        },
    ]
}

fn location_context_menu(h: Entity<OxydeApp>, path: String) -> Vec<DropdownItem> {
    vec![
        DropdownItem {
            label: "Ouvrir".to_string(),
            icon: Some(IconName::FolderOpen),
            stay_open: false,
            has_submenu: false,
            is_dimmed: false,
            on_click: std::rc::Rc::new({
                let h = h.clone();
                let p = path.clone();
                move |_, app| {
                    h.update(app, |this, cx| {
                        this.context_menus.clear();
                        let id = this.state.active_pane_id.clone();
                        this.navigate_to(&id, p.clone(), cx);
                    });
                }
            }),
            on_hover: None,
        },
        DropdownItem {
            label: "Propriétés".to_string(),
            icon: None,
            stay_open: false,
            has_submenu: false,
            is_dimmed: false,
            on_click: std::rc::Rc::new({
                let h = h.clone();
                let p = path.clone();
                move |_, app| {
                    h.update(app, |this, _| {
                        this.context_menus.clear();
                        oxyde_lib::platform::platform().show_properties_dialog(&p);
                    });
                }
            }),
            on_hover: None,
        },
    ]
}

fn drive_context_menu(h: Entity<OxydeApp>, path: String) -> Vec<DropdownItem> {
    vec![
        DropdownItem {
            label: "Ouvrir".to_string(),
            icon: Some(IconName::FolderOpen),
            stay_open: false,
            has_submenu: false,
            is_dimmed: false,
            on_click: std::rc::Rc::new({
                let h = h.clone();
                let p = path.clone();
                move |_, app| {
                    h.update(app, |this, cx| {
                        this.context_menus.clear();
                        let id = this.state.active_pane_id.clone();
                        this.navigate_to(&id, p.clone(), cx);
                    });
                }
            }),
            on_hover: None,
        },
        DropdownItem {
            label: "Ouvrir dans un nouvel onglet".to_string(),
            icon: Some(IconName::Copy),
            stay_open: false,
            has_submenu: false,
            is_dimmed: false,
            on_click: std::rc::Rc::new({
                let h = h.clone();
                let p = path.clone();
                move |_, app| {
                    h.update(app, |this, cx| {
                        this.context_menus.clear();
                        let id = this.state.active_pane_id.clone();
                        this.add_tab(&id, p.clone(), false, cx);
                    });
                }
            }),
            on_hover: None,
        },
        DropdownItem {
            label: "Propriétés".to_string(),
            icon: None,
            stay_open: false,
            has_submenu: false,
            is_dimmed: false,
            on_click: std::rc::Rc::new({
                let h = h.clone();
                let p = path.clone();
                move |_, app| {
                    h.update(app, |this, _| {
                        this.context_menus.clear();
                        oxyde_lib::platform::platform().show_properties_dialog(&p);
                    });
                }
            }),
            on_hover: None,
        },
    ]
}

fn directory_context_menu(h: Entity<OxydeApp>, path: String) -> Vec<DropdownItem> {
    vec![
        DropdownItem {
            label: "Ouvrir".to_string(),
            icon: Some(IconName::FolderOpen),
            stay_open: false,
            has_submenu: false,
            is_dimmed: false,
            on_click: std::rc::Rc::new({
                let h = h.clone();
                let p = path.clone();
                move |_, app| {
                    h.update(app, |this, cx| {
                        this.context_menus.clear();
                        let id = this.state.active_pane_id.clone();
                        this.navigate_to(&id, p.clone(), cx);
                    });
                }
            }),
            on_hover: None,
        },
        DropdownItem {
            label: "Ouvrir dans un nouvel onglet".to_string(),
            icon: Some(IconName::Copy),
            stay_open: false,
            has_submenu: false,
            is_dimmed: false,
            on_click: std::rc::Rc::new({
                let h = h.clone();
                let p = path.clone();
                move |_, app| {
                    h.update(app, |this, cx| {
                        this.context_menus.clear();
                        let id = this.state.active_pane_id.clone();
                        this.add_tab(&id, p.clone(), false, cx);
                    });
                }
            }),
            on_hover: None,
        },
    ]
}

// ── Main render ──────────────────────────────────────────────────────────────

pub fn render_sidebar(
    view: &OxydeApp,
    handle: Entity<OxydeApp>,
    _cx: &mut Context<OxydeApp>,
) -> impl IntoElement {
    let show_hidden = view.show_hidden;
    let show_system = view.show_system;
    // Paths that are genuinely in Quick Access — only these get "Désépingler"
    let pinned_paths: std::collections::HashSet<String> = view
        .state
        .pinned_items
        .iter()
        .map(|i| i.path.clone())
        .collect();
    let sidebar_items: Vec<SidebarItem> = view
        .state
        .sidebar_items
        .iter()
        .filter(|item| match item {
            SidebarItem::Directory { node, .. } => {
                if !show_hidden && node.is_hidden {
                    return false;
                }
                if !show_system && node.is_system {
                    return false;
                }
                true
            }
            SidebarItem::Pinned { item } => {
                if !show_hidden && item.name.starts_with('.') {
                    return false;
                }
                true
            }
            _ => true,
        })
        .cloned()
        .collect();
    let theme = *_cx.theme();

    // Dynamic content width (minimum = sidebar width)
    let calculated_content_w = sidebar_items
        .iter()
        .map(|item| match item {
            SidebarItem::Header(h) => h.len() as f32 * 8.0 + 20.0,
            SidebarItem::Pinned { item } => item.name.len() as f32 * 7.0 + 60.0,
            SidebarItem::Drive { drive, .. } => {
                (drive.label.len() + drive.path.len() + 3) as f32 * 7.0 + 60.0
            }
            SidebarItem::Directory { node, depth, .. } => {
                (*depth as f32 * 12.0) + node.name.len() as f32 * 7.0 + 60.0
            }
        })
        .fold(view.sidebar_width, |a, b| a.max(b));

    if (calculated_content_w - view.sidebar_content_width).abs() > 1.0 {
        let h = handle.clone();
        _cx.defer(move |cx| {
            h.update(cx, |this, _| {
                this.sidebar_content_width = calculated_content_w;
            });
        });
    }

    let viewport_w = view.sidebar_width;
    let content_w = calculated_content_w.max(viewport_w);
    let viewport_h = view.list_height;
    let content_h = (sidebar_items.len() as f32 * SIDEBAR_ROW_H).max(1.0);

    // Which path is currently "selected" (highlighted) in the sidebar
    let selected_path = view.sidebar_selected_path.clone();
    let show_tooltips = view.context_menus.is_empty();

    let mut container = div()
        .w_full()
        .h_full()
        .bg(theme.sidebar)
        .border_r_1()
        .border_color(theme.border)
        // NO horizontal padding — rows must span full width
        .overflow_hidden()
        .relative()
        .child(
            div()
                .size_full()
                .child(div().h(px(12.0))) // small top gap
                .child(
                    div()
                        .absolute()
                        .top(px(12.0))
                        .bottom_0()
                        .left(view.sidebar_scroll_h.offset().x)
                        .w(px(content_w))
                        .child({
                            let items_for_list = sidebar_items.clone();
                            let h = handle.clone();
                            let sel = selected_path.clone();
                            let pinned_paths_for_list = pinned_paths.clone();
                            uniform_list(
                                "sidebar-list",
                                sidebar_items.len(),
                                move |range, _, cx| {
                                    let items  = items_for_list.clone();
                                    let sel    = sel.clone();
                                    range.map(|i| {
                                        let theme    = *OxydeTheme::global(cx);
                                        let item     = &items[i];
                                        let h        = h.clone();

                                        match item {
                                            // ── Section header ─────────────────────────────
                                            SidebarItem::Header(label) => {
                                                div()
                                                    .h(px(SIDEBAR_ROW_H))
                                                    .px_3()
                                                    .flex()
                                                    .items_center()
                                                    .text_size(px(11.0))
                                                    .font_weight(FontWeight::SEMIBOLD)
                                                    .text_color(theme.text_muted)
                                                    .child(label.clone())
                                                    .into_any_element()
                                            }

                                            // ── Quick-access (pinned) item ─────────────────
                                            SidebarItem::Pinned { item } => {
                                                let path      = item.path.clone();
                                                let is_sel    = sel.as_deref() == Some(&path);
                                                let sel_bg:   Hsla = rgba(SEL_BG).into();
                                                let hov_bg:   Hsla = if is_sel { rgba(SEL_HOV).into() } else { rgba(ITEM_HOV).into() };
                                                let is_dimmed = item.name.starts_with('.');

                                                div()
                                                    .id(SharedString::from(format!("sb-pin-{}", path)))
                                                    .w(px(content_w))
                                                    .h(px(SIDEBAR_ROW_H))
                                                    .px_3()
                                                    .flex()
                                                    .items_center()
                                                    .when(is_dimmed, |d| d.opacity(0.5))
                                                    .when(is_sel, |d| d.bg(sel_bg))
                                                    .hover(move |s| s.bg(hov_bg))
                                                    .when(show_tooltips, |d| d.tooltip(Tooltip::with_meta(item.name.clone(), path.clone())))
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
                                                        let is_qa = pinned_paths_for_list.contains(&path);
                                                        move |e, _, cx| {
                                                            let pos   = e.position;
                                                            let items = if is_qa {
                                                                pinned_context_menu(h.clone(), p.clone())
                                                            } else {
                                                                location_context_menu(h.clone(), p.clone())
                                                            };
                                                            h.update(cx, |this, cx| {
                                                                this.sidebar_selected_path = Some(p.clone());
                                                                this.context_menus.clear();
                                                                this.context_menus.push(ContextMenuInstance {
                                                                    position: pos,
                                                                    content: ContextMenuContent::Items(items, None),
                                                                    scroll_handle: UniformListScrollHandle::new(),
                                                                    width: px(0.0),
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
                                                                let icon = crate::ui::components::icons::get_icon_type(&item.name, false, Some(&path), false, false);
                                                                svg().path(icon.path()).size_4().text_color(theme.accent)
                                                            })
                                                    )
                                                    .child(
                                                        div().flex_1().text_ellipsis().overflow_hidden()
                                                            .text_color(theme.text)
                                                            .child(item.name.clone())
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
                                                            style.bg(rgba(0x0078d440))
                                                        }
                                                    })
                                                    .on_drop({
                                                        let h = h.clone(); let tgt = path.clone();
                                                        move |data: &FileDragData, _, cx| {
                                                            let paths = data.paths.clone();
                                                            h.update(cx, |this, cx| {
                                                                if this.drag_copy_mode {
                                                                    this.execute_file_copy(paths, tgt.clone(), cx);
                                                                } else {
                                                                    this.execute_file_move(paths, tgt.clone(), cx);
                                                                }
                                                            });
                                                        }
                                                    })
                                                    .into_any_element()
                                            }

                                            // ── Drive item ─────────────────────────────────
                                            SidebarItem::Drive { drive, is_expanded } => {
                                                let path       = drive.path.clone();
                                                let is_sel     = sel.as_deref() == Some(&path);
                                                let is_exp     = *is_expanded;
                                                let sel_bg:    Hsla = rgba(SEL_BG).into();
                                                let hov_bg:    Hsla = if is_sel { rgba(SEL_HOV).into() } else { rgba(ITEM_HOV).into() };
                                                let label      = format!("{} ({})", drive.label, drive.path);

                                                div()
                                                    .id(SharedString::from(format!("sb-drv-{}", path)))
                                                    .w(px(content_w))
                                                    .h(px(SIDEBAR_ROW_H))
                                                    .px_3()
                                                    .flex()
                                                    .items_center()
                                                    .when(is_sel, |d| d.bg(sel_bg))
                                                    .hover(move |s| s.bg(hov_bg))
                                                    .tooltip(Tooltip::with_meta(label.clone(), path.clone()))
                                                    // Left click → select + navigate (single) / toggle (double)
                                                    .on_mouse_down(MouseButton::Left, {
                                                        let h = h.clone(); let p = path.clone();
                                                        move |e, _, cx| {
                                                            h.update(cx, |this, cx| {
                                                                this.sidebar_selected_path = Some(p.clone());
                                                                if e.click_count == 2 {
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
                                                            let items = drive_context_menu(h.clone(), p.clone());
                                                            h.update(cx, |this, cx| {
                                                                this.sidebar_selected_path = Some(p.clone());
                                                                this.context_menus.clear();
                                                                this.context_menus.push(ContextMenuInstance {
                                                                    position: pos,
                                                                    content: ContextMenuContent::Items(items, None),
                                                                    scroll_handle: UniformListScrollHandle::new(),
                                                                    width: px(0.0),
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
                                                            .hover(|s| s.bg(rgba(ITEM_HOV)).rounded_md())
                                                            .on_mouse_down(MouseButton::Left, {
                                                                let h = h.clone(); let p = path.clone();
                                                                move |_, _, cx| {
                                                                    h.update(cx, |view, cx| {
                                                                        view.toggle_sidebar_path(p.clone(), cx);
                                                                    });
                                                                }
                                                            })
                                                            .child(svg()
                                                                .path(if is_exp { "icons/chevron-down.svg" } else { "icons/chevron-right.svg" })
                                                                .size_full()
                                                                .text_color(theme.text_muted))
                                                    )
                                                    // Drive icon + label
                                                    .child(
                                                        div().w_4().h_4().mr_2().flex().items_center().justify_center().flex_shrink_0()
                                                            .child(svg().path("icons/hard-drive.svg").size_4().text_color(theme.accent))
                                                    )
                                                    .child(
                                                        div().flex_1().text_ellipsis().overflow_hidden()
                                                            .text_color(theme.text)
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
                                                            style.bg(rgba(0x0078d440))
                                                        }
                                                    })
                                                    .on_drop({
                                                        let h = h.clone(); let tgt = path.clone();
                                                        move |data: &FileDragData, _, cx| {
                                                            let paths = data.paths.clone();
                                                            h.update(cx, |this, cx| {
                                                                if this.drag_copy_mode {
                                                                    this.execute_file_copy(paths, tgt.clone(), cx);
                                                                } else {
                                                                    this.execute_file_move(paths, tgt.clone(), cx);
                                                                }
                                                            });
                                                        }
                                                    })
                                                    .into_any_element()
                                            }

                                            // ── Directory item ─────────────────────────────
                                            SidebarItem::Directory { node, depth, is_expanded } => {
                                                let path       = node.path.clone();
                                                let is_sel     = sel.as_deref() == Some(&path);
                                                let is_exp     = *is_expanded;
                                                let has_subs   = node.has_subdirs;
                                                let depth_px   = 8.0 + (*depth as f32 * 12.0);
                                                let sel_bg:    Hsla = rgba(SEL_BG).into();
                                                let hov_bg:    Hsla = if is_sel { rgba(SEL_HOV).into() } else { rgba(ITEM_HOV).into() };
                                                let is_dimmed  = node.is_hidden || node.is_system;

                                                div()
                                                    .id(SharedString::from(format!("sb-dir-{}", path)))
                                                    .w(px(content_w))
                                                    .h(px(SIDEBAR_ROW_H))
                                                    .pl(px(depth_px))
                                                    .pr_3()
                                                    .flex()
                                                    .items_center()
                                                    .when(is_dimmed, |d| d.opacity(0.5))
                                                    .when(is_sel, |d| d.bg(sel_bg))
                                                    .hover(move |s| s.bg(hov_bg))
                                                    .tooltip(Tooltip::with_meta(node.name.clone(), path.clone()))
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
                                                            let items = directory_context_menu(h.clone(), p.clone());
                                                            h.update(cx, |this, cx| {
                                                                this.sidebar_selected_path = Some(p.clone());
                                                                this.context_menus.clear();
                                                                this.context_menus.push(ContextMenuInstance {
                                                                    position: pos,
                                                                    content: ContextMenuContent::Items(items, None),
                                                                    scroll_handle: UniformListScrollHandle::new(),
                                                                    width: px(0.0),
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
                                                                .hover(|s| s.bg(rgba(ITEM_HOV)).rounded_md())
                                                                .on_mouse_down(MouseButton::Left, {
                                                                    let h = h.clone(); let p = path.clone();
                                                                    move |_, _, cx| {
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
                                                            .child(svg().path("icons/folder.svg").size_4().text_color(theme.accent))
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
                                                            style.bg(rgba(0x0078d440))
                                                        }
                                                    })
                                                    .on_drop({
                                                        let h = h.clone(); let tgt = path.clone(); let self_path = path.clone();
                                                        move |data: &FileDragData, _, cx| {
                                                            if data.paths == vec![self_path.clone()] { return; }
                                                            let paths = data.paths.clone();
                                                            h.update(cx, |this, cx| {
                                                                if this.drag_copy_mode {
                                                                    this.execute_file_copy(paths, tgt.clone(), cx);
                                                                } else {
                                                                    this.execute_file_move(paths, tgt.clone(), cx);
                                                                }
                                                            });
                                                        }
                                                    })
                                                    .into_any_element()
                                            }
                                        }
                                    }).collect::<Vec<_>>()
                                }
                            )
                            .track_scroll(&view.sidebar_scroll)
                            .size_full()
                        })
                )
        );

    // Vertical scrollbar
    container = container.child({
        let offset = -view
            .sidebar_scroll
            .0
            .borrow()
            .base_handle
            .offset()
            .y
            .as_f32();
        render_scrollbar(
            handle.clone(),
            ScrollbarAxis::Vertical,
            viewport_h,
            content_h,
            offset,
            matches!(view.drag_target, Some(DragTarget::SidebarV { .. })),
            "sidebar-v".to_string(),
            None,
            |this, track_id, v_h, c_h, _| {
                this.drag_target = Some(DragTarget::SidebarV {
                    viewport_h: v_h,
                    content_h: c_h,
                    track_id,
                });
            },
        )
    });

    // Horizontal scrollbar
    container = container.child(render_scrollbar(
        handle.clone(),
        ScrollbarAxis::Horizontal,
        viewport_w,
        content_w,
        view.sidebar_scroll_h.offset().x.as_f32(),
        matches!(view.drag_target, Some(DragTarget::SidebarH { .. })),
        "sidebar-h".to_string(),
        None,
        |this, track_id, v_w, c_w, _| {
            this.drag_target = Some(DragTarget::SidebarH {
                viewport_w: v_w,
                content_w: c_w,
                track_id,
            });
        },
    ));

    // Resize handle
    container = container.child(
        div()
            .absolute()
            .top_0()
            .right_0()
            .bottom_0()
            .w(px(4.0))
            .cursor_col_resize()
            .on_mouse_down(MouseButton::Left, {
                let h = handle.clone();
                move |e, _, cx| {
                    h.update(cx, |this, _| {
                        this.drag_target = Some(DragTarget::SidebarResize);
                        this.grab_offset = e.position.x.as_f32() - this.sidebar_width;
                    });
                }
            }),
    );

    container
}
