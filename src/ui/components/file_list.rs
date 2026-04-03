use gpui::*;
use gpui::prelude::*;
use oxyde_lib::models::FileEntry;
use crate::ui::app::{OxydeApp, DragTarget, ContextMenuContent};
use crate::ui::style::{ROW_HEIGHT, format_size, format_date};
use crate::ui::components::scrollbar::{render_scrollbar, ScrollbarAxis, ZED_TRACK_WIDTH};
use crate::ui::theme::{OxydeTheme, ThemeContext, PixelsExt};
use crate::ui::workspace::{SortColumn, SortDirection};

const TYPE_WIDTH: f32 = 100.0;
const SIZE_WIDTH: f32 = 100.0;
const DATE_WIDTH: f32 = 180.0;

pub fn render_file_list(view: &OxydeApp, pane_id: String, handle: Entity<OxydeApp>, _cx: &mut Context<OxydeApp>) -> impl IntoElement {
    let p_id_cap = pane_id.clone();
    let theme = _cx.theme();
    let pane = view.state.layout.find_pane(&pane_id).expect("pane exists");
    let active_tab = pane.active_tab();
    let files = active_tab.filtered_files().to_vec();
    let count = files.len() as f32;
    let _list_scroll = active_tab.list_scroll.clone();
    
    let pane_size = view.pane_sizes.get(&pane_id).cloned().unwrap_or(Size { width: view.list_width, height: view.list_height });
    
    let list_container_h = (pane_size.height - 64.0).max(1.0);
    let v_v_h = (list_container_h - 28.0).max(1.0);
    let content_h = (count * ROW_HEIGHT).max(1.0);
    
    let pane_viewport_w = pane_size.width.max(1.0);
    let fixed_w = TYPE_WIDTH + SIZE_WIDTH + DATE_WIDTH;
    let name_w = (pane_viewport_w - fixed_w - 28.0).max(80.0);
    let total_w = name_w + fixed_w;


    let is_dragging_v = matches!(view.drag_target, Some(DragTarget::FileListV { ref pane_id, .. }) if pane_id == &p_id_cap);
    let _is_dragging_h = matches!(view.drag_target, Some(DragTarget::FileListH { ref pane_id, .. }) if pane_id == &p_id_cap);

    let selected_paths = active_tab.selected_paths.clone();

    let is_active_pane = view.state.active_pane_id == pane_id;

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
                        let _ = h.update(cx, |this, _| {
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
                        .child(render_headers(view, pane_id.clone(), handle.clone(), name_w, theme))
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

                                            div()
                                                .flex()
                                                .items_center()
                                                .h(px(ROW_HEIGHT))
                                                .w(px(total_w))
                                                .bg(if is_selected { theme.selected } else if is_active_pane { theme.active_background } else { theme.background })
                                                .when(!is_selected, |s| s.hover(|s| s.bg(theme.hover)))
                                                .on_mouse_down(MouseButton::Left, {
                                                    let p = path.clone();
                                                    let h_inner_inner = h.clone();
                                                    let p_id_cap = p_id.clone();
                                                    move |e, _, cx| {
                                                        cx.stop_propagation();
                                                        let multi = e.modifiers.control;
                                                        let shift = e.modifiers.shift;
                                                        let _ = h_inner_inner.update(cx, |this, cx| {
                                                            this.state.active_pane_id = p_id_cap.clone();
                                                            this.toggle_selection(&p_id_cap, p.clone(), multi, shift, cx);
                                                        });
                                                        if e.click_count == 2 && is_dir {
                                                            let _ = h_inner_inner.update(cx, |v, cx| v.navigate_to(&p_id_cap, p.clone(), cx));
                                                        }
                                                    }
                                                })
                                                .on_mouse_down(MouseButton::Middle, {
                                                    let p = path.clone();
                                                    let h_inner_inner = h.clone();
                                                    let p_id_cap = p_id.clone();
                                                    move |_, _, cx| {
                                                        cx.stop_propagation();
                                                        if is_dir {
                                                            let _ = h_inner_inner.update(cx, |v, cx| v.add_tab(&p_id_cap, p.clone(), false, cx));
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
                                                          let _ = h_inner_inner.update(cx, |this, cx| {
                                                              this.state.active_pane_id = p_id_cap_cap.clone();
                                                              if !this.state.find_pane(&p_id_cap_cap).map(|p| p.active_tab().selected_paths.contains(&p_for_right_cap)).unwrap_or(false) {
                                                                  this.toggle_selection(&p_id_cap_cap, p_for_right_cap.clone(), false, false, cx);
                                                              }
                                                              this.context_menu = Some((pos, ContextMenuContent::Items(super::oxyde_context_menu_items(h_inner_inner.clone()), None)));
                                                              this.context_menu_scroll = UniformListScrollHandle::new();
                                                              cx.notify();
                                                          });
                                                     }
                                                 })
                                                .child(
                                                    div()
                                                        .w(px(name_w))
                                                        .px_4()
                                                        .flex()
                                                        .items_center()
                                                        .overflow_hidden()
                                                        .child(svg().path(if is_dir { "icons/folder.svg" } else { "icons/file.svg" }).size_4().flex_none().mr_2().text_color(if is_dir { theme.accent } else { theme.text_muted }))
                                                        .child(div().flex_1().overflow_hidden().text_ellipsis().font_weight(FontWeight::MEDIUM).text_color(theme.text).child(file.name.clone()))
                                                )
                                                .child(div().w(px(TYPE_WIDTH)).px_3().text_color(theme.text_muted).text_ellipsis().text_size(px(12.0)).child(if is_dir { "Dossier".to_string() } else { std::path::Path::new(&file.name).extension().and_then(|e| e.to_str()).map(|s| s.to_uppercase()).unwrap_or("Fichier".to_string()) }))
                                                .child(div().w(px(SIZE_WIDTH)).px_3().flex().justify_end().text_color(theme.text_muted).text_ellipsis().text_size(px(12.0)).child(format_size(file.size, is_dir, file.is_calculated)))
                                                .child(div().w(px(DATE_WIDTH)).px_3().text_color(theme.text_muted).text_ellipsis().text_size(px(12.0)).child(format_date(file.modified)))
                                        }).collect::<Vec<_>>()
                                    }
                                }
                            )
                            .on_mouse_down(MouseButton::Left, {
                                let h = handle.clone();
                                let p_id_cap = pane_id.clone();
                                move |e, _, cx| {
                                    let pos = e.position;
                                    let modifiers = e.modifiers;
                                    let _ = h.update(cx, |this, cx| {
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
                                        })                            .track_scroll(active_tab.list_scroll.clone())
                            .size_full()
                        )
                )
        )
        // 3. Gutters interactifs (au-dessus)
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
                        let pos = e.position;
                        let modifiers = e.modifiers;
                        let _ = h.update(cx, |this, cx| {
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
                            })        )
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
                        let _ = h.update(cx, |this, cx| {
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
                            })        )
    ;

    // Selection Rectangle
    if let Some(DragTarget::SelectionRectangle { pane_id: ref drag_pane_id, start_pos, current_pos, .. }) = view.drag_target {
        if drag_pane_id == &pane_id {
            let list_origin = view.list_bounds.get(&pane_id).map(|b| b.origin).unwrap_or(Point { x: 0.0, y: 0.0 });
            
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
                    .border_color(theme.accent)
            );
        }
    }

    let pane_suffix = format!("pane-{}", pane_id);
    let v_offset = -active_tab.list_scroll.0.borrow().base_handle.offset().y.as_f32();
    
    // Vertical scrollbar overlay
    container = container.child(
        div().absolute().top(px(ROW_HEIGHT)).right_0().bottom_0().w(px(ZED_TRACK_WIDTH))
            .child(render_scrollbar(handle.clone(), ScrollbarAxis::Vertical, v_v_h, content_h, v_offset, is_dragging_v, format!("v-{}", pane_suffix), None, {
                let p_id = p_id_cap.clone();
                move |this, track_id, v_h, c_h, _| {
                    this.drag_target = Some(DragTarget::FileListV { pane_id: p_id.clone(), viewport_h: v_h, content_h: c_h, track_id });
                }
            }))
    );

    // Horizontal scrollbar overlay
    container = container.child(
        div().absolute().left_0().bottom_0().right(px(ZED_TRACK_WIDTH)).h(px(ZED_TRACK_WIDTH))
            .child(render_scrollbar(handle.clone(), ScrollbarAxis::Horizontal, pane_viewport_w, total_w + 28.0, active_tab.list_scroll_h.offset().x.as_f32(), matches!(view.drag_target, Some(DragTarget::FileListH { ref pane_id, .. }) if pane_id == &p_id_cap), format!("h-{}", pane_suffix), None, {
                let p_id = p_id_cap.clone();
                move |this, track_id, v_w, c_w, _| {
                    this.drag_target = Some(DragTarget::FileListH { pane_id: p_id.clone(), viewport_w: v_w, content_w: c_w, track_id });
                }
            }))
    );
    
    container
}

fn render_headers(view: &OxydeApp, pane_id: String, handle: Entity<OxydeApp>, name_w: f32, theme: &OxydeTheme) -> impl IntoElement {
    let pane = view.state.layout.find_pane(&pane_id).expect("pane exists");
    let tab = pane.active_tab();
    let current_col = tab.sort_column;
    let current_dir = tab.sort_direction;

    div().flex().items_center().h(px(ROW_HEIGHT)).text_color(theme.text_muted).font_weight(FontWeight::BOLD).pl(px(8.0)).pr(px(8.0))
        .child(header_cell("Nom", name_w, current_col == SortColumn::Name, current_dir, theme, {
            let h = handle.clone(); let pid = pane_id.clone(); move |_, _, cx| { let _ = h.update(cx, |this, cx| this.toggle_sort(&pid, SortColumn::Name, cx)); }
        })
        .when(!tab.filter_name.is_empty(), |s| s.child(div().ml_1().child(svg().path("icons/filter.svg").size_3().text_color(theme.accent))))
        .on_mouse_down(MouseButton::Right, {
            let h = handle.clone();
            let pid = pane_id.clone();
            move |e, window, cx| {
                let pos = e.position;
                let _ = h.update(cx, |this, cx| this.open_name_filter(&pid, pos, window, cx));
            }
        }))
        .child(header_cell("Type", TYPE_WIDTH, current_col == SortColumn::Type, current_dir, theme, {
            let h = handle.clone(); let pid = pane_id.clone(); move |_, _, cx| { let _ = h.update(cx, |this, cx| this.toggle_sort(&pid, SortColumn::Type, cx)); }
        })
        .when(!tab.filter_types.is_empty(), |s| {
            // Check if every present type is in the filter list
            let mut all_types = std::collections::HashSet::new();
            for f in &tab.files {
                if f.is_dir { all_types.insert("Dossier".to_string()); }
                else if let Some(ext) = std::path::Path::new(&f.name).extension().and_then(|e| e.to_str()) {
                    all_types.insert(ext.to_uppercase());
                }
            }
            let is_full = all_types.iter().all(|t| tab.filter_types.contains(t));
            s.when(!is_full, |s| s.child(div().ml_1().child(svg().path("icons/filter.svg").size_3().text_color(theme.accent))))
        })
        .on_mouse_down(MouseButton::Right, {
             let h = handle.clone();
             let pid = pane_id.clone();
             move |e, _, cx| {
                 let pos = e.position;
                 let _ = h.update(cx, |this, cx| this.open_type_filter_menu(&pid, pos, cx));
             }
        }))
        .child(header_cell("Taille", SIZE_WIDTH, current_col == SortColumn::Size, current_dir, theme, {
            let h = handle.clone(); let pid = pane_id.clone(); move |_, _, cx| { let _ = h.update(cx, |this, cx| this.toggle_sort(&pid, SortColumn::Size, cx)); }
        })
        .when(!tab.filter_sizes.is_empty(), |s| {
            use crate::ui::workspace::SizeCategory;
            let categories = [SizeCategory::Minuscule, SizeCategory::Petit, SizeCategory::Moyen, SizeCategory::Grand, SizeCategory::Huge];
            let mut present = std::collections::HashSet::new();
            for f in &tab.files {
                for cat in &categories {
                    if cat.matches(f.size) { present.insert(*cat); break; }
                }
            }
            let is_full = present.iter().all(|c| tab.filter_sizes.contains(c));
            s.when(!is_full, |s| s.child(div().ml_1().child(svg().path("icons/filter.svg").size_3().text_color(theme.accent))))
        })
        .on_mouse_down(MouseButton::Right, {
             let h = handle.clone();
             let pid = pane_id.clone();
             move |e, _, cx| {
                 let pos = e.position;
                 let _ = h.update(cx, |this, cx| this.open_size_filter_menu(&pid, pos, cx));
             }
        }))
        .child(header_cell("Modifié le", DATE_WIDTH, current_col == SortColumn::Modified, current_dir, theme, {
            let h = handle.clone(); let pid = pane_id.clone(); move |_, _, cx| { let _ = h.update(cx, |this, cx| this.toggle_sort(&pid, SortColumn::Modified, cx)); }
        })
        .when(!tab.filter_dates.is_empty(), |s| {
            use crate::ui::workspace::DateCategory;
            let categories = [DateCategory::Aujourdhui, DateCategory::Hier, DateCategory::CetteSemaine, DateCategory::LaSemaineDerniere, DateCategory::PlusAncien];
            let mut present = std::collections::HashSet::new();
            for f in &tab.files {
                for cat in &categories {
                    if cat.matches(f.modified) { present.insert(*cat); break; }
                }
            }
            let is_full = present.iter().all(|c| tab.filter_dates.contains(c));
            s.when(!is_full, |s| s.child(div().ml_1().child(svg().path("icons/filter.svg").size_3().text_color(theme.accent))))
        })
        .on_mouse_down(MouseButton::Right, {
             let h = handle.clone();
             let pid = pane_id.clone();
             move |e, _, cx| {
                 let pos = e.position;
                 let _ = h.update(cx, |this, cx| this.open_date_filter_menu(&pid, pos, cx));
             }
        }))
}

pub fn get_date_filter_items(handle: Entity<OxydeApp>, pane_id: String, files: Vec<oxyde_lib::models::FileEntry>, current_filters: std::collections::HashSet<crate::ui::workspace::DateCategory>, pos: Point<Pixels>) -> Vec<crate::ui::components::dropdown::DropdownItem> {
    use crate::ui::workspace::DateCategory;
    use crate::ui::components::dropdown::{DropdownItem, IconName};
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
        if !present_categories.contains(&cat) { continue; }
        
        let h = handle.clone();
        let pid = pane_id.clone();
        let active = current_filters.contains(&cat);
        let c_cat = cat;
        
        items.push(DropdownItem {
            label: cat.label().to_string(),
            icon: Some(if active { IconName::Check } else { IconName::Square }),
            stay_open: true,
            on_click: Rc::new(move |_, cx| {
                let pid_inner = pid.clone();
                let _ = h.update(cx, |this, cx| {
                    this.update_filter_date(&pid_inner, c_cat, !active, cx);
                    this.open_date_filter_menu(&pid_inner, pos, cx);
                });
            }),
        });
    }
    
    items
}

pub fn get_size_filter_items(handle: Entity<OxydeApp>, pane_id: String, files: Vec<oxyde_lib::models::FileEntry>, current_filters: std::collections::HashSet<crate::ui::workspace::SizeCategory>, pos: Point<Pixels>) -> Vec<crate::ui::components::dropdown::DropdownItem> {
    use crate::ui::workspace::SizeCategory;
    use crate::ui::components::dropdown::{DropdownItem, IconName};
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
        if !present_categories.contains(&cat) { continue; }
        
        let h = handle.clone();
        let pid = pane_id.clone();
        let active = current_filters.contains(&cat);
        let c_cat = cat;
        
        items.push(DropdownItem {
            label: cat.label().to_string(),
            icon: Some(if active { IconName::Check } else { IconName::Square }),
            stay_open: true,
            on_click: Rc::new(move |_, cx| {
                let pid_inner = pid.clone();
                let _ = h.update(cx, |this, cx| {
                    this.update_filter_size(&pid_inner, c_cat, !active, cx);
                    this.open_size_filter_menu(&pid_inner, pos, cx);
                });
            }),
        });
    }
    
    items
}

fn header_cell(label: &str, width: f32, is_active: bool, direction: SortDirection, theme: &OxydeTheme, on_click: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static) -> Div {
    div()
        .w(px(width))
        .px_4()
        .h_full()
        .flex()
        .items_center()
        .child(div().flex_1().overflow_hidden().text_ellipsis().child(label.to_string()))
        .when(is_active, |s| {
            s.child(
                div()
                    .ml_1()
                    .flex()
                    .items_center()
                    .child(
                        svg()
                            .path(if direction == SortDirection::Ascending { "icons/arrow-up.svg" } else { "icons/arrow-down.svg" })
                            .size_3()
                            .text_color(theme.accent)
                    )
            )
        })
        .hover(|s| s.bg(rgba(0xffffff05)))
        .on_mouse_down(MouseButton::Left, on_click)
}

pub fn get_type_filter_items(h: Entity<OxydeApp>, pid: String, tab_files: Vec<FileEntry>, current_filters: std::collections::HashSet<String>, pos: Point<Pixels>) -> Vec<crate::ui::components::dropdown::DropdownItem> {
    use crate::ui::components::dropdown::{DropdownItem, IconName};
    
    let mut exts = std::collections::HashSet::new();
    let mut dirs_present = false;
    for f in &tab_files {
        if f.is_dir { dirs_present = true; }
        else if let Some(e) = std::path::Path::new(&f.name).extension().and_then(|e| e.to_str()) { exts.insert(e.to_uppercase()); }
    }
    let mut sorted_exts: Vec<String> = exts.into_iter().collect();
    sorted_exts.sort();

    let mut items = Vec::new();
    
    // Dossiers
    if dirs_present {
        let h_dir = h.clone();
        let pid_dir = pid.clone();
        let is_sel = current_filters.contains("Dossier");
        
        items.push(DropdownItem {
            label: "Dossier".to_string(),
            icon: Some(if is_sel { IconName::Check } else { IconName::Square }),
            stay_open: true,
            on_click: std::rc::Rc::new(move |_, app| {
                let pid_cap = pid_dir.clone();
                let _ = h_dir.update(app, |this, cx| {
                    this.toggle_filter_type(&pid_cap, "Dossier".to_string(), cx);
                    this.open_type_filter_menu(&pid_cap, pos, cx);
                });
            })
        });
    }

    for ext in sorted_exts {
        let h_ext = h.clone();
        let pid_ext = pid.clone();
        let e_cap = ext.clone();
        let is_sel = current_filters.contains(&e_cap);

        items.push(DropdownItem {
            label: ext,
            icon: Some(if is_sel { IconName::Check } else { IconName::Square }),
            stay_open: true,
            on_click: std::rc::Rc::new(move |_, app| {
                let pid_cap = pid_ext.clone();
                let e_inner = e_cap.clone();
                let _ = h_ext.update(app, |this, cx| {
                    this.toggle_filter_type(&pid_cap, e_inner, cx);
                    this.open_type_filter_menu(&pid_cap, pos, cx);
                });
            })
        });
    }
    
    items
}
