use crate::ui::app::{
    ContextMenuContent, ContextMenuInstance, DragTarget, FileDragData, FileDragPreview, OxydeApp,
};
use crate::ui::components::scrollbar::{render_scrollbar, ScrollbarAxis, ZED_TRACK_WIDTH};
use crate::ui::components::tooltip::Tooltip;
use crate::ui::style::{format_date, format_size, ROW_HEIGHT};
use crate::ui::theme::{OxydeTheme, ThemeContext};
use crate::ui::workspace::{SortColumn, SortDirection};
use gpui::prelude::*;
use gpui::*;
use oxyde_lib::models::FileEntry;

/// Returns the display type label for a file entry.
/// Symlinks and junctions are identified before extension/dir detection.
fn file_type_label(f: &FileEntry) -> String {
    if f.is_symlink {
        return "Symlink".to_string();
    }
    if f.is_junction {
        return "Junction".to_string();
    }
    if f.is_dir {
        return "Dossier".to_string();
    }
    std::path::Path::new(&f.name)
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_uppercase())
        .unwrap_or_else(|| "Fichier".to_string())
}

pub fn render_file_list(
    view: &OxydeApp,
    pane_id: String,
    handle: Entity<OxydeApp>,
    cx: &mut Context<OxydeApp>,
) -> impl IntoElement {
    let p_id_cap = pane_id.clone();
    let theme = cx.theme();
    let pane = view.state.layout.find_pane(&pane_id).expect("pane exists");
    let active_tab = pane.active_tab();
    let files = active_tab.filtered_files().to_vec();
    let count = files.len() as f32;

    let pane_size = view.pane_sizes.get(&pane_id).cloned().unwrap_or(Size {
        width: view.list_width,
        height: view.list_height,
    });
    let pane_viewport_w = pane_size.width.max(1.0);

    let list_container_h = (pane_size.height - 64.0).max(1.0);
    let v_v_h = (list_container_h - 28.0).max(1.0);
    let content_h = (count * ROW_HEIGHT).max(1.0);

    let type_w = active_tab.type_col_width;
    let size_w = active_tab.size_col_width;
    let date_w = active_tab.date_col_width;

    let fixed_w = type_w + size_w + date_w;
    let name_w = (pane_viewport_w - fixed_w - 28.0).max(80.0);
    let total_w = name_w + fixed_w;

    let is_active_pane = view.state.active_pane_id == pane_id;
    let is_dragging_v = matches!(view.drag_target, Some(DragTarget::FileListV { ref pane_id, .. }) if pane_id == &p_id_cap);
    let show_tooltips = view.context_menus.is_empty();
    let show_media_previews = view.show_media_previews;

    let selected_paths = active_tab.selected_paths.clone();

    let mut container = div()
        .flex_1()
        .flex()
        .flex_col()
        .bg(if is_active_pane { theme.active_background } else { theme.background })
        .overflow_hidden()
        .relative()
        .child(
            canvas(
                |bounds, _, _| bounds,
                {
                    let h = handle.clone();
                    let p_id = pane_id.clone();
                    move |_, bounds: Bounds<Pixels>, _, cx| {
                        let b = Bounds {
                            origin: Point { x: bounds.origin.x.as_f32(), y: bounds.origin.y.as_f32() },
                            size: Size { width: bounds.size.width.as_f32(), height: bounds.size.height.as_f32() },
                        };
                        h.update(cx, |this, _| {
                            this.list_bounds.insert(p_id.clone(), b);
                        });
                    }
                }
            ).absolute().size_full()
        )
        // 1. Background fixe du header (en dessous)
        .child(
            div()
                .absolute()
                .top_0()
                .left_0()
                .right_0()
                .h(px(ROW_HEIGHT))
                .bg(theme.header)
                .border_b_1()
                .border_color(theme.border)
        )
        // 2. Contenu défilant (Headers + List)
        .child(
            div()
                .size_full()
                .child(
                    div()
                        .absolute()
                        .left(active_tab.list_scroll_h.offset().x + px(8.0))
                        .w(px(total_w))
                        .h_full()
                        .flex()
                        .flex_col()
                        .child(render_headers(view, pane_id.clone(), handle.clone(), name_w, type_w, size_w, date_w, theme))
                        .child(
                            uniform_list(
                                SharedString::from(format!("file-list-{}", pane_id)),
                                files.len(),
                                {
                                    let files = files.clone();
                                    let h_inner = handle.clone();
                                    let p_id_for_list = pane_id.clone();
                                    let selected_paths = selected_paths.clone();
                                    move |range, _, cx| {
                                        range.map(|i| {
                                            let theme = OxydeTheme::global(cx);
                                            let file = &files[i];
                                            let is_dir = file.is_dir;
                                            let path = file.path.clone();
                                            let is_selected = selected_paths.contains(&path);
                                            let p_id = p_id_for_list.clone();
                                            let h = h_inner.clone();

                                            {
                                            let file_name_tt = file.name.clone();
                                            let path_tt = path.clone();
                                            let ext_tt = file_type_label(file);
                                            let size_tt = format_size(file.size, is_dir, file.is_calculated);
                                            let date_tt = format_date(file.modified);
                                            let is_dimmed = file.is_hidden || file.is_system;
                                            div()
                                                .id(SharedString::from(format!("file-row-{}", path)))
                                                .flex()
                                                .items_center()
                                                .h(px(ROW_HEIGHT))
                                                .w(px(total_w))
                                                .bg(if is_selected { theme.selected } else if is_active_pane { theme.active_background } else { theme.background })
                                                .when(is_dimmed, |d| d.opacity(0.5))
                                                .when(!is_selected, |s| s.hover(|s| s.bg(theme.hover)))
                                                .when(show_tooltips && show_media_previews, |d| d.tooltip(Tooltip::for_file_full(file_name_tt.clone(), path_tt, ext_tt.clone(), size_tt.clone(), date_tt.clone())))
                                                .when(show_tooltips && !show_media_previews, |d| d.tooltip(Tooltip::for_file_no_preview(file_name_tt, ext_tt, size_tt, date_tt)))
                                                .on_mouse_down(MouseButton::Left, {
                                                    let p = path.clone();
                                                    let h_inner_inner = h.clone();
                                                    let p_id_cap = p_id.clone();
                                                    move |e, _, cx| {
                                                        cx.stop_propagation();
                                                        let multi = e.modifiers.control;
                                                        let shift = e.modifiers.shift;
                                                        h_inner_inner.update(cx, |this, cx| {
                                                            this.state.active_pane_id = p_id_cap.clone();
                                                            // If file is already part of a multi-selection and no modifier
                                                            // is held, preserve the selection so dragging all files works.
                                                            // on_click will narrow the selection if no drag happens.
                                                            let preserve = !multi && !shift && this.state
                                                                .find_pane(&p_id_cap)
                                                                .map(|pane| {
                                                                    let sel = &pane.active_tab().selected_paths;
                                                                    sel.len() > 1 && sel.contains(&p)
                                                                })
                                                                .unwrap_or(false);
                                                            if !preserve {
                                                                this.toggle_selection(&p_id_cap, p.clone(), multi, shift, cx);
                                                            }
                                                        });
                                                        if e.click_count == 2 && is_dir {
                                                            h_inner_inner.update(cx, |v, cx| v.navigate_to(&p_id_cap, p.clone(), cx));
                                                        }
                                                    }
                                                })
                                                .on_click({
                                                    let h = h.clone();
                                                    let p = path.clone();
                                                    let p_id_cap = p_id.clone();
                                                    move |event, _, cx| {
                                                        // Only narrow when no modifier: click without drag on a
                                                        // multi-selected file should deselect the others.
                                                        if event.modifiers().control || event.modifiers().shift {
                                                            return;
                                                        }
                                                        h.update(cx, |this, cx| {
                                                            let is_multi = this.state.find_pane(&p_id_cap)
                                                                .map(|pane| pane.active_tab().selected_paths.len() > 1)
                                                                .unwrap_or(false);
                                                            if is_multi {
                                                                if let Some(pane) = this.state.find_pane_mut(&p_id_cap) {
                                                                    let tab = pane.active_tab_mut();
                                                                    tab.selected_paths.clear();
                                                                    tab.selected_paths.insert(p.clone());
                                                                    tab.selection_anchor = Some(p.clone());
                                                                }
                                                                cx.notify();
                                                            }
                                                        });
                                                    }
                                                })
                                                .on_mouse_down(MouseButton::Middle, {
                                                    let p = path.clone();
                                                    let h_inner_inner = h.clone();
                                                    let p_id_cap = p_id.clone();
                                                    move |_, _, cx| {
                                                        cx.stop_propagation();
                                                        if is_dir {
                                                            h_inner_inner.update(cx, |v, cx| v.add_tab(&p_id_cap, p.clone(), false, cx));
                                                        }
                                                    }
                                                })
                                                .on_mouse_down(MouseButton::Right, {
                                                     let h_inner_inner = h.clone();
                                                     let p_id_cap = p_id.clone();
                                                     let p_for_right = path.clone();
                                                     move |e, _, cx| {
                                                         cx.stop_propagation();
                                                         let pos = e.position;
                                                         let p_id_cap_cap = p_id_cap.clone();
                                                         let p_for_right_cap = p_for_right.clone();
                                                          h_inner_inner.update(cx, |this, cx| {
                                                              this.state.active_pane_id = p_id_cap_cap.clone();
                                                              if !this.state.find_pane(&p_id_cap_cap).map(|p| p.active_tab().selected_paths.contains(&p_for_right_cap)).unwrap_or(false) {
                                                                  this.toggle_selection(&p_id_cap_cap, p_for_right_cap.clone(), false, false, cx);
                                                              }
                                                              this.context_menus.clear();
                                                              this.context_menus.push(ContextMenuInstance {
                                                                  position: pos,
                                                                  content: ContextMenuContent::Items(super::oxyde_context_menu_items(h_inner_inner.clone(), is_dir, p_for_right_cap.clone()), None),
                                                                  scroll_handle: UniformListScrollHandle::new(),
                                                                  width: px(0.0),
                                    open_left: false,
                                                              });
                                                              cx.notify();
                                                          });
                                                     }
                                                 })
                                                .on_drag({
                                                    let drag_paths = if is_selected && !selected_paths.is_empty() {
                                                        selected_paths.iter().cloned().collect::<Vec<_>>()
                                                    } else {
                                                        vec![path.clone()]
                                                    };
                                                    FileDragData { paths: drag_paths.clone(), source_pane_id: p_id.clone() }
                                                }, {
                                                    let drag_paths = if is_selected && !selected_paths.is_empty() {
                                                        selected_paths.iter().cloned().collect::<Vec<_>>()
                                                    } else {
                                                        vec![path.clone()]
                                                    };
                                                    let h_drag = h.clone();
                                                    move |_, cursor_offset, _window, cx| {
                                                        let app = h_drag.clone();
                                                        let paths = drag_paths.clone();
                                                        cx.new(|cx| FileDragPreview::new(paths, cursor_offset, app, cx))
                                                    }
                                                })
                                                .when(is_dir, |row| {
                                                    let h_drop = h.clone();
                                                    let target_path = path.clone();
                                                    row
                                                        .drag_over::<FileDragData>({
                                                            let h_over = h.clone();
                                                            let tgt = path.clone();
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
                                                                style.bg(rgba(0x0078d440))
                                                            }
                                                        })
                                                        .on_drop(move |data: &FileDragData, _, cx| {
                                                            let paths = data.paths.clone();
                                                            let target = target_path.clone();
                                                            h_drop.update(cx, |this, cx| {
                                                                if this.drag_copy_mode {
                                                                    this.execute_file_copy(paths, target, cx);
                                                                } else {
                                                                    this.execute_file_move(paths, target, cx);
                                                                }
                                                            });
                                                        })
                                                })
                                                .child({
                                                    let path_str = path.clone();
                                                    let file_name = file.name.clone();

                                                    div()
                                                        .w(px(name_w))
                                                        .h_full()
                                                        .flex()
                                                        .items_center()
                                                        .overflow_hidden()
                                                        .child(
                                                            div()
                                                                .id(SharedString::from(format!("file-content-{}", path_str)))
                                                                .h_full()
                                                                .px(px(8.0))
                                                                .flex()
                                                                .items_center()
                                                                .child({
                                                                    let icon_type = crate::ui::components::icons::get_icon_type(&file_name, is_dir, Some(&path_str), file.is_symlink, file.is_junction);
                                                                    svg()
                                                                        .path(icon_type.path())
                                                                        .size_4()
                                                                        .flex_none()
                                                                        .mr_2()
                                                                        .text_color(if is_dir { theme.accent } else { theme.text_muted })
                                                                })
                                                                .child(
                                                                    div()
                                                                        .flex_1()
                                                                        .overflow_hidden()
                                                                        .child({
                                                                            let app_view = h.read(cx);
                                                                            if app_view.editing_path.as_ref() == Some(&path_str) {
                                                                                if let Some(edit_view) = &app_view.editing_view {
                                                                                    div().h(px(ROW_HEIGHT)).w_full().child(edit_view.clone()).into_any_element()
                                                                                } else {
                                                                                    div().child(file_name.clone()).into_any_element()
                                                                                }
                                                                            } else {
                                                                                div()
                                                                                    .text_ellipsis()
                                                                                    .font_weight(FontWeight::MEDIUM)
                                                                                    .text_color(theme.text)
                                                                                    .child(file_name.clone())
                                                                                    .into_any_element()
                                                                            }
                                                                        })
                                                                )
                                                        )
                                                })
                                                .child(div().w(px(type_w)).px(px(8.0)).text_color(theme.text_muted).text_ellipsis().text_size(px(12.0)).child(file_type_label(file)))
                                                .child(div().w(px(size_w)).px(px(8.0)).flex().justify_end().text_color(theme.text_muted).text_ellipsis().text_size(px(12.0)).child(format_size(file.size, is_dir, file.is_calculated)))
                                                .child(div().w(px(date_w)).px(px(8.0)).text_color(theme.text_muted).text_ellipsis().text_size(px(12.0)).child(format_date(file.modified)))
                                            }}).collect::<Vec<_>>()
                                    }
                                }
                            )
                            .on_mouse_down(MouseButton::Left, {
                                let h = handle.clone();
                                let p_id_cap = pane_id.clone();
                                move |e, _, cx| {
                                    let pos = e.position;
                                    let modifiers = e.modifiers;
                                    h.update(cx, |this, cx| {
                                        this.state.active_pane_id = p_id_cap.clone();
                                        if !modifiers.control && !modifiers.shift {
                                            if let Some(pane) = this.state.find_pane_mut(&p_id_cap) {
                                                let tab = pane.active_tab_mut();
                                                tab.selected_paths.clear();
                                                tab.selection_anchor = None;
                                            }
                                        }
                                        this.drag_target = Some(DragTarget::SelectionRectangle {
                                            pane_id: p_id_cap.clone(),
                                            start_pos: pos,
                                            current_pos: pos,
                                            modifiers,
                                            initial_selection: this.state.find_pane(&p_id_cap).map(|p| p.active_tab().selected_paths.clone()).unwrap_or_default(),
                                        });
                                        cx.notify();
                                    });
                                }
                            })
                            .on_mouse_down(MouseButton::Right, {
                                 let h = handle.clone();
                                 let p_id_cap = pane_id.clone();
                                 move |e, _, cx| {
                                     let pos = e.position;
                                     let p_id_cap_cap = p_id_cap.clone();
                                      h.update(cx, |this, cx| {
                                          this.state.active_pane_id = p_id_cap_cap.clone();
                                          this.context_menus.clear();
                                          this.context_menus.push(ContextMenuInstance {
                                              position: pos,
                                              content: ContextMenuContent::Items(super::pane_context_menu_items(h.clone(), p_id_cap_cap), None),
                                              scroll_handle: UniformListScrollHandle::new(),
                                              width: px(0.0),
                                    open_left: false,
                                          });
                                          cx.notify();
                                      });
                                 }
                             })
                            .drag_over::<FileDragData>({
                                let p_id_cap = pane_id.clone();
                                let h_over = handle.clone();
                                let current_path = active_tab.path.clone();
                                move |style, data, _, cx| {
                                    if data.source_pane_id != p_id_cap {
                                        let default_copy = data.paths.first()
                                            .map(|s| crate::ui::app::drives_differ(s, &current_path))
                                            .unwrap_or(false);
                                        h_over.update(cx, |this, cx| {
                                            if this.drag_target_default_copy != default_copy {
                                                this.drag_target_default_copy = default_copy;
                                                this.update_drag_copy_mode(cx);
                                            }
                                        });
                                        style.bg(rgba(0x0078d420))
                                    } else {
                                        style
                                    }
                                }
                            })
                            .on_drop({
                                let h = handle.clone();
                                let p_id_cap = pane_id.clone();
                                move |data: &FileDragData, _, cx| {
                                    let paths = data.paths.clone();
                                    h.update(cx, |this, cx| {
                                        let target = this.state.find_pane(&p_id_cap)
                                            .map(|p| p.active_tab().path.clone())
                                            .unwrap_or_default();
                                        if !target.is_empty() {
                                            if this.drag_copy_mode {
                                                this.execute_file_copy(paths, target, cx);
                                            } else {
                                                this.execute_file_move(paths, target, cx);
                                            }
                                        }
                                    });
                                }
                            })
                            .track_scroll(&active_tab.list_scroll)
                            .size_full()
                        )
                )
        )
        // 3. Gutters interactifs
        .child(
            div()
                .absolute()
                .top(px(ROW_HEIGHT))
                .left_0()
                .bottom_0()
                .w(px(8.0))
                .on_mouse_down(MouseButton::Left, {
                    let h = handle.clone();
                    let p_id_cap = pane_id.clone();
                    move |e, _, cx| {
                        cx.stop_propagation();
                        let pos = e.position;
                        let modifiers = e.modifiers;
                        h.update(cx, |this, cx| {
                            this.state.active_pane_id = p_id_cap.clone();
                            if !modifiers.control && !modifiers.shift {
                                if let Some(pane) = this.state.find_pane_mut(&p_id_cap) {
                                    let tab = pane.active_tab_mut();
                                    tab.selected_paths.clear();
                                    tab.selection_anchor = None;
                                }
                            }
                            this.drag_target = Some(DragTarget::SelectionRectangle {
                                pane_id: p_id_cap.clone(),
                                start_pos: pos,
                                current_pos: pos,
                                modifiers,
                                initial_selection: this.state.find_pane(&p_id_cap).map(|p| p.active_tab().selected_paths.clone()).unwrap_or_default(),
                            });
                            cx.notify();
                            });
                    }
                })
        )
        .child(
            div()
                .absolute()
                .top(px(ROW_HEIGHT))
                .right(px(ZED_TRACK_WIDTH))
                .bottom_0()
                .w(px(8.0))
                .on_mouse_down(MouseButton::Left, {
                    let h = handle.clone();
                    let p_id_cap = pane_id.clone();
                    move |e, _, cx| {
                        cx.stop_propagation();
                        let pos = e.position;
                        let modifiers = e.modifiers;
                        h.update(cx, |this, cx| {
                            this.state.active_pane_id = p_id_cap.clone();
                            if !modifiers.control && !modifiers.shift {
                                if let Some(pane) = this.state.find_pane_mut(&p_id_cap) {
                                    let tab = pane.active_tab_mut();
                                    tab.selected_paths.clear();
                                    tab.selection_anchor = None;
                                }
                            }
                            this.drag_target = Some(DragTarget::SelectionRectangle {
                                pane_id: p_id_cap.clone(),
                                start_pos: pos,
                                current_pos: pos,
                                modifiers,
                                initial_selection: this.state.find_pane(&p_id_cap).map(|p| p.active_tab().selected_paths.clone()).unwrap_or_default(),
                            });
                            cx.notify();
                            });
                    }
                })
        )
    ;

    // Selection Rectangle
    if let Some(DragTarget::SelectionRectangle {
        pane_id: ref drag_pane_id,
        start_pos,
        current_pos,
        ..
    }) = view.drag_target
    {
        if drag_pane_id == &pane_id {
            let list_origin = view
                .list_bounds
                .get(&pane_id)
                .map(|b| b.origin)
                .unwrap_or(Point { x: 0.0, y: 0.0 });

            let rect_x = start_pos.x.as_f32().min(current_pos.x.as_f32()) - list_origin.x;
            let rect_y = start_pos.y.as_f32().min(current_pos.y.as_f32()) - list_origin.y;
            let rect_w = (start_pos.x.as_f32() - current_pos.x.as_f32()).abs();
            let rect_h = (start_pos.y.as_f32() - current_pos.y.as_f32()).abs();

            container = container.child(
                div()
                    .absolute()
                    .left(px(rect_x))
                    .top(px(rect_y))
                    .w(px(rect_w))
                    .h(px(rect_h))
                    .bg(theme.selected)
                    .border_1()
                    .border_color(theme.accent),
            );
        }
    }

    let pane_suffix = format!("pane-{}", pane_id);
    let v_offset = -active_tab
        .list_scroll
        .0
        .borrow()
        .base_handle
        .offset()
        .y
        .as_f32();

    // Vertical scrollbar overlay
    container = container.child(
        div()
            .absolute()
            .top(px(ROW_HEIGHT))
            .right_0()
            .bottom_0()
            .w(px(ZED_TRACK_WIDTH))
            .child(render_scrollbar(
                handle.clone(),
                ScrollbarAxis::Vertical,
                v_v_h,
                content_h,
                v_offset,
                is_dragging_v,
                format!("v-{}", pane_suffix),
                None,
                {
                    let p_id = p_id_cap.clone();
                    move |this, track_id, v_h, c_h, _| {
                        this.drag_target = Some(DragTarget::FileListV {
                            pane_id: p_id.clone(),
                            viewport_h: v_h,
                            content_h: c_h,
                            track_id,
                        });
                    }
                },
            )),
    );

    // Horizontal scrollbar overlay
    container = container.child(
        div().absolute().left_0().bottom_0().right(px(ZED_TRACK_WIDTH)).h(px(ZED_TRACK_WIDTH))
            .child(render_scrollbar(handle.clone(), ScrollbarAxis::Horizontal, pane_viewport_w, total_w + 28.0, -active_tab.list_scroll_h.offset().x.as_f32(), matches!(view.drag_target, Some(DragTarget::FileListH { ref pane_id, .. }) if pane_id == &p_id_cap), format!("h-{}", pane_suffix), None, {
                let p_id = p_id_cap.clone();
                move |this, track_id, v_w, c_w, _| {
                    this.drag_target = Some(DragTarget::FileListH { pane_id: p_id.clone(), viewport_w: v_w, content_w: c_w, track_id });
                }
            }))
    );

    container
}

fn render_headers(
    view: &OxydeApp,
    pane_id: String,
    handle: Entity<OxydeApp>,
    name_w: f32,
    type_w: f32,
    size_w: f32,
    date_w: f32,
    theme: &OxydeTheme,
) -> impl IntoElement {
    let pane = view.state.layout.find_pane(&pane_id).expect("pane exists");
    let tab = pane.active_tab();
    let current_col = tab.sort_column;
    let current_dir = tab.sort_direction;

    // Check filters
    let name_filtered = !tab.filter_name.is_empty();

    let type_filtered = if tab.filter_types.is_empty() {
        false
    } else {
        let mut all_types = std::collections::HashSet::new();
        for f in &tab.files {
            all_types.insert(file_type_label(f));
        }
        !all_types.iter().all(|t| tab.filter_types.contains(t))
    };

    let size_filtered = if tab.filter_sizes.is_empty() {
        false
    } else {
        use crate::ui::workspace::SizeCategory;
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
        !present.iter().all(|c| tab.filter_sizes.contains(c))
    };

    let date_filtered = if tab.filter_dates.is_empty() {
        false
    } else {
        use crate::ui::workspace::DateCategory;
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
        !present.iter().all(|c| tab.filter_dates.contains(c))
    };

    div()
        .flex()
        .items_center()
        .h(px(ROW_HEIGHT))
        .text_color(theme.text_muted)
        .font_weight(FontWeight::BOLD)
        .child(
            header_cell(
                "Nom",
                name_w,
                current_col == SortColumn::Name,
                current_dir,
                name_filtered,
                theme,
                {
                    let h = handle.clone();
                    let pid = pane_id.clone();
                    move |_, _, cx| {
                        h.update(cx, |this, cx| this.toggle_sort(&pid, SortColumn::Name, cx));
                    }
                },
            )
            .on_mouse_down(MouseButton::Right, {
                let h = handle.clone();
                let pid = pane_id.clone();
                move |e, window, cx| {
                    let pos = e.position;
                    h.update(cx, |this, cx| this.open_name_filter(&pid, pos, window, cx));
                }
            }),
        )
        .child(
            header_cell(
                "Type",
                type_w,
                current_col == SortColumn::Type,
                current_dir,
                type_filtered,
                theme,
                {
                    let h = handle.clone();
                    let pid = pane_id.clone();
                    move |_, _, cx| {
                        h.update(cx, |this, cx| this.toggle_sort(&pid, SortColumn::Type, cx));
                    }
                },
            )
            .on_mouse_down(MouseButton::Right, {
                let h = handle.clone();
                let pid = pane_id.clone();
                move |e, _, cx| {
                    let pos = e.position;
                    h.update(cx, |this, cx| this.open_type_filter_menu(&pid, pos, cx));
                }
            }),
        )
        .child(
            header_cell(
                "Taille",
                size_w,
                current_col == SortColumn::Size,
                current_dir,
                size_filtered,
                theme,
                {
                    let h = handle.clone();
                    let pid = pane_id.clone();
                    move |_, _, cx| {
                        h.update(cx, |this, cx| this.toggle_sort(&pid, SortColumn::Size, cx));
                    }
                },
            )
            .on_mouse_down(MouseButton::Right, {
                let h = handle.clone();
                let pid = pane_id.clone();
                move |e, _, cx| {
                    let pos = e.position;
                    h.update(cx, |this, cx| this.open_size_filter_menu(&pid, pos, cx));
                }
            }),
        )
        .child(
            header_cell(
                "Modifié le",
                date_w,
                current_col == SortColumn::Modified,
                current_dir,
                date_filtered,
                theme,
                {
                    let h = handle.clone();
                    let pid = pane_id.clone();
                    move |_, _, cx| {
                        h.update(cx, |this, cx| {
                            this.toggle_sort(&pid, SortColumn::Modified, cx)
                        });
                    }
                },
            )
            .on_mouse_down(MouseButton::Right, {
                let h = handle.clone();
                let pid = pane_id.clone();
                move |e, _, cx| {
                    let pos = e.position;
                    h.update(cx, |this, cx| this.open_date_filter_menu(&pid, pos, cx));
                }
            }),
        )
}

fn header_cell(
    label: &str,
    width: f32,
    is_active: bool,
    direction: SortDirection,
    is_filtered: bool,
    theme: &OxydeTheme,
    on_click: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
) -> Div {
    div()
        .w(px(width))
        .px(px(8.0))
        .h_full()
        .flex()
        .items_center()
        .child(
            div()
                .flex_1()
                .overflow_hidden()
                .text_ellipsis()
                .child(label.to_string()),
        )
        .child(div().ml_1().when(is_filtered, |s| {
            s.child(
                svg()
                    .path("icons/filter.svg")
                    .size_3()
                    .text_color(theme.accent),
            )
        }))
        .child(
            div()
                .w(px(16.0))
                .flex()
                .justify_center()
                .when(is_active, |s| {
                    s.child(
                        svg()
                            .path(if direction == SortDirection::Ascending {
                                "icons/arrow-up.svg"
                            } else {
                                "icons/arrow-down.svg"
                            })
                            .size_3()
                            .text_color(theme.accent),
                    )
                }),
        )
        .hover(|s| s.bg(rgba(0xffffff05)))
        .on_mouse_down(MouseButton::Left, on_click)
}

pub fn get_type_filter_items(
    h: Entity<OxydeApp>,
    pid: String,
    tab_files: Vec<FileEntry>,
    current_filters: std::collections::HashSet<String>,
    pos: Point<Pixels>,
) -> Vec<crate::ui::components::dropdown::DropdownItem> {
    use crate::ui::components::dropdown::{DropdownItem, IconName};

    let mut exts = std::collections::HashSet::new();
    for f in &tab_files {
        exts.insert(file_type_label(f));
    }
    let mut sorted_exts: Vec<String> = exts.into_iter().collect();
    sorted_exts.sort();

    let mut items = Vec::new();

    for ext in sorted_exts {
        let h_ext = h.clone();
        let pid_ext = pid.clone();
        let e_cap = ext.clone();
        let is_sel = current_filters.contains(&e_cap);

        items.push(DropdownItem {
            label: ext,
            icon: Some(if is_sel {
                IconName::Check
            } else {
                IconName::Square
            }),
            stay_open: true,
            has_submenu: false,
            is_dimmed: false,
            on_click: std::rc::Rc::new(move |_, app| {
                let pid_cap = pid_ext.clone();
                let e_inner = e_cap.clone();
                h_ext.update(app, |this, cx| {
                    this.toggle_filter_type(&pid_cap, e_inner, cx);
                    this.open_type_filter_menu(&pid_cap, pos, cx);
                });
            }),
            on_hover: None,
        });
    }

    items
}

pub fn get_date_filter_items(
    handle: Entity<OxydeApp>,
    pane_id: String,
    files: Vec<oxyde_lib::models::FileEntry>,
    current_filters: std::collections::HashSet<crate::ui::workspace::DateCategory>,
    pos: Point<Pixels>,
) -> Vec<crate::ui::components::dropdown::DropdownItem> {
    use crate::ui::components::dropdown::{DropdownItem, IconName};
    use crate::ui::workspace::DateCategory;
    use std::rc::Rc;

    let categories = [
        DateCategory::Aujourdhui,
        DateCategory::Hier,
        DateCategory::CetteSemaine,
        DateCategory::LaSemaineDerniere,
        DateCategory::PlusAncien,
    ];

    let mut present_categories = std::collections::HashSet::new();
    for f in &files {
        for cat in &categories {
            if cat.matches(f.modified) {
                present_categories.insert(*cat);
                break;
            }
        }
    }

    let mut items = Vec::new();

    for cat in categories {
        if !present_categories.contains(&cat) {
            continue;
        }

        let h = handle.clone();
        let pid = pane_id.clone();
        let active = current_filters.contains(&cat);
        let c_cat = cat;

        items.push(DropdownItem {
            label: cat.label().to_string(),
            icon: Some(if active {
                IconName::Check
            } else {
                IconName::Square
            }),
            stay_open: true,
            has_submenu: false,
            is_dimmed: false,
            on_click: Rc::new(move |_, cx| {
                let pid_inner = pid.clone();
                h.update(cx, |this, cx| {
                    this.update_filter_date(&pid_inner, c_cat, !active, cx);
                    this.open_date_filter_menu(&pid_inner, pos, cx);
                });
            }),
            on_hover: None,
        });
    }

    items
}

pub fn get_size_filter_items(
    handle: Entity<OxydeApp>,
    pane_id: String,
    files: Vec<oxyde_lib::models::FileEntry>,
    current_filters: std::collections::HashSet<crate::ui::workspace::SizeCategory>,
    pos: Point<Pixels>,
) -> Vec<crate::ui::components::dropdown::DropdownItem> {
    use crate::ui::components::dropdown::{DropdownItem, IconName};
    use crate::ui::workspace::SizeCategory;
    use std::rc::Rc;

    let categories = [
        SizeCategory::Minuscule,
        SizeCategory::Petit,
        SizeCategory::Moyen,
        SizeCategory::Grand,
        SizeCategory::Huge,
    ];

    let mut present_categories = std::collections::HashSet::new();
    for f in &files {
        for cat in &categories {
            if cat.matches(f.size) {
                present_categories.insert(*cat);
                break;
            }
        }
    }

    let mut items = Vec::new();

    for cat in categories {
        if !present_categories.contains(&cat) {
            continue;
        }

        let h = handle.clone();
        let pid = pane_id.clone();
        let active = current_filters.contains(&cat);
        let c_cat = cat;

        items.push(DropdownItem {
            label: cat.label().to_string(),
            icon: Some(if active {
                IconName::Check
            } else {
                IconName::Square
            }),
            stay_open: true,
            has_submenu: false,
            is_dimmed: false,
            on_click: Rc::new(move |_, cx| {
                let pid_inner = pid.clone();
                h.update(cx, |this, cx| {
                    this.update_filter_size(&pid_inner, c_cat, !active, cx);
                    this.open_size_filter_menu(&pid_inner, pos, cx);
                });
            }),
            on_hover: None,
        });
    }

    items
}
