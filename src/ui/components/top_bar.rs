use crate::ui::app::{FileDragData, OxydeApp};
use crate::ui::components::dropdown::IconName;
use crate::ui::components::tooltip::Tooltip;
use crate::ui::i18n::{FluentArgs, I18nContext, Message};
use crate::ui::style::ROW_HEIGHT;
use crate::ui::theme::ThemeContext;
use gpui::prelude::*;
use gpui::*;
use oxyde_lib::models::TransactionType;

fn op_label(op: &TransactionType) -> Message {
    match op {
        TransactionType::Copy => Message::OpCopy,
        TransactionType::Move => Message::OpMove,
        TransactionType::Rename => Message::OpRename,
        TransactionType::Delete => Message::OpDelete,
        TransactionType::NewFolder => Message::OpNewFolder,
        TransactionType::Restore => Message::OpRestore,
    }
}

use oxyde_lib::models::FileEntry;

/// Returns the display label for a drive letter segment (e.g. "C:" → "Disque local (C:)").
/// Falls back to the raw part if no matching drive is found in sidebar_items.
fn drive_display(part: &str, view: &OxydeApp) -> String {
    use crate::ui::workspace::SidebarItem;
    let part_upper = part.to_uppercase();
    for item in &view.state.sidebar_items {
        if let SidebarItem::Drive { drive, .. } = item {
            if drive.path.to_uppercase().starts_with(&part_upper) && !drive.label.is_empty() {
                return format!("{} ({})", drive.label, part);
            }
        }
    }
    part.to_string()
}

pub fn render_top_bar(
    view: &OxydeApp,
    pane_id: String,
    handle: Entity<OxydeApp>,
    _cx: &mut Context<OxydeApp>,
) -> impl IntoElement {
    let pane = view.state.layout.find_pane(&pane_id).expect("pane exists");
    let active_tab = pane.active_tab();
    let current_path = active_tab.path.clone();
    let breadcrumb_scroll = active_tab.breadcrumb_scroll.clone();
    let breadcrumb_viewport_width = active_tab.breadcrumb_viewport_width.clone();
    let theme = *_cx.theme();

    let path_parts: Vec<&str> = current_path
        .split(['/', '\\'])
        .filter(|s: &&str| !s.is_empty())
        .collect();
    let total_parts = path_parts.len();
    if *active_tab.breadcrumb_scroll_path.borrow() != current_path {
        *active_tab.breadcrumb_scroll_path.borrow_mut() = current_path.clone();
        if total_parts > 0 {
            breadcrumb_scroll.scroll_to_item(total_parts * 2 - 2);
        } else {
            breadcrumb_scroll.set_offset(point(px(0.0), px(0.0)));
        }
        // The scroll handle's dimensions are updated during prepaint.
        _cx.notify();
    }
    let scroll_left = -breadcrumb_scroll.offset().x.as_f32();
    let scroll_max = breadcrumb_scroll.max_offset().x.as_f32();
    let fade_left = scroll_left > 2.0;
    let fade_right = scroll_max - scroll_left > 2.0;

    let can_go_back = !active_tab.back_stack.is_empty();
    let can_go_forward = !active_tab.forward_stack.is_empty();
    let show_tooltips = view.context_menus.is_empty();
    let show_hidden_setting = view.show_hidden;
    let show_system_setting = view.show_system;

    let history_state = view.history.get_state();
    let has_undo_entry = !history_state.undo_stack.is_empty();
    let undo_blocked_by_overwrite = history_state
        .undo_stack
        .last()
        .is_some_and(|transaction| transaction.undo_blocked_by_overwrite());
    let can_undo = has_undo_entry && !undo_blocked_by_overwrite;
    let can_redo = !history_state.redo_stack.is_empty();
    let has_history_actions = has_undo_entry || can_redo;

    let undo_tooltip = history_state.undo_stack.last().map(|t| {
        let mut args = FluentArgs::new();
        args.set(
            "operation",
            _cx.i18n().text(op_label(&t.op_type)).to_string(),
        );
        _cx.i18n()
            .text_with_args(Message::UndoOperation, &args)
            .to_string()
    });
    let redo_tooltip = history_state.redo_stack.last().map(|t| {
        let mut args = FluentArgs::new();
        args.set(
            "operation",
            _cx.i18n().text(op_label(&t.op_type)).to_string(),
        );
        _cx.i18n()
            .text_with_args(Message::RedoOperation, &args)
            .to_string()
    });

    // Shared constants for consistent look
    let icon_active = theme.text_muted;
    let icon_disabled = theme.icon_disabled;
    let hover_bg: Hsla = theme.control_hover;
    let crumb_sep: Hsla = theme.icon_muted; // chevrons — dim separator
    let crumb_parent: Hsla = theme.text_muted; // parent segments — readable but recessed
    let crumb_current = theme.text; // current segment — full brightness

    div()
        .h(px(32.0))
        .w_full()
        .flex()
        .items_center()
        .px_4()
        .bg(theme.background)
        .border_b_1()
        .border_color(theme.border)
        .child(
            div()
                .flex()
                .items_center()
                .gap_1()
                .child({
                    let mut btn = div()
                        .id("back-btn")
                        .w_6()
                        .h_6()
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded_md();

                    if can_go_back {
                        let p_id = pane_id.clone();
                        btn = btn
                            .hover(move |s| s.bg(hover_bg))
                            .when(show_tooltips, |d| d.tooltip(Tooltip::with_meta(_cx.i18n().text(Message::Back), _cx.i18n().text(Message::ShortcutBack))))
                            .on_mouse_down(MouseButton::Left, {
                                let h = handle.clone();
                                move |_, _, cx| { h.update(cx, |v: &mut OxydeApp, cx| v.go_back(&p_id, cx)); }
                            });
                    }

                    btn.child(
                        svg()
                            .path("icons/arrow-left.svg")
                            .text_color(if can_go_back { icon_active } else { icon_disabled })
                            .size_4()
                    )
                })
                .child({
                    div()
                        .id("refresh-btn")
                        .w_6()
                        .h_6()
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded_md()
                        .hover(move |s| s.bg(hover_bg))
                        .when(show_tooltips, |d| d.tooltip(Tooltip::text(_cx.i18n().text(Message::Refresh))))
                        .on_mouse_down(MouseButton::Left, {
                            let h = handle.clone();
                            move |_, _, cx| { h.update(cx, |this, cx| this.refresh_all(cx)); }
                        })
                        .child(
                            svg()
                                .path("icons/refresh.svg")
                                .text_color(icon_active)
                                .size_4()
                        )
                })
                .child({
                    let mut btn = div()
                        .id("forward-btn")
                        .w_6()
                        .h_6()
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded_md();

                    if can_go_forward {
                        let p_id = pane_id.clone();
                        btn = btn
                            .hover(move |s| s.bg(hover_bg))
                            .when(show_tooltips, |d| d.tooltip(Tooltip::with_meta(_cx.i18n().text(Message::Forward), _cx.i18n().text(Message::ShortcutForward))))
                            .on_mouse_down(MouseButton::Left, {
                                let h = handle.clone();
                                move |_, _, cx| { h.update(cx, |v: &mut OxydeApp, cx| v.go_forward(&p_id, cx)); }
                            });
                    }

                    btn.child(
                        svg()
                            .path("icons/arrow-right.svg")
                            .text_color(if can_go_forward { icon_active } else { icon_disabled })
                            .size_4()
                    )
                })
        )
        // Separator
        .child(
            div()
                .w(px(1.0))
                .h(px(16.0))
                .mx_3()
                .bg(theme.border)
        )
        // Undo / Redo block — only rendered when at least one action is available
        .when(has_history_actions, |bar| {
            bar
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_1()
        .when(has_undo_entry, |d| {
            let h = handle.clone();
            let tip = if undo_blocked_by_overwrite {
                _cx.i18n().text(Message::UndoOverwriteUnavailable).to_string()
            } else {
                undo_tooltip.clone().unwrap_or_default()
            };
            let mut button = div()
                .id("undo-btn")
                .w_6()
                .h_6()
                .flex()
                .items_center()
                .justify_center()
                .rounded_md();
            if can_undo {
                button = button
                    .hover(move |s| s.bg(hover_bg))
                    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                        h.update(cx, |this, cx| this.execute_undo(cx));
                    });
            }
            d.child(
                button
                    .when(show_tooltips, |d| d.tooltip(Tooltip::with_meta(_cx.i18n().text(Message::Undo), tip)))
                    .child(svg().path("icons/undo-2.svg").size_4().text_color(
                        if can_undo { icon_active } else { icon_disabled }
                    ))
            )
        })
                        .when(can_redo, |d| {
                            let h = handle.clone();
                            let tip = redo_tooltip.clone().unwrap_or_default();
                            d.child(
                                div()
                                    .id("redo-btn")
                                    .w_6()
                                    .h_6()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .rounded_md()
                                    .hover(move |s| s.bg(hover_bg))
                                    .when(show_tooltips, |d| d.tooltip(Tooltip::with_meta(_cx.i18n().text(Message::Redo), tip)))
                                    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                        h.update(cx, |this, cx| this.execute_redo(cx));
                                    })
                                    .child(
                                        svg()
                                            .path("icons/redo-2.svg")
                                            .size_4()
                                            .text_color(icon_active)
                                    )
                            )
                        })
                )
                .child(
                    div()
                        .w(px(1.0))
                        .h(px(16.0))
                        .mx_3()
                        .bg(theme.border)
                )
        })
        .child(
            div()
                .flex()
                .items_center()
                .flex_1()
                .min_w_0()
                .overflow_hidden()
                .when(view.breadcrumb_editing && view.breadcrumb_pane_id.as_deref() == Some(pane_id.as_str()), |bar| {
                    if let Some(input) = view.editing_view.clone() {
                        let field = div()
                            .w_full()
                            .h_full()
                            .when(view.breadcrumb_invalid, |field| {
                                field.border_1().border_color(theme.error).rounded_sm()
                            })
                            .child(input.clone());
                        let field_shell = div()
                            .flex_1()
                            .min_w_0()
                            .h(px(ROW_HEIGHT))
                            // Reserve room for the shake so the input border stays inside
                            // the breadcrumb's clipped viewport at both horizontal extremes.
                            .px(px(8.0))
                            .flex()
                            .items_center()
                            .child(field);
                        if view.breadcrumb_invalid {
                            let animation_id = SharedString::from(format!("breadcrumb-shake-{}", view.breadcrumb_shake_id));
                            bar.child(field_shell.with_animation(
                                animation_id,
                                Animation::new(std::time::Duration::from_millis(480)),
                                |field_shell, delta| {
                                    let amplitude = (1.0 - delta) * 8.0;
                                    let offset = (delta * std::f32::consts::PI * 8.0).sin() * amplitude;
                                    field_shell.relative().left(px(offset))
                                },
                            ))
                        } else {
                            bar.child(field_shell)
                        }
                    } else { bar }
                })
                .when(!(view.breadcrumb_editing && view.breadcrumb_pane_id.as_deref() == Some(pane_id.as_str())), |bar| bar.child({
                    let mut accumulated_path = String::new();
                    let mut elements: Vec<AnyElement> = Vec::new();

                    for (index, part) in path_parts.iter().enumerate() {
                        let p_id = pane_id.clone();
                        let is_last = index == total_parts - 1;

                        if index == 0 && part.ends_with(':') {
                            accumulated_path = format!("{}/", part);
                        } else {
                            if !accumulated_path.is_empty() && !accumulated_path.ends_with('/') {
                                accumulated_path.push('/');
                            }
                            accumulated_path.push_str(part);
                        }

                        let level_path = accumulated_path.clone();

                        // Chevron separator (not before first element)
                        if index > 0 {
                            let mut parent_path = level_path.clone();
                            if parent_path.ends_with(part) {
                                parent_path = parent_path[..parent_path.len() - part.len()].to_string();
                            }
                            let parent_path_cap = parent_path.clone();

                            elements.push(
                               div()
                                   .relative()
                                   .flex_none()
                                   .child(
                                       div()
                                           .id(SharedString::from(format!("chevron-btn-{}-{}", p_id, index)))
                                           .h(px(24.0))
                                           .w(px(18.0))
                                           .flex()
                                           .items_center()
                                           .justify_center()
                                           .rounded_md()
                                           .hover(move |s| s.bg(hover_bg))
                                           .child(svg().path("icons/chevron-right.svg").size_3().text_color(crumb_sep))
                                           .on_mouse_down(MouseButton::Left, {
                                               let handle = handle.clone();
                                               let p_id_cap = p_id.clone();
                                               let path_cap = parent_path_cap.clone();
                                               let idx_cap = index;
                                               let show_hidden_cap = show_hidden_setting;
                                               let show_system_cap = show_system_setting;
                                               move |_, _, cx: &mut App| {
                                                   let handle = handle.clone();
                                                   let p_id_cap = p_id_cap.clone();
                                                   let path_cap = path_cap.clone();
                                                   cx.spawn(move |cx: &mut AsyncApp| {
                                                       let cx = cx.clone();
                                                       let handle = handle.clone();
                                                       let p_id_cap = p_id_cap.clone();
                                                       let path_cap = path_cap.clone();
                                                       let idx_cap = idx_cap;
                                                       async move {
                                                           if let Ok(dirs) = oxyde_lib::commands::io::list_dir(path_cap.clone(), None, Some(show_hidden_cap), Some(show_system_cap), None).await {
                                                                cx.update(|cx| {
                                                                    handle.update(cx, |this, cx| {
                                                                        let filtered_entries: Vec<FileEntry> = dirs.entries.into_iter().filter(|e| e.is_dir).collect();
                                                                        this.breadcrumb_menu = Some((p_id_cap, path_cap, idx_cap, filtered_entries));
                                                                        cx.notify();
                                                                    });
                                                                });
                                                           }
                                                       }
                                                   }).detach();
                                               }
                                           })
                                   )
                                   .child({
                                       let breadcrumb_menu = view.breadcrumb_menu.clone();
                                       div()
                                            .when_some(breadcrumb_menu, |this: Div, (m_pane_id, menu_path, _, dirs): (String, String, usize, Vec<FileEntry>)| {
                                                if m_pane_id == p_id && menu_path == parent_path_cap {
                                                    let items = dirs.into_iter()
                                                        .map(|entry| {
                                                        let h = handle.clone();
                                                        let p_id = p_id.clone();
                                                        let entry_path = entry.path.clone();
                                                        let is_dimmed = entry.is_hidden || entry.is_system;
                                                        crate::ui::components::dropdown::DropdownItem {
                                                            label: entry.name.clone(),
                                                            icon: Some(IconName::FolderOpen),
                                                            stay_open: false,
                                                            has_submenu: false,
                                                            is_dimmed,
                                                            on_click: std::rc::Rc::new(move |_, app| {
                                                                h.update(app, |this, cx| {
                                                                    this.navigate_to(&p_id, entry_path.clone(), cx);
                                                                    this.breadcrumb_menu = None;
                                                                });
                                                            }),
                                                            on_hover: None,
                                                        }
                                                    }).collect();

                                                    this.child(
                                                        deferred(
                                                            anchored()
                                                                .anchor(Anchor::BottomLeft)
                                                                .child(
                                                                    crate::ui::components::dropdown::GenericDropdown {
                                                                        id: "breadcrumb-dropdown".into(),
                                                                        h_app: handle.clone(),
                                                                        items,
                                                                        reserve_icon_space: true,
                                                                        is_context_menu: false,
                                                                        bottom_padding: 0.0,
                                                                        scroll_handle: view.breadcrumb_scroll.clone(),
                                                                        width: None,
                                                                        max_height: 360.0,
                                                                        header: None,
                                                                        on_close: {
                                                                            let h = handle.clone();
                                                                            std::rc::Rc::new(move |_, app| {
                                                                                h.update(app, |this, cx| {
                                                                                    this.breadcrumb_menu = None;
                                                                                    cx.notify();
                                                                                });
                                                                            })
                                                                        },
                                                                        on_outside: None,
                                                                        on_drag: std::rc::Rc::new(|this, tid, vh, ch, _| {
                                                                           this.drag_target = Some(crate::ui::app::DragTarget::BreadcrumbV {
                                                                               viewport_h: vh,
                                                                               content_h: ch,
                                                                               track_id: tid
                                                                           });
                                                                        }),
                                                                        is_dragging: matches!(view.drag_target, Some(crate::ui::app::DragTarget::BreadcrumbV { .. })),
                                                                        on_item_hover: None,
                                                                        on_width_calculated: None,
                                                                        on_hover_change: None,
                                                                    }
                                                                )
                                                        )
                                                    )
                                                } else {
                                                    this
                                                }
                                            })
                                   })
                                   .into_any_element()
                            );
                        }

                        // Breadcrumb segment label
                        let target_path = level_path.clone();
                        let seg_color = if is_last { crumb_current } else { crumb_parent };

                        let display_label = if index == 0 && part.ends_with(':') {
                            drive_display(part, view)
                        } else {
                            part.to_string()
                        };

                        let seg = div()
                            .id(SharedString::from(format!("breadcrumb-{}-{}", p_id, index)))
                            .flex_none()
                            .h(px(24.0))
                            .px(px(6.0))
                            .flex()
                            .items_center()
                            .rounded_md()
                            .text_color(seg_color)
                            .when(is_last, |d| d.font_weight(FontWeight::MEDIUM))
                            .when(!is_last, |d| d.hover(move |s| s.text_color(theme.text_muted).bg(hover_bg)))
                            .on_mouse_down(MouseButton::Left, {
                                let h = handle.clone();
                                let p = pane_id.clone();
                                let nav_path = target_path.clone();
                                let is_current = is_last;
                                move |event, window, cx| {
                                    if event.click_count >= 2 {
                                        h.update(cx, |this, cx| this.begin_breadcrumb_edit(p.clone(), window, cx));
                                    } else if !is_current {
                                        h.update(cx, |this, cx| this.navigate_to(&p, nav_path.clone(), cx));
                                    }
                                }
                            })
                            .drag_over::<FileDragData>({
                                let h_over = handle.clone();
                                let tgt = target_path.clone();
                                move |style, data, _, cx| {
                                    let default_copy = data.paths.first()
                                        .map(|s| crate::ui::app::drives_differ(s, &tgt))
                                        .unwrap_or(false);
                                    h_over.update(cx, |this, cx| {
                                        if this.drag_target_default_copy != default_copy {
                                            this.drag_target_default_copy = default_copy;
                                            this.update_drag_copy_mode(cx);
                                        }
                                    });
                                    style.bg(theme.selection_hover).text_color(theme.text_on_accent)
                                }
                            })
                            .on_drop({
                                let h_drop = handle.clone();
                                let tgt = target_path.clone();
                                move |data: &FileDragData, _, cx| {
                                    let paths = data.paths.clone();
                                    let target = tgt.clone();
                                    h_drop.update(cx, |this, cx| {
                                        if this.drag_copy_mode {
                                            this.execute_file_copy(paths, target, cx);
                                        } else {
                                            this.execute_file_move(paths, target, cx);
                                        }
                                    });
                                }
                            })
                            .on_mouse_down(MouseButton::Right, {
                                let h = handle.clone();
                                let path = target_path.clone();
                                move |event, _, cx| {
                                    let item = crate::ui::components::dropdown::DropdownItem {
                                        label: cx.i18n().text(crate::ui::i18n::Message::CopyPath).to_string(),
                                        icon: Some(IconName::Copy),
                                        stay_open: false,
                                        has_submenu: false,
                                        is_dimmed: false,
                                        on_click: std::rc::Rc::new({
                                            let h = h.clone();
                                            let path = path.clone();
                                            move |_, app| {
                                                let _ = oxyde_lib::platform::clipboard::set_clipboard_text(path.clone());
                                                h.update(app, |this, cx| { this.context_menus.clear(); cx.notify(); });
                                            }
                                        }),
                                        on_hover: None,
                                    };
                                    h.update(cx, |this, cx| {
                                        this.context_menus.clear();
                                        this.context_menus.push(crate::ui::app::ContextMenuInstance {
                                            position: event.position,
                                            content: crate::ui::app::ContextMenuContent::Items(vec![item], None),
                                            scroll_handle: UniformListScrollHandle::new(),
                                            width: px(0.0), bottom_padding: 3.0, open_left: false,
                                            hovered_item: None, native_request_id: None,
                                        });
                                        cx.notify();
                                    });
                                }
                            })
                            .child(display_label);

                        elements.push(seg.into_any_element());
                    }
                    let blank_handle = handle.clone();
                    let blank_pane = pane_id.clone();
                    elements.push(
                        div()
                            .id("breadcrumb-edit-blank")
                            .flex_1()
                            .min_w(px(24.0))
                            .h_full()
                            .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                                blank_handle.update(cx, |this, cx| {
                                    this.begin_breadcrumb_edit(blank_pane.clone(), window, cx);
                                });
                            })
                            .into_any_element(),
                    );
                    let wheel_scroll = breadcrumb_scroll.clone();
                    let wheel_handle = handle.clone();
                    let measure_handle = handle.clone();
                    let measure_pane = pane_id.clone();
                    let measured_width = breadcrumb_viewport_width.get();
                    let viewport_width = breadcrumb_viewport_width.clone();
                    let resize_scroll = breadcrumb_scroll.clone();
                    div()
                        .relative()
                        .flex()
                        .flex_1()
                        .when(measured_width > 0.0, |bar| bar.w(px(measured_width)).flex_none())
                        .min_w_0()
                        .h_full()
                        .child(canvas(|bounds, _, _| bounds, move |_, bounds: Bounds<Pixels>, window, cx| {
                            measure_handle.update(cx, |this, cx| {
                                if let (Some(origin), Some(size)) = (this.pane_origins.get(&measure_pane), this.pane_sizes.get(&measure_pane)) {
                                    let right = (origin.x + size.width).min(window.bounds().size.width.as_f32());
                                    let available = (right - bounds.origin.x.as_f32() - 16.0).max(0.0);
                                    if (viewport_width.get() - available).abs() > 1.0 {
                                        viewport_width.set(available);
                                        if total_parts > 0 {
                                            if let Some(last) = resize_scroll.bounds_for_item(total_parts * 2 - 1) {
                                                let overflow = (last.right().as_f32() - bounds.origin.x.as_f32() - available).max(0.0);
                                                resize_scroll.set_offset(point(px(-overflow), px(0.0)));
                                            }
                                        }
                                        cx.notify();
                                    }
                                }
                            });
                        }).absolute().size_full())
                        .child(
                            div()
                                .id(SharedString::from(format!("breadcrumb-scroll-{}", pane_id)))
                                .flex()
                                .flex_1()
                                .min_w_0()
                                .items_center()
                                .h_full()
                                .overflow_x_scroll()
                                .track_scroll(&breadcrumb_scroll)
                                .on_scroll_wheel(move |event, window, cx| {
                                    let delta = event.delta.pixel_delta(window.line_height());
                                    let amount = if delta.y.as_f32().abs() > delta.x.as_f32().abs() {
                                        delta.y
                                    } else {
                                        delta.x
                                    };
                                    let max = wheel_scroll.max_offset().x;
                                    if max > px(0.0) && amount != px(0.0) {
                                        let offset = wheel_scroll.offset();
                                        let x = (offset.x + amount * 1.5).clamp(-max, px(0.0));
                                        wheel_scroll.set_offset(point(x, offset.y));
                                        wheel_handle.update(cx, |_, cx| cx.notify());
                                        cx.stop_propagation();
                                    }
                                })
                                .children(elements),
                        )
                        .when(fade_left, |bar| {
                            bar.child(
                                div()
                                    .absolute()
                                    .left_0()
                                    .top_0()
                                    .h_full()
                                    .w(px(24.0))
                                    .bg(linear_gradient(90.0, linear_color_stop(theme.background, 0.0), linear_color_stop(theme.background.alpha(0.0), 1.0))),
                            )
                        })
                        .when(fade_right, |bar| {
                            bar.child(
                                div()
                                    .absolute()
                                    .right_0()
                                    .top_0()
                                    .h_full()
                                    .w(px(24.0))
                                    .bg(linear_gradient(270.0, linear_color_stop(theme.background, 0.0), linear_color_stop(theme.background.alpha(0.0), 1.0))),
                            )
                        })
                }))
        )
}
