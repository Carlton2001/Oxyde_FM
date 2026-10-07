use crate::ui::scale::{logical};
use crate::ui::app::{FileDragData, OxydeApp};
use crate::ui::components::dropdown::IconName;
use crate::ui::components::tooltip::Tooltip;
use crate::ui::i18n::{FluentArgs, I18nContext, Message};

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

fn breadcrumb_root_icon(path: &str, view: &OxydeApp) -> &'static str {
    if path == "trash://" || path.starts_with("trash:\\\\") {
        return "icons/trash-sidebar.svg";
    }
    if path == "__network_vincinity__" {
        return "icons/globe.svg";
    }
    if path.starts_with("\\\\") || path.starts_with("::{") || path.starts_with('?') {
        return "icons/network.svg";
    }
    if path.starts_with("search://") {
        return "icons/search.svg";
    }

    if let Some(drive) = view.state.drives.iter().filter(|drive| {
        std::path::Path::new(path).ancestors().any(|ancestor| {
            crate::ui::components::sidebar::sidebar_paths_equal(
                &ancestor.to_string_lossy(), &drive.path,
            )
        })
    }).max_by_key(|drive| drive.path.len()) {
        return crate::ui::components::icons::drive_icon_path(drive);
    }

    "icons/hard-drive.svg"
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
    let root_icon = breadcrumb_root_icon(&current_path, view);
    let is_trash_path = current_path == "trash://" || current_path.starts_with("trash:\\\\");
    let is_network_neighborhood = current_path == "__network_vincinity__";
    let special_path_label = if is_trash_path {
        Some(_cx.i18n().text(Message::SidebarTrash).to_string())
    } else if is_network_neighborhood {
        Some(_cx.i18n().text(Message::SidebarNetwork).to_string())
    } else if current_path.starts_with("search://") {
        crate::ui::app::search::search_label(&current_path, _cx.i18n())
    } else {
        None
    };
    let theme = *_cx.theme();

    let path_parts: Vec<&str> = if special_path_label.is_some() {
        vec![current_path.as_str()]
    } else {
        current_path
            .split(['/', '\\'])
            .filter(|s: &&str| !s.is_empty())
            .collect()
    };
    let total_parts = path_parts.len();
    let breadcrumb_path_changed = *active_tab.breadcrumb_scroll_path.borrow() != current_path;
    if breadcrumb_path_changed {
        *active_tab.breadcrumb_scroll_path.borrow_mut() = current_path.clone();
        if total_parts == 0 {
            breadcrumb_scroll.set_offset(point(px(0.0), px(0.0)));
        }
        // The scroll handle's dimensions are updated during prepaint.
        _cx.notify();
    }
    let scroll_left = -breadcrumb_scroll.offset().x.as_f32();
    let scroll_max = breadcrumb_scroll.max_offset().x.as_f32();
    let fade_left = scroll_left > logical(2.0);
    let fade_right = scroll_max - scroll_left > logical(2.0);

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
    let parent_path = if special_path_label.is_none() {
        std::path::Path::new(&current_path).parent().filter(|p| !p.as_os_str().is_empty())
            .map(|p| p.to_string_lossy().into_owned())
    } else { None };
    let selected = !active_tab.selected_paths.is_empty();
    let ordinary = special_path_label.is_none()
        && oxyde_lib::utils::archive::split_virtual_path(&current_path).is_none();

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
        .h(crate::ui::style::NAV_BAR_HEIGHT)
        .flex_none()
        .text_size(rems(0.8125))
        .font_weight(crate::ui::style::DEFAULT_FONT_WEIGHT)
        .w_full()
        .flex()
        .items_center()
        .pl(rems(0.5))
        .pr(rems(0.25))
        .when(view.breadcrumb_editing && view.breadcrumb_pane_id.as_deref() == Some(pane_id.as_str()), |bar| bar.pr(rems(0.5)))
        .bg(theme.surface)
        .border_b(rems(0.0625))
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
        .child({
            let mut actions = div().flex().items_center().gap_1().ml_1().flex_shrink_0();
            for (visible, id, icon, label, tip, action) in [
                (parent_path.is_some(), "up-btn", "icons/arrow-up.svg", Message::Up, String::new(), 0u8),
                (!is_trash_path && has_undo_entry, "undo-btn", "icons/undo-2.svg", Message::Undo,
                    if undo_blocked_by_overwrite { _cx.i18n().text(Message::UndoOverwriteUnavailable).to_string() }
                    else { undo_tooltip.clone().unwrap_or_default() }, 1),
                (!is_trash_path && can_redo, "redo-btn", "icons/redo-2.svg", Message::Redo, redo_tooltip.clone().unwrap_or_default(), 2),
                (selected && (ordinary || is_trash_path) && !view.trash_busy, "cut-btn", "icons/scissors.svg", Message::Cut, "Ctrl+X".into(), 3),
                (selected && !is_trash_path && !is_network_neighborhood, "copy-btn", "icons/copy.svg", Message::Copy, "Ctrl+C".into(), 4),
                (ordinary && view.clipboard_has_files, "paste-btn", "icons/clipboard-paste.svg", Message::Paste, "Ctrl+V".into(), 5),
                (selected && !is_trash_path && !is_network_neighborhood && oxyde_lib::utils::archive::split_virtual_path(&current_path).is_none(),
                    "delete-btn", "icons/trash.svg", Message::Delete, String::new(), 6),
                (is_trash_path && selected, "trash-delete-btn", "icons/trash-2.svg", Message::TrashDeletePermanent, String::new(), 7),
                (is_trash_path, "trash-restore-btn", "icons/rotate-ccw.svg", Message::TrashRestoreSelection, String::new(), 8),
                (is_trash_path, "trash-restore-all-btn", "icons/rotate-ccw.svg", Message::TrashRestoreAll, String::new(), 9),
                (is_trash_path, "trash-delete-all-btn", "icons/trash-2.svg", Message::TrashDeleteAll, String::new(), 10),
            ] {
                if !visible { continue; }
                let enabled = (action != 1 || can_undo)
                    && (action < 7 || (!view.trash_busy && if action < 9 { selected } else { !active_tab.files.is_empty() }));
                let mut button = div().id(id).min_w(rems(1.5)).h_6().px(rems(0.125)).flex().items_center().justify_center().rounded_md()
                    .when(show_tooltips && (action == 1 || action == 2), |d| d.tooltip(Tooltip::with_meta(_cx.i18n().text(label), tip.clone())))
                    .when(show_tooltips && action == 6, |d| d.tooltip(Tooltip::for_delete(handle.clone())))
                    .when(show_tooltips && action != 1 && action != 2 && action != 6, |d| d.tooltip(Tooltip::with_shortcut(_cx.i18n().text(label),
                        (!tip.is_empty()).then(|| SharedString::from(tip)))));
                if enabled {
                    let h = handle.clone();
                    let p = pane_id.clone();
                    let parent = parent_path.clone();
                    button = button.hover(move |s| s.bg(hover_bg))
                        .on_mouse_down(MouseButton::Left, move |event, _, cx| {
                            h.update(cx, |this, cx| {
                                this.shift_delete = event.modifiers.shift;
                                this.state.active_pane_id = p.clone();
                                match action {
                                    0 => { if let Some(parent) = &parent { this.navigate_to(&p, parent.clone(), cx); } }
                                    1 => this.execute_undo(cx),
                                    2 => this.execute_redo(cx),
                                    3 => this.execute_couper(cx),
                                    4 => this.execute_copier(cx),
                                    5 => this.execute_coller(cx),
                                    7 => this.execute_trash_action(false, false, cx),
                                    8 => this.execute_trash_action(true, false, cx),
                                    9 => this.execute_trash_action(true, true, cx),
                                    10 => this.execute_trash_action(false, true, cx),
                                    _ => this.execute_supprimer(cx),
                                }
                                cx.notify();
                            });
                        });
                }
                actions = actions.child(button.child(svg().path(icon).size_4()
                    .text_color(if !enabled { icon_disabled } else if action == 7 || action == 10 { theme.error } else { icon_active }))
                    .when(action == 9 || action == 10, |button| button.child(
                        div().ml(rems(0.125)).text_size(rems(0.625)).text_color(theme.text_muted)
                            .child(_cx.i18n().text(Message::TrashAll))
                    )));
            }
            actions
        })
        // The only separator divides the actions from the breadcrumb.
        .child(div().w(rems(0.0625)).h(rems(1.0)).mx_3().flex_shrink_0().bg(theme.border))
        .child(
            div()
                .flex()
                .items_center()
                .flex_1()
                .min_w_0()
                .overflow_hidden()
                .when(view.breadcrumb_editing && view.editing_view.is_some() && view.breadcrumb_pane_id.as_deref() == Some(pane_id.as_str()), |bar| {
                    if let Some(input) = view.editing_view.clone() {
                        let field = div()
                            .flex_1()
                            .min_w_0()
                            .h_full()
                            .when(view.breadcrumb_invalid, |field| {
                                field.border(rems(0.0625)).border_color(theme.error).rounded_sm()
                            })
                            .child(input.clone());
                        let field_shell = div()
                            .flex_1()
                            .min_w_0()
                            .h(px(crate::ui::style::row_height()))
                            .when(view.breadcrumb_invalid, |shell| shell.px(rems(0.5)))
                            .flex()
                            .items_center()
                            .child(field);
                        if view.breadcrumb_invalid {
                            let animation_id = SharedString::from(format!("breadcrumb-shake-{}", view.breadcrumb_shake_id));
                            bar.child(field_shell.with_animation(
                                animation_id,
                                Animation::new(std::time::Duration::from_millis(480)),
                                |field_shell, delta| {
                                    let amplitude = (1.0 - delta) * logical(8.0);
                                    let offset = (delta * std::f32::consts::PI * 8.0).sin() * amplitude;
                                    field_shell.relative().left(px(offset))
                                },
                            ))
                        } else {
                            bar.child(field_shell)
                        }
                    } else { bar }
                })
                .when(
                    !(view.breadcrumb_editing
                        && view.editing_view.is_some()
                        && view.breadcrumb_pane_id.as_deref() == Some(pane_id.as_str())),
                    |bar| {
                    let separator = if current_path.contains("://") { '/' } else { std::path::MAIN_SEPARATOR };
                    let mut accumulated_path = if current_path.starts_with("\\\\") || current_path.starts_with("//") {
                        format!("{separator}{separator}")
                    } else if current_path.starts_with('/') {
                        separator.to_string()
                    } else {
                        String::new()
                    };
                    let mut elements: Vec<AnyElement> = Vec::new();

                    for (index, part) in path_parts.iter().enumerate() {
                        let p_id = pane_id.clone();
                        let is_last = index == total_parts - 1;

                        if index == 0 && special_path_label.is_some() {
                            accumulated_path = current_path.clone();
                        } else if index == 0 && part.ends_with(':') {
                            accumulated_path = format!("{}{separator}", part);
                        } else {
                            if !accumulated_path.is_empty() && !accumulated_path.ends_with(separator) {
                                accumulated_path.push(separator);
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
                                           .h(rems(1.5))
                                           .w(rems(1.125))
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
                                                            icon: Some(if cfg!(target_os = "windows") && view.use_system_icons() {
                                                                IconName::NativeFolder(entry.path.clone())
                                                            } else { IconName::FolderOpen }),
                                                            badge: None,
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
                                                                        max_height: logical(360.0),
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

                        let display_label = if index == 0 {
                            if let Some(label) = special_path_label.as_ref() {
                                label.clone()
                            } else if part.ends_with(':') {
                                drive_display(part, view)
                            } else {
                                part.to_string()
                            }
                        } else {
                            part.to_string()
                        };

                        let seg = div()
                            .id(SharedString::from(format!("breadcrumb-{}-{}", p_id, index)))
                            .flex_none()
                            .h(rems(1.5))
                            .px(rems(0.375))
                            .flex()
                            .items_center()
                            .rounded_md()
                            .text_color(seg_color)
                            .when(!is_last, |d| d.hover(move |s| s.text_color(theme.text_muted).bg(hover_bg)))
                            .on_mouse_down(MouseButton::Left, {
                                let h = handle.clone();
                                let p = pane_id.clone();
                                let nav_path = target_path.clone();
                                let is_current = is_last;
                                move |event, window, cx| {
                                    if event.click_count >= 2 {
                                        cx.stop_propagation();
                                        h.update(cx, |this, cx| this.begin_breadcrumb_edit(p.clone(), window, cx));
                                    } else if !is_current {
                                        h.update(cx, |this, cx| this.navigate_to(&p, nav_path.clone(), cx));
                                    }
                                }
                            })
                            .when(!is_last, |d| d.on_mouse_down(MouseButton::Middle, {
                                let h = handle.clone();
                                let p = pane_id.clone();
                                let path = target_path.clone();
                                move |_, _, cx| {
                                    cx.stop_propagation();
                                    h.update(cx, |this, cx| this.add_tab(&p, path.clone(), false, cx));
                                }
                            }))
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
                                    style.bg(theme.selection_hover).text_color(theme.text)
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
                                let p = pane_id.clone();
                                move |event, _, cx| {
                                    cx.stop_propagation();
                                    let item = crate::ui::components::dropdown::DropdownItem {
                                        label: cx.i18n().text(crate::ui::i18n::Message::CopyPath).to_string(),
                                        icon: Some(IconName::Copy),
                                        badge: None,
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
                                    let mut items = Vec::new();
                                    if !is_last {
                                        items.push(crate::ui::components::dropdown::DropdownItem {
                                            label: cx.i18n().text(Message::OpenNewTab).to_string(),
                                            icon: Some(IconName::ExternalLink),
                                            badge: None,
                                            stay_open: false,
                                            has_submenu: false,
                                            is_dimmed: false,
                                            on_click: std::rc::Rc::new({
                                                let h = h.clone();
                                                let path = path.clone();
                                                let p = p.clone();
                                                move |_, app| {
                                                    h.update(app, |this, cx| {
                                                        this.context_menus.clear();
                                                        this.add_tab(&p, path.clone(), false, cx);
                                                    });
                                                }
                                            }),
                                            on_hover: None,
                                        });
                                    }
                                    items.push(item);
                                    h.update(cx, |this, cx| {
                                        this.context_menus.clear();
                                        this.context_menus.push(crate::ui::app::ContextMenuInstance {
                                            position: event.position,
                                            content: crate::ui::app::ContextMenuContent::Items(items, None),
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
                            .min_w(rems(1.5))
                            .h_full()
                            .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                                cx.stop_propagation();
                                blank_handle.update(cx, |this, cx| {
                                    this.begin_breadcrumb_edit(blank_pane.clone(), window, cx);
                                });
                            })
                            .into_any_element(),
                    );
                    let wheel_scroll = breadcrumb_scroll.clone();
                    let wheel_handle = handle.clone();
                    let measure_handle = handle.clone();
                    let viewport_width = breadcrumb_viewport_width.clone();
                    let resize_scroll = breadcrumb_scroll.clone();
                    let auto_scroll_to_end = breadcrumb_path_changed;
                    bar.child(
                        div()
                            .w_4()
                            .h_4()
                            .mr_2()
                            .flex()
                            .items_center()
                            .justify_center()
                            .flex_none()
                            .child(svg().path(root_icon).size_4().text_color(if total_parts == 1 { crumb_current } else { crumb_parent })),
                    )
                    .child(div()
                        .relative()
                        .flex()
                        .flex_1()
                        .min_w_0()
                        .h_full()
                        .child(canvas(|bounds, _, _| bounds, move |_, bounds: Bounds<Pixels>, _, cx| {
                            measure_handle.update(cx, |_, cx| {
                                {
                                    let available = bounds.size.width.as_f32().max(0.0);
                                    if auto_scroll_to_end {
                                        resize_scroll.set_offset(point(-resize_scroll.max_offset().x, px(0.0)));
                                    }
                                    if (viewport_width.get() - available).abs() > 1.0 {
                                        viewport_width.set(available);
                                        if total_parts > 0 {
                                            resize_scroll.scroll_to_item(total_parts * 2 - 2);
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
                                    .w(rems(1.5))
                                    .bg(linear_gradient(logical(90.0), linear_color_stop(theme.surface, 0.0), linear_color_stop(theme.surface.alpha(0.0), 1.0))),
                            )
                        })
                        .when(fade_right, |bar| {
                            bar.child(
                                div()
                                    .absolute()
                                    .right_0()
                                    .top_0()
                                    .h_full()
                                    .w(rems(1.5))
                                    .bg(linear_gradient(logical(270.0), linear_color_stop(theme.surface, 0.0), linear_color_stop(theme.surface.alpha(0.0), 1.0))),
                            )
                        }))
                    },
                )
        )
        .when(!is_trash_path && !is_network_neighborhood && !current_path.starts_with("::{"), |bar| {
            bar.child(view.render_quick_search_control(&pane_id, _cx))
        })
}
