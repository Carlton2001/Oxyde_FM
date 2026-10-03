use crate::ui::scale::{logical, rem_pixels};
use crate::ui::app::{
    ContextMenuContent, ContextMenuInstance, DragTarget, FileDragData, FileDragPreview, OxydeApp};
use crate::ui::components::dropdown::{DropdownItem, IconName};
use crate::ui::components::scrollbar::{render_scrollbar, ScrollbarAxis};
use crate::ui::components::tooltip::Tooltip;
use crate::ui::i18n::{I18n, I18nContext, Message};
use crate::ui::theme::{OxydeTheme, ThemeContext};
use crate::ui::workspace::SidebarItem;
use gpui::prelude::*;
use gpui::*;

pub(crate) fn sidebar_paths_equal(left: &str, right: &str) -> bool {
    #[cfg(target_os = "windows")]
    {
        left.replace('/', "\\").trim_end_matches('\\').to_lowercase()
            == right.replace('/', "\\").trim_end_matches('\\').to_lowercase()
    }
    #[cfg(not(target_os = "windows"))]
    {
        std::path::Path::new(left) == std::path::Path::new(right)
    }
}

const SIDEBAR_ROW_H: Rems = rems(1.75);
fn sidebar_row_h() -> f32 { rem_pixels(SIDEBAR_ROW_H).as_f32() }

pub(crate) fn sidebar_item_visible(item: &SidebarItem, show_hidden: bool, show_system: bool, show_network: bool) -> bool {
    match item {
        SidebarItem::Directory { node, .. } =>
            (show_hidden || !node.is_hidden) && (show_system || !node.is_system),
        SidebarItem::Pinned { item } => (show_network || item.path != "__network_vincinity__")
            && (show_hidden || !item.name.starts_with('.')),
        SidebarItem::Header(Message::SidebarQuickAccess) => false,
        _ => true,
    }
}

fn pinned_display_name(item: &oxyde_lib::models::QuickAccessItem, i18n: &I18n) -> String {
    match item.path.as_str() {
        "__network_vincinity__" => i18n.text(Message::SidebarNetwork).to_string(),
        "trash://" => i18n.text(Message::SidebarTrash).to_string(),
        _ => item.name.clone(),
    }
}
// Selection / hover colours

// ── Per-item right-click menus ───────────────────────────────────────────────

fn pinned_context_menu(i18n: &I18n, h: Entity<OxydeApp>, path: String) -> Vec<DropdownItem> {
    vec![
        DropdownItem {
            label: i18n.text(Message::Open).to_string(),
            icon: Some(IconName::ExternalLink),
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
            label: i18n.text(Message::OpenNewTab).to_string(),
            icon: Some(IconName::ExternalLink),
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
            label: i18n.text(Message::UnpinQuickAccess).to_string(),
            icon: Some(IconName::PinOff),
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
            label: i18n.text(Message::Properties).to_string(),
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

fn location_context_menu(i18n: &I18n, h: Entity<OxydeApp>, path: String) -> Vec<DropdownItem> {
    vec![
        DropdownItem {
            label: i18n.text(Message::Open).to_string(),
            icon: Some(IconName::ExternalLink),
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
            label: i18n.text(Message::Properties).to_string(),
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

fn drive_context_menu(i18n: &I18n, h: Entity<OxydeApp>, path: String) -> Vec<DropdownItem> {
    let mut items = vec![
        DropdownItem {
            label: i18n.text(Message::Open).to_string(),
            icon: Some(IconName::ExternalLink),
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
            label: i18n.text(Message::OpenNewTab).to_string(),
            icon: Some(IconName::ExternalLink),
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
            label: i18n.text(Message::Properties).to_string(),
            icon: Some(IconName::Settings),
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
    ];
    items.extend(sidebar_tree_actions(i18n, h, path, false));
    items
}

fn directory_context_menu(i18n: &I18n, h: Entity<OxydeApp>, path: String) -> Vec<DropdownItem> {
    let mut items = vec![
        DropdownItem {
            label: i18n.text(Message::Open).to_string(),
            icon: Some(IconName::ExternalLink),
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
            label: i18n.text(Message::OpenNewTab).to_string(),
            icon: Some(IconName::ExternalLink),
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
    ];
    items.extend(sidebar_tree_actions(i18n, h, path, true));
    items
}

fn sidebar_tree_actions(
    i18n: &I18n,
    h: Entity<OxydeApp>,
    path: String,
    pin: bool,
) -> Vec<DropdownItem> {
    let mut items = Vec::new();
    for (message, icon, expand) in [
        (Message::ExpandAll, IconName::ChevronDown, true),
        (Message::CollapseAll, IconName::ChevronUp, false),
    ] {
        let h_action = h.clone();
        let path_action = path.clone();
        items.push(DropdownItem {
            label: i18n.text(message).to_string(),
            icon: Some(icon),
            stay_open: false,
            has_submenu: false,
            is_dimmed: false,
            on_click: std::rc::Rc::new(move |_, app| {
                h_action.update(app, |this, cx| {
                    this.context_menus.clear();
                    if expand {
                        this.expand_all_sidebar_path(path_action.clone(), cx);
                    } else {
                        this.state._collapse_all_sidebar_path(path_action.clone());
                        this.reconcile_watchers();
                        cx.notify();
                    }
                })
            }),
            on_hover: None,
        });
    }
    if pin {
        let path_pin = path.clone();
        let h_pin = h.clone();
        items.push(DropdownItem {
            label: i18n.text(Message::PinQuickAccess).to_string(),
            icon: Some(IconName::Pin),
            stay_open: false,
            has_submenu: false,
            is_dimmed: false,
            on_click: std::rc::Rc::new(move |_, app| {
                h_pin.update(app, |this, cx| {
                    this.context_menus.clear();
                    let _ = oxyde_lib::commands::system::add_to_quick_access(path_pin.clone());
                    this.state.reload_quick_access_and_refresh();
                    cx.notify();
                })
            }),
            on_hover: None,
        });
    }
    let p = path.clone();
    let h_prop = h.clone();
    items.push(DropdownItem {
        label: i18n.text(Message::Properties).to_string(),
        icon: Some(IconName::Settings),
        stay_open: false,
        has_submenu: false,
        is_dimmed: false,
        on_click: std::rc::Rc::new(move |_, app| {
            h_prop.update(app, |this, _| {
                this.context_menus.clear();
                oxyde_lib::platform::platform().show_properties_dialog(&p);
            })
        }),
        on_hover: None,
    });
    let p = path;
    items.push(DropdownItem {
        label: i18n.text(Message::MoreOptions).to_string(),
        icon: Some(IconName::Ellipsis),
        stay_open: true,
        has_submenu: true,
        is_dimmed: false,
        on_click: std::rc::Rc::new(|_, _| {}),
        on_hover: Some(std::rc::Rc::new(move |pos, left, app| {
            h.update(app, |this, cx| {
                this.request_native_context_menu(0, p.clone(), false, pos, left, cx);
            })
        })),
    });
    items
}

// ── Main render ──────────────────────────────────────────────────────────────

fn sidebar_button(
    id: String,
    label: SharedString,
    icon: Option<&'static str>,
    text: Option<String>,
    theme: OxydeTheme,
    enabled: bool,
    action: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    div()
        .id(id)
        .size(rems(2.0))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded_md()
        .text_color(if enabled {
            theme.text_muted
        } else {
            theme.icon_disabled
        })
        .when(enabled, |d| d.hover(move |s| s.bg(theme.hover)))
        .tooltip(Tooltip::text(label))
        .when(enabled, |d| d.on_mouse_down(MouseButton::Left, action))
        .when_some(icon, |d, path| {
            d.child(svg().path(path).size(rems(1.125)).text_color(if enabled {
                theme.text_muted
            } else {
                theme.icon_disabled
            }))
        })
        .when_some(text, |d, text| d.child(text))
}

fn sidebar_toolbar(view: &OxydeApp, handle: Entity<OxydeApp>, cx: &mut Context<OxydeApp>) -> Div {
    let theme = *cx.theme();
    let mut toolbar = div()
        .h(rems(2.5))
        .flex_none()
        .flex()
        .items_center()
        .px_2()
        .gap_1();
    if !view.sidebar_collapsed {
        let h = handle.clone();
        toolbar = toolbar.child(sidebar_button(
            "sidebar-collapse-all".into(),
            cx.i18n().text(Message::SidebarCollapseAll),
            Some("icons/chevrons-up.svg"),
            None,
            theme,
            view.state
                .drives
                .iter()
                .any(|drive| view.state.expanded_paths.contains(&drive.path)),
            move |_, _, cx| {
                h.update(cx, |this, cx| {
                    this.sidebar_follow_task = None;
                    this.state.expanded_paths.clear();
                    this.state.refresh_sidebar();
                    this.reconcile_watchers();
                    cx.notify();
                });
            },
        ));
        let h = handle.clone();
        toolbar = toolbar
            .child(sidebar_button(
                "sidebar-top".into(),
                cx.i18n().text(Message::SidebarScrollTop),
                Some("icons/arrow-up-to-line.svg"),
                None,
                theme,
                view.sidebar_scroll.0.borrow().base_handle.offset().y < px(0.0),
                move |_, _, cx| {
                    h.update(cx, |this, cx| {
                        let mut scroll = this.sidebar_scroll.0.borrow_mut();
                        // Reset the actual offset, even when the first row is already visible.
                        scroll.deferred_scroll_to_item = None;
                        let offset = scroll.base_handle.offset();
                        scroll.base_handle.set_offset(point(offset.x, px(0.0)));
                        cx.notify();
                    });
                },
            ))
            .child(div().flex_1());
    }
    toolbar.child(sidebar_button(
        "sidebar-toggle".into(),
        cx.i18n().text(if view.sidebar_collapsed {
            Message::SidebarExpand
        } else {
            Message::SidebarCollapse
        }),
        Some(if view.sidebar_collapsed {
            "icons/panel-left.svg"
        } else {
            "icons/panel-left-close.svg"
        }),
        None,
        theme,
        true,
        move |_, _, cx| {
            handle.update(cx, |this, cx| this.toggle_sidebar_panel(cx));
        },
    ))
}

fn render_sidebar_rail(
    view: &OxydeApp,
    handle: Entity<OxydeApp>,
    cx: &mut Context<OxydeApp>,
) -> Div {
    let theme = *cx.theme();
    let mut rail = div()
        .size_full()
        .flex()
        .flex_col()
        .bg(theme.sidebar)
        .child(sidebar_toolbar(view, handle.clone(), cx));
    let h = handle.clone();
    let pinned: Vec<_> = view
        .state
        .pinned_items
        .iter()
        .filter(|item| view.show_hidden || !item.name.starts_with('.'))
        .cloned()
        .collect();
    rail = rail.child(div().px_2().py_2().child(sidebar_button(
        "sidebar-pinned".into(),
        cx.i18n().text(Message::SidebarQuickAccess),
        Some("icons/pin.svg"),
        None,
        theme,
        true,
        move |e, _, cx| {
            let items = pinned
                .iter()
                .map(|item| {
                    let h = h.clone();
                    let path = item.path.clone();
                    DropdownItem {
                        label: pinned_display_name(item, cx.i18n()),
                        icon: Some(IconName::Pin),
                        stay_open: false,
                        has_submenu: false,
                        is_dimmed: false,
                        on_hover: None,
                        on_click: std::rc::Rc::new(move |_, cx| {
                            h.update(cx, |this, cx| {
                                this.context_menus.clear();
                                let id = this.state.active_pane_id.clone();
                                this.navigate_to(&id, path.clone(), cx);
                            });
                        }),
                    }
                })
                .collect();
            h.update(cx, |this, cx| {
                if !this.context_menus.is_empty() {
                    this.context_menus.clear();
                } else {
                    this.context_menus.push(ContextMenuInstance {
                        position: point(rem_pixels(rems(3.0)), e.position.y),
                        content: ContextMenuContent::Items(items, None),
                        scroll_handle: UniformListScrollHandle::new(),
                        width: px(0.0),
                        bottom_padding: 3.0,
                        open_left: false,
                        hovered_item: None,
                        native_request_id: None,
                    });
                }
                cx.notify();
            });
        },
    )));
    let mut entries = div()
        .id("sidebar-rail-scroll")
        .flex_1()
        .min_h_0()
        .overflow_y_scroll()
        .flex()
        .flex_col()
        .items_center()
        .gap_2();
    for item in &view.state.sidebar_items {
        let (path, label, icon, text) = match item {
            SidebarItem::Drive { drive, .. } => (
                drive.path.clone(),
                format!("{} ({})", drive.label, drive.path),
                None,
                Some(drive.path.chars().next().unwrap_or('/').to_string()),
            ),
            SidebarItem::Pinned { item }
                if item.path == "__network_vincinity__" || item.path == "trash://" =>
            {
                (
                    item.path.clone(),
                    pinned_display_name(item, cx.i18n()),
                    Some(if item.path == "trash://" {
                        "icons/trash-sidebar.svg"
                    } else {
                        "icons/globe.svg"
                    }),
                    None,
                )
            }
            _ => continue,
        };
        let h = handle.clone();
        let is_drive = matches!(item, SidebarItem::Drive { .. });
        let selected = view
            .state
            .layout
            .find_pane(&view.state.active_pane_id)
            .is_some_and(|pane| {
                let current_path = &pane.active_tab().path;
                if is_drive {
                    std::path::Path::new(current_path).ancestors().any(|ancestor| {
                        super::paths_match(&ancestor.to_string_lossy(), &path)
                    })
                } else {
                    super::paths_match(current_path, &path)
                }
            });
        entries = entries.child(
            sidebar_button(
                format!("rail-{path}"),
                label.into(),
                icon,
                text,
                theme,
                true,
                move |_, _, cx| {
                    h.update(cx, |this, cx| {
                        this.context_menus.clear();
                        let id = this.state.active_pane_id.clone();
                        this.navigate_to(&id, path.clone(), cx);
                    });
                },
            )
            .when(is_drive, |d| d.font_weight(FontWeight::BOLD).text_size(rems(0.75)))
            .when(selected, |d| {
                d.bg(theme.accent.opacity(0.1)).text_color(theme.accent)
            }),
        );
    }
    rail.child(entries)
}
pub fn render_sidebar(
    view: &OxydeApp,
    handle: Entity<OxydeApp>,
    _cx: &mut Context<OxydeApp>,
) -> impl IntoElement {
    if view.sidebar_collapsed {
        return render_sidebar_rail(view, handle, _cx);
    }
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
        .filter(|item| sidebar_item_visible(item, show_hidden, show_system, view.show_network()))
        .cloned()
        .collect();
    let theme = *_cx.theme();
    let use_system_icons = cfg!(target_os = "windows") && view.use_system_icons();

    // Dynamic content width (minimum = sidebar width)
    let calculated_content_w = sidebar_items
        .iter()
        .map(|item| match item {
            SidebarItem::Header(_) => 0.0,
            SidebarItem::Pinned { item } => {
                pinned_display_name(item, _cx.i18n()).len() as f32 * logical(7.0) + logical(60.0)
            }
            SidebarItem::Drive { drive, .. } => {
                (drive.label.len() + drive.path.len() + 3) as f32 * logical(7.0) + logical(60.0)
            }
            SidebarItem::Directory { node, depth, .. } => {
                (*depth as f32 * logical(12.0)) + node.name.len() as f32 * logical(7.0) + logical(60.0)
            }
        })
        .fold(view.sidebar_display_width(), |a, b| a.max(b));

    if (calculated_content_w - view.sidebar_content_width).abs() > 1.0 {
        let h = handle.clone();
        _cx.defer(move |cx| {
            h.update(cx, |this, _| {
                this.sidebar_content_width = calculated_content_w;
            });
        });
    }

    let viewport_w = view.sidebar_display_width();
    let content_w = calculated_content_w.max(viewport_w);
    let viewport_h = (view.list_height - logical(76.0)).max(1.0);
    let content_h = (sidebar_items.len() as f32 * sidebar_row_h()).max(1.0);

    // Highlight only the folder currently open in the active pane's active tab.
    let selected_path = view
        .state
        .layout
        .find_pane(&view.state.active_pane_id)
        .map(|pane| pane.active_tab().path.clone());
    let show_tooltips = view.context_menus.is_empty();

    let mut container = div()
        .w_full()
        .h_full()
        .bg(theme.sidebar)
        .on_scroll_wheel(_cx.listener(|_, _, _, cx| cx.notify()))
        // Inset the virtual list itself so selection backgrounds keep both margins.
        .overflow_hidden()
        .relative()
        .child(
            div()
                .size_full()
                .child(sidebar_toolbar(view, handle.clone(), _cx))
                .child(
                    div()
                        .absolute()
                        .top(rems(2.5))
                        .bottom(rems(2.25))
                        .left(rems(0.5))
                        .right(rems(1.125)) // 12 px scrollbar track + 6 px clear margin.
                        .overflow_hidden()
                        .child(div()
                        .relative()
                        .h_full()
                        .left(view.sidebar_scroll_h.offset().x)
                        .w(px((content_w - logical(26.0)).max(1.0)))
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
                                                    .when(show_tooltips, |d| d.tooltip(Tooltip::with_meta(pinned_display_name(item, cx.i18n()), path.clone())))
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
                                                        let is_qa = pinned_paths_for_list.iter().any(|pinned| super::paths_match(pinned, &path));
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
                                                                svg().path(icon_path).size_4().text_color(theme.accent)
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
                                                let is_sel     = sel.as_deref().is_some_and(|selected| sidebar_paths_equal(selected, &path));
                                                let is_exp     = *is_expanded;
                                                let sel_bg:    Hsla = theme.selected;
                                                let hov_bg:    Hsla = if is_sel { theme.selection_hover } else { theme.hover };
                                                let label      = format!("{} ({})", drive.label, drive.path);

                                                div()
                                                    .id(SharedString::from(format!("sb-drv-{}", path)))
                                                    .w(px((content_w - logical(26.0)).max(1.0)))
                                                    .h(px(sidebar_row_h()))
                                                    .px_3()
                                                    .flex()
                                                    .items_center()
                                                    .relative().when(is_sel, |d| d.bg(sel_bg).child(div().absolute().left_0().top_0().bottom_0().w(rems(0.1875)).bg(theme.accent)))
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
                                                            let items = drive_context_menu(cx.i18n(), h.clone(), p.clone());
                                                            h.update(cx, |this, cx| {
                                                                this.sidebar_selected_path = Some(p.clone());
                                                                this.context_menus.clear();
                                                                this.context_menus.push(ContextMenuInstance {
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
                                                            .hover(|s| s.bg(theme.hover).rounded_md())
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
                                                            let items = directory_context_menu(cx.i18n(), h.clone(), p.clone());
                                                            h.update(cx, |this, cx| {
                                                                this.sidebar_selected_path = Some(p.clone());
                                                                this.context_menus.clear();
                                                                this.context_menus.push(ContextMenuInstance {
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
                                                                .hover(|s| s.bg(theme.hover).rounded_md())
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
                                                                path.clone(), crate::ui::components::icons::IconType::Folder,
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
                        }))
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
        div()
            .absolute()
            .top(rems(2.5))
            .bottom(rems(2.25))
            .right_0()
            .w(rems(0.75))
            .child(render_scrollbar(
                theme,
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
            ))
    });

    // Horizontal scrollbar
    container = container.child(render_scrollbar(
        theme,
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

    container = container.child(crate::ui::components::operations::render_operations_footer(
        view,
        handle.clone(),
        _cx,
    ));

    container
}
