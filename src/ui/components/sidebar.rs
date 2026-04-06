use gpui::*;
use crate::ui::app::{OxydeApp, DragTarget, ContextMenuContent, ContextMenuInstance, FileDragData, FileDragPreview};
use crate::ui::workspace::SidebarItem;
use crate::ui::components::scrollbar::{render_scrollbar, ScrollbarAxis};
use crate::ui::theme::{OxydeTheme, ThemeContext, PixelsExt};

const SIDEBAR_ROW_H: f32 = 28.0;

pub fn render_sidebar(view: &OxydeApp, handle: Entity<OxydeApp>, _cx: &mut Context<OxydeApp>) -> impl IntoElement {
    let sidebar_items = view.state.sidebar_items.clone();
    let _sidebar_scroll = view.sidebar_scroll.clone();
    let theme = *_cx.theme();
    
    // CALCULATE DYNAMIC CONTENT WIDTH
    let calculated_content_w = sidebar_items.iter().map(|item| {
        match item {
            SidebarItem::Header(h) => h.len() as f32 * 8.0 + 20.0,
            SidebarItem::Pinned { item } => item.name.len() as f32 * 7.0 + 60.0,
            SidebarItem::Drive { drive, .. } => (drive.label.len() + drive.path.len() + 3) as f32 * 7.0 + 60.0,
            SidebarItem::Directory { node, depth, .. } => (*depth as f32 * 12.0) + (node.name.len() as f32 * 7.0) + 60.0,
        }
    }).fold(view.sidebar_width, |a, b| a.max(b));

    if (calculated_content_w - view.sidebar_content_width).abs() > 1.0 {
        let h = handle.clone();
        _cx.defer(move |cx| {
            let _ = h.update(cx, |this, _| {
                this.sidebar_content_width = calculated_content_w;
            });
        });
    }

    let viewport_w = view.sidebar_width;
    let content_w = calculated_content_w.max(viewport_w);
    let viewport_h = view.list_height;
    let content_h = (sidebar_items.len() as f32 * SIDEBAR_ROW_H).max(1.0);

    let mut container = div()
        .w_full()
        .h_full()
        .bg(theme.sidebar)
        .border_r_1()
        .border_color(theme.border)
        .p_1()
        .overflow_hidden()
        .relative()
        .child(
            div()
                .size_full()
                .on_mouse_move({
                    let h = handle.clone();
                    move |_, _, cx| {
                        let _ = h.update(cx, |this, cx| {
                            if this.tooltip_request.is_some() || this.tooltip_timer.is_some() {
                                this.tooltip_request = None;
                                this.tooltip_timer = None;
                                cx.notify();
                            }
                        });
                    }
                })
                .child(
                    div()
                        .h(px(16.0))
                )
                .child(
                    div()
                        .absolute()
                        .top(px(16.0))
                        .bottom_0()
                        .left(view.sidebar_scroll_h.offset().x)
                        .w(px(content_w))
                        .child({
                            uniform_list(
                                "sidebar-list",
                                sidebar_items.len(),
                                {
                                    let items_for_uniform_list = sidebar_items.clone();
                                    let h = handle.clone();
                                    move |range, _, cx| {
                                        let items = items_for_uniform_list.clone();
                                        range.map(|i| {
                                            let theme = OxydeTheme::global(cx);
                                            let item = &items[i];
                                            match item {
                                                SidebarItem::Header(label) => {
                                                    div()
                                                        .h(px(SIDEBAR_ROW_H))
                                                        .px_3()
                                                        .flex()
                                                        .items_center()
                                                        .pb_1()
                                                        .text_color(theme.text_muted)
                                                        .child(label.clone())
                                                        .into_any_element()
                                                }
                                                SidebarItem::Pinned { item } => {
                                                    let path = item.path.clone();
                                                    let h_inner = h.clone();
                                                    div()
                                                        .h(px(SIDEBAR_ROW_H))
                                                        .px_3()
                                                        .rounded_md()
                                                        .flex()
                                                        .items_center()
                                                        .hover(|s| s.bg(theme.hover))
                                                        .on_mouse_move({
                                                            let h = h_inner.clone();
                                                            let name = item.name.clone();
                                                            let path_cap = path.clone();
                                                            move |e, _, cx| {
                                                                cx.stop_propagation();
                                                                let path_inner = path_cap.clone();
                                                                let _ = h.update(cx, |this, cx| {
                                                                    let mut data = crate::ui::app::TooltipData {
                                                                        name: name.clone(),
                                                                        type_str: "".to_string(),
                                                                        size_str: "".to_string(),
                                                                        date_str: "".to_string(),
                                                                        full_path: Some(path_inner.clone()),
                                                                        position: e.position,
                                                                        view: None,
                                                                    };

                                                                    if let Some(existing) = this.tooltip_request.as_mut() {
                                                                        if existing.full_path.as_ref() == data.full_path.as_ref() {
                                                                            data.view = existing.view.clone();
                                                                        } else {
                                                                            data.view = Some(crate::ui::components::tooltip::Tooltip::new_for_file_full(
                                                                                data.name.clone(),
                                                                                path_inner.clone(),
                                                                                data.type_str.clone(),
                                                                                data.size_str.clone(),
                                                                                data.date_str.clone(),
                                                                                cx,
                                                                            ).into());
                                                                        }
                                                                        this.tooltip_request = Some(data);
                                                                        cx.notify();
                                                                    } else if this.tooltip_timer.is_none() {
                                                                        let h_inner = h.clone();
                                                                        let path_timer = path_inner.clone();
                                                                        this.tooltip_timer = Some(cx.spawn(move |_, cx: &mut AsyncApp| {
                                                                            let mut cx = cx.clone();
                                                                            let h_inner = h_inner.clone();
                                                                            let path_timer = path_timer.clone();
                                                                            async move {
                                                                                cx.background_executor().timer(std::time::Duration::from_millis(500)).await;
                                                                                let _ = h_inner.update(&mut cx, |this, cx| {
                                                                                    data.position = this.mouse_pos;
                                                                                    data.view = Some(crate::ui::components::tooltip::Tooltip::new_for_file_full(
                                                                                        data.name.clone(),
                                                                                        path_timer.clone(),
                                                                                        data.type_str.clone(),
                                                                                        data.size_str.clone(),
                                                                                        data.date_str.clone(),
                                                                                        cx,
                                                                                    ).into());
                                                                                    this.tooltip_request = Some(data);
                                                                                    this.tooltip_timer = None;
                                                                                    cx.notify();
                                                                                });
                                                                            }
                                                                        }));
                                                                    }
                                                                });
                                                            }
                                                        })
                                                        .on_mouse_down(MouseButton::Left, {
                                                            let p = path.clone();
                                                            let h_inner_inner = h_inner.clone();
                                                            move |e, _, cx| {
                                                                if e.click_count != 2 {
                                                                    let _ = h_inner_inner.update(cx, |this, cx| {
                                                                        let p_id = this.state.active_pane_id.clone();
                                                                        this.navigate_to(&p_id, p.clone(), cx);
                                                                    });
                                                                }
                                                            }
                                                        })
                                                        .on_mouse_down(MouseButton::Middle, {
                                                            let p = path.clone();
                                                            let h_inner_inner = h_inner.clone();
                                                            move |_, _, cx| {
                                                                let _ = h_inner_inner.update(cx, |view, cx| {
                                                                    let p_id = view.state.active_pane_id.clone();
                                                                    view.add_tab(&p_id, p.clone(), false, cx);
                                                                });
                                                            }
                                                        })
                                                        .on_mouse_down(MouseButton::Right, {
                                                            let h_right = h_inner.clone();
                                                            let items_cap = items.clone();
                                                            move |e, _, cx| {
                                                                let pos = e.position;
                                                                let value = items_cap.clone();
                                                                let h_lvl1 = h_right.clone();
                                                                let _ = h_right.update(cx, |this, cx| {
                                                                    this.context_menus.clear();
                                                                    let items: Vec<crate::ui::components::dropdown::DropdownItem> = value.into_iter().map(|it| {
                                                                        use crate::ui::workspace::SidebarItem;
                                                                        let h = h_lvl1.clone();
                                                                        match it {
                                                                            SidebarItem::Directory { node, .. } => {
                                                                                let path = node.path.clone();
                                                                                crate::ui::components::dropdown::DropdownItem::new(node.name.clone(), move |_, app| {
                                                                                    let _ = h.update(app, |this, cx| {
                                                                                        this.navigate_to("pane-0", path.clone(), cx);
                                                                                        this.context_menus.clear();
                                                                                    });
                                                                                }).icon(crate::ui::components::dropdown::IconName::FolderOpen)
                                                                            }
                                                                            SidebarItem::Drive { drive, .. } => {
                                                                                let path = drive.path.clone();
                                                                                crate::ui::components::dropdown::DropdownItem::new(drive.label.clone(), move |_, app| {
                                                                                    let _ = h.update(app, |this, cx| {
                                                                                        this.navigate_to("pane-0", path.clone(), cx);
                                                                                        this.context_menus.clear();
                                                                                    });
                                                                                }).icon(crate::ui::components::dropdown::IconName::HardDrive)
                                                                            }
                                                                            _ => crate::ui::components::dropdown::DropdownItem::new("Inconnu", |_, _| {})
                                                                        }
                                                                    }).collect();

                                                                    this.context_menus.push(ContextMenuInstance {
                                                                        position: pos,
                                                                        content: ContextMenuContent::Items(items, None),
                                                                        scroll_handle: UniformListScrollHandle::new(),
                                                                        width: px(0.0),
                                    open_left: false,
                                                                    });
                                                                    this.tooltip_request = None;
                                                                    this.tooltip_timer = None;
                                                                    cx.notify();
                                                                });
                                                            }
                                                        })
                                                        .child(
                                                            div()
                                                                .w_4()
                                                                .h_4()
                                                                .mr_2()
                                                                .flex()
                                                                .items_center()
                                                                .justify_center()
                                                                .flex_shrink_0()
                                                                .child({
                                                                    let icon_type = crate::ui::components::icons::get_icon_type(&item.name, false, Some(&path));
                                                                    svg().path(icon_type.path()).size_4().text_color(theme.accent)
                                                                })
                                                        )
                                                        .child(
                                                            div()
                                                                .flex_1()
                                                                .text_ellipsis()
                                                                .overflow_hidden()
                                                                .text_color(theme.text)
                                                                .child(item.name.clone())
                                                        )
                                                        .drag_over::<FileDragData>({
                                                            let h_over = h_inner.clone();
                                                            let tgt = path.clone();
                                                            move |style, data, _, cx| {
                                                                let default_copy = data.paths.first()
                                                                    .map(|s| crate::ui::app::drives_differ(s, &tgt))
                                                                    .unwrap_or(false);
                                                                let _ = h_over.update(cx, |this, cx| {
                                                                    if this.drag_target_default_copy != default_copy {
                                                                        this.drag_target_default_copy = default_copy;
                                                                        this.update_drag_copy_mode(cx);
                                                                    }
                                                                });
                                                                style.bg(rgba(0x0078d440))
                                                            }
                                                        })
                                                        .on_drop({
                                                            let h_drop = h_inner.clone();
                                                            let drop_path = path.clone();
                                                            move |data: &FileDragData, _, cx| {
                                                                let paths = data.paths.clone();
                                                                let target = drop_path.clone();
                                                                let _ = h_drop.update(cx, |this, cx| {
                                                                    if this.drag_copy_mode {
                                                                        this.execute_file_copy(paths, target, cx);
                                                                    } else {
                                                                        this.execute_file_move(paths, target, cx);
                                                                    }
                                                                });
                                                            }
                                                        })
                                                        .into_any_element()
                                                }
                                                SidebarItem::Drive { drive, is_expanded } => {
                                                    let path = drive.path.clone();
                                                    let h_inner = h.clone();
                                                    div()
                                                        .h(px(SIDEBAR_ROW_H))
                                                        .px_3()
                                                        .rounded_md()
                                                        .flex()
                                                        .items_center()
                                                        .hover(|s| s.bg(theme.hover))
                                                        .on_mouse_move({
                                                            let h = h_inner.clone();
                                                            let label = drive.label.clone();
                                                            let path_cap = path.clone();
                                                            move |e, _, cx| {
                                                                cx.stop_propagation();
                                                                let path_inner = path_cap.clone();
                                                                let _ = h.update(cx, |this, cx| {
                                                                    let mut data = crate::ui::app::TooltipData {
                                                                        name: label.clone(),
                                                                        type_str: "".to_string(),
                                                                        size_str: "".to_string(),
                                                                        date_str: "".to_string(),
                                                                        full_path: Some(path_inner.clone()),
                                                                        position: e.position,
                                                                        view: None,
                                                                    };

                                                                    if let Some(existing) = this.tooltip_request.as_mut() {
                                                                        if existing.full_path.as_ref() == data.full_path.as_ref() {
                                                                            data.view = existing.view.clone();
                                                                        } else {
                                                                            data.view = Some(crate::ui::components::tooltip::Tooltip::new_for_file_full(
                                                                                data.name.clone(),
                                                                                path_inner.clone(),
                                                                                data.type_str.clone(),
                                                                                data.size_str.clone(),
                                                                                data.date_str.clone(),
                                                                                cx,
                                                                            ).into());
                                                                        }
                                                                        this.tooltip_request = Some(data);
                                                                        cx.notify();
                                                                    } else if this.tooltip_timer.is_none() {
                                                                        let h_inner = h.clone();
                                                                        let path_timer = path_inner.clone();
                                                                        this.tooltip_timer = Some(cx.spawn(move |_, cx: &mut AsyncApp| {
                                                                            let mut cx = cx.clone();
                                                                            let h_inner = h_inner.clone();
                                                                            let path_timer = path_timer.clone();
                                                                            async move {
                                                                                cx.background_executor().timer(std::time::Duration::from_millis(500)).await;
                                                                                let _ = h_inner.update(&mut cx, |this, cx| {
                                                                                    data.position = this.mouse_pos;
                                                                                    data.view = Some(crate::ui::components::tooltip::Tooltip::new_for_file_full(
                                                                                        data.name.clone(),
                                                                                        path_timer.clone(),
                                                                                        data.type_str.clone(),
                                                                                        data.size_str.clone(),
                                                                                        data.date_str.clone(),
                                                                                        cx,
                                                                                    ).into());
                                                                                    this.tooltip_request = Some(data);
                                                                                    this.tooltip_timer = None;
                                                                                    cx.notify();
                                                                                });
                                                                            }
                                                                        }));
                                                                    }
                                                                });
                                                            }
                                                        })
                                                        .on_mouse_down(MouseButton::Middle, {
                                                            let p = path.clone();
                                                            let h_inner_inner = h_inner.clone();
                                                            move |_, _, cx| {
                                                                let _ = h_inner_inner.update(cx, |view, cx| {
                                                                    let p_id = view.state.active_pane_id.clone();
                                                                    view.add_tab(&p_id, p.clone(), false, cx);
                                                                });
                                                            }
                                                        })
                                                        .on_mouse_down(MouseButton::Right, {
                                                            let h_right = h_inner.clone();
                                                            let items_cap = items.clone();
                                                            move |e, _, cx| {
                                                                let pos = e.position;
                                                                let value = items_cap.clone();
                                                                let h_lvl1 = h_right.clone();
                                                                let _ = h_right.update(cx, |this, cx| {
                                                                    this.context_menus.clear();
                                                                    let items: Vec<crate::ui::components::dropdown::DropdownItem> = value.into_iter().map(|it| {
                                                                        use crate::ui::workspace::SidebarItem;
                                                                        let h = h_lvl1.clone();
                                                                        match it {
                                                                            SidebarItem::Directory { node, .. } => {
                                                                                let path = node.path.clone();
                                                                                crate::ui::components::dropdown::DropdownItem::new(node.name.clone(), move |_, app| {
                                                                                    let _ = h.update(app, |this, cx| {
                                                                                        this.navigate_to("pane-0", path.clone(), cx);
                                                                                        this.context_menus.clear();
                                                                                    });
                                                                                }).icon(crate::ui::components::dropdown::IconName::FolderOpen)
                                                                            }
                                                                            SidebarItem::Drive { drive, .. } => {
                                                                                let path = drive.path.clone();
                                                                                crate::ui::components::dropdown::DropdownItem::new(drive.label.clone(), move |_, app| {
                                                                                    let _ = h.update(app, |this, cx| {
                                                                                        this.navigate_to("pane-0", path.clone(), cx);
                                                                                        this.context_menus.clear();
                                                                                    });
                                                                                }).icon(crate::ui::components::dropdown::IconName::HardDrive)
                                                                            }
                                                                            _ => crate::ui::components::dropdown::DropdownItem::new("Inconnu", |_, _| {})
                                                                        }
                                                                    }).collect();

                                                                    this.context_menus.push(ContextMenuInstance {
                                                                        position: pos,
                                                                        content: ContextMenuContent::Items(items, None),
                                                                        scroll_handle: UniformListScrollHandle::new(),
                                                                        width: px(0.0),
                                    open_left: false,
                                                                    });
                                                                    this.tooltip_request = None;
                                                                    this.tooltip_timer = None;
                                                                    cx.notify();
                                                                });
                                                            }
                                                        })
                                                        .child(
                                                            div()
                                                                .w_4()
                                                                .h_4()
                                                                .mr_1()
                                                                .hover(|s| s.bg(theme.hover).rounded_md())
                                                                .on_mouse_down(MouseButton::Left, {
                                                                    let p = path.clone();
                                                                    let h = h_inner.clone();
                                                                    move |_, _, cx| {
                                                                        let _ = h.update(cx, |view, _| {
                                                                            view.state.toggle_sidebar_path(p.clone());
                                                                        });
                                                                    }
                                                                })
                                                                .child(svg().path(if *is_expanded { "icons/chevron-down.svg" } else { "icons/chevron-right.svg" }).text_color(theme.text_muted).size_full())
                                                        )
                                                        .child(
                                                            div()
                                                                .flex_1()
                                                                .flex()
                                                                .items_center()
                                                                .on_mouse_down(MouseButton::Left, {
                                                                    let p = path.clone();
                                                                    let h_inner_inner = h_inner.clone();
                                                                    move |e, _, cx| {
                                                                        if e.click_count == 2 {
                                                                            let _ = h_inner_inner.update(cx, |view, _| {
                                                                                view.state.toggle_sidebar_path(p.clone());
                                                                            });
                                                                        } else {
                                                                            let _ = h_inner_inner.update(cx, |this, cx| {
                                                                                let p_id = this.state.active_pane_id.clone();
                                                                                this.navigate_to(&p_id, p.clone(), cx);
                                                                            });
                                                                        }
                                                                    }
                                                                })
                                                                .child(
                                                                    div()
                                                                        .w_4()
                                                                        .h_4()
                                                                        .mr_2()
                                                                        .flex()
                                                                        .items_center()
                                                                        .justify_center()
                                                                        .flex_shrink_0()
                                                                        .child(svg().path("icons/hard-drive.svg").size_4().text_color(theme.accent))
                                                                )
                                                                .child(
                                                                    div()
                                                                        .flex_1()
                                                                        .text_ellipsis()
                                                                        .overflow_hidden()
                                                                        .text_color(theme.text)
                                                                        .child(format!("{} ({})", drive.label, drive.path))
                                                                )
                                                        )
                                                        .drag_over::<FileDragData>({
                                                            let h_over = h_inner.clone();
                                                            let tgt = path.clone();
                                                            move |style, data, _, cx| {
                                                                let default_copy = data.paths.first()
                                                                    .map(|s| crate::ui::app::drives_differ(s, &tgt))
                                                                    .unwrap_or(false);
                                                                let _ = h_over.update(cx, |this, cx| {
                                                                    if this.drag_target_default_copy != default_copy {
                                                                        this.drag_target_default_copy = default_copy;
                                                                        this.update_drag_copy_mode(cx);
                                                                    }
                                                                });
                                                                style.bg(rgba(0x0078d440))
                                                            }
                                                        })
                                                        .on_drop({
                                                            let h_drop = h_inner.clone();
                                                            let drop_path = path.clone();
                                                            move |data: &FileDragData, _, cx| {
                                                                let paths = data.paths.clone();
                                                                let target = drop_path.clone();
                                                                let _ = h_drop.update(cx, |this, cx| {
                                                                    if this.drag_copy_mode {
                                                                        this.execute_file_copy(paths, target, cx);
                                                                    } else {
                                                                        this.execute_file_move(paths, target, cx);
                                                                    }
                                                                });
                                                            }
                                                        })
                                                        .into_any_element()
                                                }
                                                SidebarItem::Directory { node, depth, is_expanded } => {
                                                    let path = node.path.clone();
                                                    let h_inner = h.clone();
                                                    div()
                                                        .id(SharedString::from(format!("sidebar-dir-{}", path)))
                                                        .h(px(SIDEBAR_ROW_H))
                                                        .pl(px(8.0 + (*depth as f32 * 12.0)))
                                                        .pr_3()
                                                        .rounded_md()
                                                        .flex()
                                                        .items_center()
                                                        .text_color(theme.text)
                                                        .hover(|s| s.bg(theme.hover))
                                                        .on_mouse_move({
                                                            let h = h_inner.clone();
                                                            let name = node.name.clone();
                                                            let path_cap = path.clone();
                                                            move |e, _, cx| {
                                                                cx.stop_propagation();
                                                                let path_inner = path_cap.clone();
                                                                let _ = h.update(cx, |this, cx| {
                                                                    let mut data = crate::ui::app::TooltipData {
                                                                        name: name.clone(),
                                                                        type_str: "".to_string(),
                                                                        size_str: "".to_string(),
                                                                        date_str: "".to_string(),
                                                                        full_path: Some(path_inner.clone()),
                                                                        position: e.position,
                                                                        view: None,
                                                                    };

                                                                    if let Some(existing) = this.tooltip_request.as_mut() {
                                                                        if existing.full_path.as_ref() == data.full_path.as_ref() {
                                                                            data.view = existing.view.clone();
                                                                        } else {
                                                                            data.view = Some(crate::ui::components::tooltip::Tooltip::new_for_file_full(
                                                                                data.name.clone(),
                                                                                path_inner.clone(),
                                                                                data.type_str.clone(),
                                                                                data.size_str.clone(),
                                                                                data.date_str.clone(),
                                                                                cx,
                                                                            ).into());
                                                                        }
                                                                        this.tooltip_request = Some(data);
                                                                        cx.notify();
                                                                    } else if this.tooltip_timer.is_none() {
                                                                        let h_inner = h.clone();
                                                                        let path_timer = path_inner.clone();
                                                                        this.tooltip_timer = Some(cx.spawn(move |_, cx: &mut AsyncApp| {
                                                                            let mut cx = cx.clone();
                                                                            let h_inner = h_inner.clone();
                                                                            let path_timer = path_timer.clone();
                                                                            async move {
                                                                                cx.background_executor().timer(std::time::Duration::from_millis(500)).await;
                                                                                let _ = h_inner.update(&mut cx, |this, cx| {
                                                                                    data.position = this.mouse_pos;
                                                                                    data.view = Some(crate::ui::components::tooltip::Tooltip::new_for_file_full(
                                                                                        data.name.clone(),
                                                                                        path_timer.clone(),
                                                                                        data.type_str.clone(),
                                                                                        data.size_str.clone(),
                                                                                        data.date_str.clone(),
                                                                                        cx,
                                                                                    ).into());
                                                                                    this.tooltip_request = Some(data);
                                                                                    this.tooltip_timer = None;
                                                                                    cx.notify();
                                                                                });
                                                                            }
                                                                        }));
                                                                    }
                                                                });
                                                            }
                                                        })
                                                        .on_mouse_down(MouseButton::Right, {
                                                            let h_right = h_inner.clone();
                                                            let items_cap = items.clone();
                                                            move |e, _, cx| {
                                                                let pos = e.position;
                                                                let value = items_cap.clone();
                                                                let h_lvl1 = h_right.clone();
                                                                let _ = h_right.update(cx, |this, cx| {
                                                                    this.context_menus.clear();
                                                                    let items: Vec<crate::ui::components::dropdown::DropdownItem> = value.into_iter().map(|it| {
                                                                        use crate::ui::workspace::SidebarItem;
                                                                        let h = h_lvl1.clone();
                                                                        match it {
                                                                            SidebarItem::Directory { node, .. } => {
                                                                                let path = node.path.clone();
                                                                                crate::ui::components::dropdown::DropdownItem::new(node.name.clone(), move |_, app| {
                                                                                    let _ = h.update(app, |this, cx| {
                                                                                        this.navigate_to("pane-0", path.clone(), cx);
                                                                                        this.context_menus.clear();
                                                                                    });
                                                                                }).icon(crate::ui::components::dropdown::IconName::FolderOpen)
                                                                            }
                                                                            SidebarItem::Drive { drive, .. } => {
                                                                                let path = drive.path.clone();
                                                                                crate::ui::components::dropdown::DropdownItem::new(drive.label.clone(), move |_, app| {
                                                                                    let _ = h.update(app, |this, cx| {
                                                                                        this.navigate_to("pane-0", path.clone(), cx);
                                                                                        this.context_menus.clear();
                                                                                    });
                                                                                }).icon(crate::ui::components::dropdown::IconName::HardDrive)
                                                                            }
                                                                            _ => crate::ui::components::dropdown::DropdownItem::new("Inconnu", |_, _| {})
                                                                        }
                                                                    }).collect();

                                                                    this.context_menus.push(ContextMenuInstance {
                                                                        position: pos,
                                                                        content: ContextMenuContent::Items(items, None),
                                                                        scroll_handle: UniformListScrollHandle::new(),
                                                                        width: px(0.0),
                                    open_left: false,
                                                                    });
                                                                    this.tooltip_request = None;
                                                                    this.tooltip_timer = None;
                                                                    cx.notify();
                                                                });
                                                            }
                                                        })
                                                        .child(
                                                            div()
                                                                .w_4()
                                                                .h_4()
                                                                .mr_1()
                                                                .hover(|s| if node.has_subdirs { s.bg(theme.hover).rounded_md() } else { s })
                                                                .on_mouse_down(MouseButton::Left, {
                                                                    let p = path.clone();
                                                                    let has_subs = node.has_subdirs;
                                                                    let h_inner_inner = h_inner.clone();
                                                                    move |_, _, cx| {
                                                                        if has_subs {
                                                                            let _ = h_inner_inner.update(cx, |view, _| {
                                                                                view.state.toggle_sidebar_path(p.clone());
                                                                            });
                                                                        }
                                                                    }
                                                                })
                                                                .child(if node.has_subdirs {
                                                                    svg().path(if *is_expanded { "icons/chevron-down.svg" } else { "icons/chevron-right.svg" }).text_color(theme.text_muted).size_full()
                                                                } else {
                                                                    svg().path("")
                                                                })
                                                        )
                                                        .child(
                                                            div()
                                                                .flex_1()
                                                                .flex()
                                                                .items_center()
                                                                .on_mouse_down(MouseButton::Left, {
                                                                    let p = path.clone();
                                                                    let has_subs = node.has_subdirs;
                                                                    let h_inner_inner = h_inner.clone();
                                                                    move |e, _, cx| {
                                                                        if e.click_count == 2 && has_subs {
                                                                            let _ = h_inner_inner.update(cx, |view, _| {
                                                                                view.state.toggle_sidebar_path(p.clone());
                                                                            });
                                                                        } else {
                                                                            let _ = h_inner_inner.update(cx, |view, cx| {
                                                                                let p_id = view.state.active_pane_id.clone();
                                                                                view.navigate_to(&p_id, p.clone(), cx);
                                                                            });
                                                                        }
                                                                    }
                                                                })
                                                                .on_mouse_down(MouseButton::Middle, {
                                                                    let p = path.clone();
                                                                    let h_inner_inner = h_inner.clone();
                                                                    move |_, _, cx| {
                                                                        let _ = h_inner_inner.update(cx, |view, cx| {
                                                                            let p_id = view.state.active_pane_id.clone();
                                                                            view.add_tab(&p_id, p.clone(), false, cx);
                                                                        });
                                                                    }
                                                                })
                                                                .child(
                                                                    div()
                                                                        .w_4()
                                                                        .h_4()
                                                                        .mr_2()
                                                                        .flex()
                                                                        .items_center()
                                                                        .justify_center()
                                                                        .flex_shrink_0()
                                                                        .child(svg().path("icons/folder.svg").size_4().text_color(theme.accent))
                                                                )
                                                                .child(
                                                                    div()
                                                                        .flex_1()
                                                                        .text_ellipsis()
                                                                        .overflow_hidden()
                                                                        .child(node.name.clone())
                                                                )
                                                        )
                                                        .on_drag(
                                                            FileDragData { paths: vec![path.clone()], source_pane_id: "sidebar".to_string() },
                                                            {
                                                                let drag_path = path.clone();
                                                                let h_drag = h_inner.clone();
                                                                move |_, cursor_offset, _, cx| {
                                                                    let app = h_drag.clone();
                                                                    let paths = vec![drag_path.clone()];
                                                                    cx.new(|cx| FileDragPreview::new(paths, cursor_offset, app, cx))
                                                                }
                                                            }
                                                        )
                                                        .drag_over::<FileDragData>({
                                                            let drag_path = path.clone();
                                                            let h_over = h_inner.clone();
                                                            let tgt = path.clone();
                                                            move |style, data, _, cx| {
                                                                // Don't highlight if dragging onto itself
                                                                if data.paths == vec![drag_path.clone()] {
                                                                    return style;
                                                                }
                                                                let default_copy = data.paths.first()
                                                                    .map(|s| crate::ui::app::drives_differ(s, &tgt))
                                                                    .unwrap_or(false);
                                                                let _ = h_over.update(cx, |this, cx| {
                                                                    if this.drag_target_default_copy != default_copy {
                                                                        this.drag_target_default_copy = default_copy;
                                                                        this.update_drag_copy_mode(cx);
                                                                    }
                                                                });
                                                                style.bg(rgba(0x0078d440))
                                                            }
                                                        })
                                                        .on_drop({
                                                            let h_drop = h_inner.clone();
                                                            let drop_path = path.clone();
                                                            move |data: &FileDragData, _, cx| {
                                                                // Prevent dropping a directory onto itself
                                                                if data.paths == vec![drop_path.clone()] {
                                                                    return;
                                                                }
                                                                let paths = data.paths.clone();
                                                                let target = drop_path.clone();
                                                                let _ = h_drop.update(cx, |this, cx| {
                                                                    if this.drag_copy_mode {
                                                                        this.execute_file_copy(paths, target, cx);
                                                                    } else {
                                                                        this.execute_file_move(paths, target, cx);
                                                                    }
                                                                });
                                                            }
                                                        })
                                                        .into_any_element()
                                                }
                                            }
                                        }).collect::<Vec<_>>()
                                    }
                                }
                            )
                            .track_scroll(view.sidebar_scroll.clone())
                            .size_full()
                        })
                )
        );

    // Vertical scrollbar
    container = container.child({
        let scroll_offset = -view.sidebar_scroll.0.borrow().base_handle.offset().y.as_f32();
        render_scrollbar(handle.clone(), ScrollbarAxis::Vertical, viewport_h, content_h, scroll_offset, matches!(view.drag_target, Some(DragTarget::SidebarV { .. })), "sidebar-v".to_string(), None, {
            move |this, track_id, v_h, c_h, _| {
                this.drag_target = Some(DragTarget::SidebarV { viewport_h: v_h, content_h: c_h, track_id });
            }
        })
    });

    // Horizontal scrollbar
    container = container.child(
        render_scrollbar(handle.clone(), ScrollbarAxis::Horizontal, viewport_w, content_w, view.sidebar_scroll_h.offset().x.as_f32(), matches!(view.drag_target, Some(DragTarget::SidebarH { .. })), "sidebar-h".to_string(), None, {
            move |this, track_id, v_w, c_w, _| {
                this.drag_target = Some(DragTarget::SidebarH { viewport_w: v_w, content_w: c_w, track_id });
            }
        })
    );

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
                    let _ = h.update(cx, |this, _| {
                        this.drag_target = Some(DragTarget::SidebarResize);
                        this.grab_offset = e.position.x.as_f32() - this.sidebar_width;
                    });
                }
            })
    );

    container
}
