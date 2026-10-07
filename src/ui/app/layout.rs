use crate::ui::scale::{logical};
use crate::ui::app::{DragTarget, FileDragData, OxydeApp};
use crate::ui::i18n::{FluentArgs, I18nContext, Message};
pub use crate::ui::theme::ThemeContext;
pub use gpui::prelude::*;
use gpui::*;

impl OxydeApp {
    pub(crate) fn measure_metadata_columns(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let i18n = cx.i18n();
        let language = i18n.language();
        let reference = crate::ui::scale::normalized_reference(self.config.ui_rem_size);
        let date_format = crate::ui::format::date_format(cx);
        // Every format has the same number of tabular digits. One sample suffices.
        let date_sample = crate::ui::format::format_date(1_760_000_000_000, date_format);
        let measure = |text: &str, bold: bool| {
            let mut font = window.text_style().font();
            font.family = "Inter".into();
            font.weight = if bold { FontWeight::BOLD } else { crate::ui::style::DEFAULT_FONT_WEIGHT };
            font.features = crate::ui::style::TNUM.clone();
            let line = window.text_system().shape_line(text.to_string().into(), crate::ui::scale::rem_pixels(rems(if bold { 0.8125 } else { 0.75 })), &[TextRun {
                len: text.len(), font, color: cx.theme().text, background_color: None, underline: None, strikethrough: None,
            }], None);
            line.width.as_f32() / logical(1.0)
        };
        self.state.layout.visit_panes_mut(&mut |pane| {
            let tab = pane.active_tab_mut();
            let key = (reference, language, date_format, !tab.filter_types.is_empty(), !tab.filter_dates.is_empty(), !tab.filter_sizes.is_empty());
            if tab.column_measurement_key == Some(key) { return; }
            let type_width = tab.column_type_labels.iter().map(|label| {
                let message = match label.as_str() {
                    "Dossier" => Some(Message::FileTypeFolder), "Fichier" => Some(Message::FileTypeFile),
                    "Symlink" => Some(Message::FileTypeSymlink), "Junction" => Some(Message::FileTypeJunction),
                    "Réseau" => Some(Message::SidebarNetwork), _ => None,
                };
                let text = message.map(|message| i18n.text(message).to_string()).unwrap_or_else(|| label.clone());
                measure(&text, false)
            }).fold(0.0_f32, f32::max);
            tab.type_col_width = crate::ui::style::metadata_column_width(type_width, measure(i18n.text(Message::ColumnType).as_ref(), true), key.3);
            tab.date_col_width = crate::ui::style::metadata_column_width(measure(&date_sample, false), measure(i18n.text(Message::ColumnModified).as_ref(), true), key.4);
            tab.deleted_col_width = crate::ui::style::metadata_column_width(measure(&date_sample, false), measure(i18n.text(Message::TrashDeletedAt).as_ref(), false), false);
            let size_width = tab.column_size_values.iter().map(|&(size, directory, calculated)| {
                if directory && calculated && size == 0 { measure(i18n.text(Message::FolderSizeEmpty).as_ref(), false) } else { measure(&crate::ui::format::format_size(size, directory, calculated, language), false) }
            }).fold(0.0_f32, f32::max);
            tab.size_col_width = crate::ui::style::metadata_column_width(size_width, measure(i18n.text(Message::ColumnSize).as_ref(), true), key.5);
            tab.column_measurement_key = Some(key);
        });
    }

    pub fn clamp_scrolls(&mut self) {
        // Clamp Sidebar horizontal scroll
        let sidebar_viewport_w = self.sidebar_display_width();
        let sidebar_content_w = self.sidebar_content_width;
        let sidebar_max_offset_x = (sidebar_content_w - sidebar_viewport_w).max(0.0);
        let current_sidebar_h_offset = self.sidebar_scroll_h.offset().x.as_f32().abs();

        if current_sidebar_h_offset > sidebar_max_offset_x {
            let new_x = (-sidebar_max_offset_x).min(0.0);
            self.sidebar_scroll_h
                .set_offset(Point::new(px(new_x), self.sidebar_scroll_h.offset().y));
        }

        // Clamp FileList horizontal scrolls for all panes
        let p_sizes = &self.pane_sizes;
        let l_w = self.list_width;
        let l_h = self.list_height;

        self.state.layout.visit_panes_mut(&mut |pane| {
            let pane_id = pane.id.clone();
            let active_tab = pane.active_tab_mut();
            let pane_size = p_sizes.get(&pane_id).cloned().unwrap_or(Size {
                width: l_w,
                height: l_h,
            });

            let columns = crate::ui::style::DetailsColumnWidths::for_viewport_with_search(
                pane_size.width - if active_tab.path == "trash://" { logical(active_tab.deleted_col_width) } else { 0.0 },
                active_tab.type_col_width,
                active_tab.size_col_width,
                active_tab.date_col_width,
                active_tab.quick_search.is_some() || active_tab.path == "trash://",
            );
            let total_w = columns.content_width() + if active_tab.path == "trash://" { logical(active_tab.deleted_col_width) } else { 0.0 };

            let max_h_offset = (total_w - pane_size.width).max(0.0);
            let current_h_offset = active_tab.list_scroll_h.offset().x.as_f32().abs();

            if current_h_offset > max_h_offset {
                let new_x = (-max_h_offset).min(0.0);
                active_tab
                    .list_scroll_h
                    .set_offset(Point::new(px(new_x), active_tab.list_scroll_h.offset().y));
            }
        });

        // Clamp Breadcrumb scroll
        if let Some((_, _, _, dirs)) = &self.breadcrumb_menu {
            let item_count = dirs.len();
            let content_h = item_count as f32 * logical(28.0);
            let viewport_h = content_h.min(logical(360.0));
            let max_v_offset = (content_h - viewport_h).max(0.0);
            let current_v_offset = self
                .breadcrumb_scroll
                .0
                .borrow()
                .base_handle
                .offset()
                .y
                .as_f32()
                .abs();
            if current_v_offset > max_v_offset {
                self.breadcrumb_scroll
                    .0
                    .borrow()
                    .base_handle
                    .set_offset(Point::new(px(0.0), px(-max_v_offset)));
            }
        } else {
            self.breadcrumb_scroll
                .0
                .borrow()
                .base_handle
                .set_offset(Point::new(px(0.0), px(0.0)));
        }

        // Clamp ContextMenu scrolls
        for instance in &self.context_menus {
            let item_count = match &instance.content {
                crate::ui::app::ContextMenuContent::Items(items, _) => items.iter().filter(|item| !matches!(item.icon, Some(crate::ui::components::dropdown::IconName::Separator))).count(),
                crate::ui::app::ContextMenuContent::NativeItems { items, .. } => items.len(),
                crate::ui::app::ContextMenuContent::NameFilter { .. } => 0,
            };
            if item_count > 0 {
                let content_h = item_count as f32 * logical(28.0);
                let viewport_h = content_h.min(logical(480.0));
                let max_v_offset = (content_h - viewport_h).max(0.0);
                let current_v_offset = instance
                    .scroll_handle
                    .0
                    .borrow()
                    .base_handle
                    .offset()
                    .y
                    .as_f32()
                    .abs();
                if current_v_offset > max_v_offset {
                    instance
                        .scroll_handle
                        .0
                        .borrow()
                        .base_handle
                        .set_offset(Point::new(px(0.0), px(-max_v_offset)));
                }
            } else {
                instance
                    .scroll_handle
                    .0
                    .borrow()
                    .base_handle
                    .set_offset(Point::new(px(0.0), px(0.0)));
            }
        }
    }

    pub(crate) fn render_layout_node(
        &self,
        node: &crate::ui::workspace::LayoutNode,
        handle: Entity<Self>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        match node {
            crate::ui::workspace::LayoutNode::Pane(pane) => {
                let p_id = pane.id.clone();
                div()
                    .flex_1()
                    .min_w_0()
                    .size_full()
                    .flex()
                    .relative()
                    .child(
                        canvas(|bounds, _window, _cx| bounds, {
                            let h = handle.clone();
                            let p_id = p_id.clone();
                            move |_, bounds: Bounds<Pixels>, _, cx| {
                                let size = Size {
                                    width: bounds.size.width.as_f32(),
                                    height: bounds.size.height.as_f32(),
                                };
                                let origin = Point {
                                    x: bounds.origin.x.as_f32(),
                                    y: bounds.origin.y.as_f32(),
                                };
                                h.update(cx, |this, cx| {
                                    let width_changed = this
                                        .pane_sizes
                                        .get(&p_id)
                                        .is_none_or(|old| (old.width - size.width).abs() > 1.0);
                                    this.pane_sizes.insert(p_id.clone(), size);
                                    this.pane_origins.insert(p_id.clone(), origin);
                                    if width_changed {
                                        if let Some(pane) = this.state.layout.find_pane(&p_id) {
                                            let parts = pane
                                                .active_tab()
                                                .path
                                                .split(['/', '\\'])
                                                .filter(|part| !part.is_empty())
                                                .count();
                                            if parts > 0 {
                                                pane.active_tab()
                                                    .breadcrumb_scroll
                                                    .scroll_to_item(parts * 2 - 2);
                                            }
                                        }
                                        cx.notify();
                                    }
                                });
                            }
                        })
                        .absolute()
                        .size_full(),
                    )
                    .child(self.render_pane(pane, handle, cx))
                    .into_any_element()
            }
            crate::ui::workspace::LayoutNode::Split {
                id,
                axis,
                children,
                weights,
            } => {
                let total_weight: f32 = weights.iter().sum();
                let mut container = div().flex_1().min_w_0().size_full().flex().child(
                    canvas(|bounds, _window, _cx| bounds, {
                        let h = handle.clone();
                        let s_id = id.clone();
                        let axis_copy = axis.clone();
                        move |_, bounds: Bounds<Pixels>, _window, cx| {
                            let size = match axis_copy {
                                crate::ui::workspace::LayoutAxis::Horizontal => {
                                    bounds.size.width.as_f32()
                                }
                                crate::ui::workspace::LayoutAxis::Vertical => {
                                    bounds.size.height.as_f32()
                                }
                            };
                            h.update(cx, |this, _| {
                                this.split_sizes.insert(s_id.clone(), size);
                            });
                        }
                    })
                    .absolute()
                    .size_full(),
                );

                match axis {
                    crate::ui::workspace::LayoutAxis::Horizontal => {
                        container = container.flex_row();
                    }
                    crate::ui::workspace::LayoutAxis::Vertical => {
                        container = container.flex_col();
                    }
                }

                for (i, child) in children.iter().enumerate() {
                    if i > 0 {
                        let separator = match axis {
                            crate::ui::workspace::LayoutAxis::Horizontal => {
                                div()
                                    .group("pane-resize-handle")
                                    .w(rems(0.25))
                                    .mx(rems(-0.125))
                                    .flex_none()
                                    .cursor(CursorStyle::ResizeLeftRight)
                                    .h_full()
                                    .flex()
                                    .justify_center()
                                    .occlude()
                                    .child(
                                        div()
                                            .w(rems(0.0625))
                                            .h_full()
                                            .bg(cx.theme().border)
                                            .group_hover("pane-resize-handle", |s| s.bg(cx.theme().accent.opacity(0.6))),
                                    )
                                    .on_mouse_down(MouseButton::Left, {
                                        let h = handle.clone();
                                        let s_id = id.clone();
                                        let w_copy = weights.clone();
                                        let idx = i - 1;
                                        move |e, _, cx| {
                                            cx.stop_propagation();
                                            h.update(cx, |this, _| {
                                                this.drag_target = Some(DragTarget::PaneResize {
                                                    split_id: s_id.clone(),
                                                    child_idx: idx,
                                                    initial_weights: w_copy.clone(),
                                                });
                                                this.grab_offset = e.position.x.as_f32();
                                            });
                                        }
                                    })
                            }
                            crate::ui::workspace::LayoutAxis::Vertical => div()
                                .group("pane-resize-handle")
                                .h(rems(0.25))
                                .my(rems(-0.125))
                                .occlude()
                                .flex_none()
                                .cursor(CursorStyle::ResizeUpDown)
                                .w_full()
                                .flex()
                                .items_center()
                                .child(
                                    div()
                                        .h(rems(0.0625))
                                        .w_full()
                                        .bg(cx.theme().border)
                                        .group_hover("pane-resize-handle", |s| s.bg(cx.theme().accent.opacity(0.6))),
                                )
                                .on_mouse_down(MouseButton::Left, {
                                    let h = handle.clone();
                                    let s_id = id.clone();
                                    let w_copy = weights.clone();
                                    let idx = i - 1;
                                    move |e, _, cx| {
                                        cx.stop_propagation();
                                        h.update(cx, |this, _| {
                                            this.drag_target = Some(DragTarget::PaneResize {
                                                split_id: s_id.clone(),
                                                child_idx: idx,
                                                initial_weights: w_copy.clone(),
                                            });
                                            this.grab_offset = e.position.y.as_f32();
                                        });
                                    }
                                }),
                        };
                        container = container.child(deferred(separator));
                    }
                    container = container.child(
                        div()
                            .flex_basis(relative(weights[i] / total_weight.max(0.1)))
                            .flex_grow(1.0)
                            .flex_shrink(1.0)
                            .min_w_0()
                            .size_full()
                            .overflow_hidden()
                            .child(self.render_layout_node(child, handle.clone(), cx)),
                    );
                }
                container.into_any_element()
            }
        }
    }

    pub(crate) fn render_pane(
        &self,
        pane: &crate::ui::workspace::PaneState,
        handle: Entity<Self>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        use crate::ui::components::file_list::render_file_list;
        use crate::ui::components::tabs::{Tab, TabBar, TabDragPreview, TabPosition};
        use crate::ui::components::top_bar::render_top_bar;

        let h = handle.clone();
        let p_id = pane.id.clone();
        let theme = *cx.theme();

        div()
            .id(SharedString::from(format!(
                "pane-container-{}",
                p_id.clone()
            )))
            .relative()
            .flex_1()
            .min_w_0()
            .h_full()
            .flex()
            .flex_col()
            .child({
                let active_idx = pane.active_tab_index;
                let tabs_count = pane.tabs.len();
                let mut pane_count = 0;
                self.state.layout.visit_panes(&mut |_| pane_count += 1);
                let can_close_tab = pane_count > 1 || tabs_count > 1;
                let can_close_other_tabs = tabs_count > 1;
                let mut tab_bar = TabBar::new(SharedString::from(format!("tab-bar-{}", p_id)))
                    .scroll_state(pane.tab_strip.clone(), pane.active_tab().id.clone(), active_idx)
                    .on_new_tab({
                        let h = h.clone();
                        let p_id_cap = p_id.clone();
                        move |_, cx| {
                            h.update(cx, |this, cx| {
                                this.add_tab(&p_id_cap, "C:/".to_string(), true, cx);
                            });
                        }
                    });

                for (t_ix, tab_data) in pane.tabs.iter().enumerate() {
                    let is_tab_active = t_ix == active_idx;
                    let h = h.clone();
                    let p_id_cap = p_id.clone();

                    let is_root =
                        tab_data.path.len() <= 3 && tab_data.path.chars().nth(1) == Some(':');
                    let label = if let Some(label) = crate::ui::app::search::search_label(&tab_data.path, cx.i18n()) {
                        label
                    } else if tab_data.path == "__network_vincinity__" {
                        cx.i18n().text(Message::SidebarNetwork).to_string()
                    } else if tab_data.path == "trash://" {
                        cx.i18n().text(Message::SidebarTrash).to_string()
                    } else if is_root {
                        let mut drive_name = None;
                        for item in &self.state.sidebar_items {
                            if let crate::ui::workspace::SidebarItem::Drive { drive, .. } = item {
                                let norm_drive = drive.path.trim_end_matches(['/', '\\']);
                                let norm_tab = tab_data.path.trim_end_matches(['/', '\\']);
                                if norm_drive.eq_ignore_ascii_case(norm_tab) {
                                    drive_name = Some(if drive.label.is_empty() {
                                        {
                                            let mut args = FluentArgs::new();
                                            args.set("drive", norm_drive);
                                            cx.i18n()
                                                .text_with_args(Message::LocalDrive, &args)
                                                .to_string()
                                        }
                                    } else {
                                        format!("{} ({})", drive.label, norm_drive)
                                    });
                                    break;
                                }
                            }
                        }
                        drive_name.unwrap_or_else(|| {
                            tab_data.path.trim_end_matches(['/', '\\']).to_string()
                        })
                    } else {
                        tab_data
                            .path
                            .split(['/', '\\'])
                            .rfind(|p| !p.is_empty())
                            .unwrap_or(cx.i18n().text(Message::UnknownItem).as_ref())
                            .to_string()
                    };

                    let position = if t_ix == 0 {
                        TabPosition::First
                    } else if t_ix == tabs_count - 1 {
                        TabPosition::Last
                    } else {
                        TabPosition::Middle(t_ix.cmp(&active_idx))
                    };

                    let label_cap = label.clone();
                    let tab_path = tab_data.path.clone();
                    let is_dragging = self.dragging_tab == Some((p_id.clone(), t_ix));
                    let context_anchor_key = format!("tab-{}-{}", p_id, t_ix);

                    use crate::ui::components::tabs::TabIcon;
                    let tab_icon = match tab_data.path.as_str() {
                        "__network_vincinity__" => TabIcon::Svg("icons/globe.svg"),
                        "trash://" => TabIcon::Svg("icons/trash-sidebar.svg"),
                        _ => {
                            if let Some(drive) = self.state.drives.iter().find(|drive| {
                                crate::ui::components::sidebar::sidebar_paths_equal(&drive.path, &tab_data.path)
                            }) {
                                TabIcon::Svg(crate::ui::components::icons::drive_icon_path(drive))
                            } else {
                                TabIcon::Item {
                                    path: tab_data.path.clone(),
                                    icon: crate::ui::components::icons::get_icon_type(&label, true, Some(&tab_data.path), false, false),
                                    system: cfg!(target_os = "windows") && self.use_system_icons(),
                                }
                            }
                        }
                    };
                    let tab = Tab::new(
                        SharedString::from(format!("tab-{}-{}", p_id, t_ix)),
                        label.clone(),
                    )
                    .selected(is_tab_active)
                    .position(position)
                    .icon(tab_icon.clone())
                    .on_click({
                        let h = h.clone();
                        let p_id_cap = p_id.clone();
                        move |_, cx| {
                            h.update(cx, |this, cx| this.switch_tab(&p_id_cap, t_ix, cx));
                        }
                    })
                    .on_bounds({
                        let h = h.clone();
                        let anchor_key = context_anchor_key.clone();
                        move |bounds, cx| {
                            let anchor = point(bounds.left(), bounds.bottom());
                            h.update(cx, |this, _| {
                                this.context_anchor_points
                                    .insert(anchor_key.clone(), anchor);
                            });
                        }
                    })
                    .on_right_click({
                        let h = h.clone();
                        let p_id_cap = p_id.clone();
                        let anchor_key = context_anchor_key.clone();
                        move |_, cx, pos| {
                            h.update(cx, |this, cx| {
                                use crate::ui::app::{ContextMenuContent, ContextMenuInstance};
                                let position = this
                                    .context_anchor_points
                                    .get(&anchor_key)
                                    .copied()
                                    .unwrap_or(pos);
                                this.context_menus.clear();
                                this.context_menus.push(ContextMenuInstance {
                                    position,
                                    content: ContextMenuContent::Items(
                                        crate::ui::components::tab_context_menu_items(
                                            cx.i18n(),
                                            h.clone(),
                                            p_id_cap.clone(),
                                            t_ix,
                                            can_close_tab,
                                            can_close_other_tabs,
                                        ),
                                        None,
                                    ),
                                    scroll_handle: gpui::UniformListScrollHandle::new(),
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
                    .when(can_close_tab, |tab| {
                        tab.on_middle_click({
                            let h = h.clone();
                            let p_id_cap = p_id.clone();
                            move |_, cx| {
                                h.update(cx, |this, cx| this.close_tab(&p_id_cap, t_ix, cx));
                            }
                        })
                    })
                    .when(can_close_tab, |s| {
                        let h = h.clone();
                        let p_id_cap = p_id.clone();
                        s.on_close(move |_, cx| {
                            h.update(cx, |this, cx| this.close_tab(&p_id_cap, t_ix, cx));
                        })
                    })
                    .on_drag((p_id.clone(), t_ix), {
                        let lbl = label_cap.clone();
                        let h = h.clone();
                        let p_id_cap = p_id.clone();
                        move |_window, cx| {
                            h.update(cx, |this, _| {
                                this.dragging_tab = Some((p_id_cap.clone(), t_ix));
                            });
                            cx.new(|_| TabDragPreview { label: lbl.clone(), icon: tab_icon.clone() })
                        }
                    })
                    .with_dragging(is_dragging);

                    tab_bar = tab_bar.child(
                        div()
                            .relative()
                            .child(tab)
                            // File Drop Zone (drop files onto tab to move them to that tab's directory)
                            .child(
                                div()
                                    .absolute()
                                    .inset_0()
                                    .drag_over::<FileDragData>({
                                        let h_over = h.clone();
                                        let tgt = tab_path.clone();
                                        move |style, data, _, cx| {
                                            let default_copy = data
                                                .paths
                                                .first()
                                                .map(|s| crate::ui::app::drives_differ(s, &tgt))
                                                .unwrap_or(false);
                                            h_over.update(cx, |this, cx| {
                                                if this.drag_target_default_copy != default_copy {
                                                    this.drag_target_default_copy = default_copy;
                                                    this.update_drag_copy_mode(cx);
                                                }
                                            });
                                            style.bg(theme.selection_hover)
                                        }
                                    })
                                    .on_drop({
                                        let h = h.clone();
                                        let target = tab_path.clone();
                                        move |data: &FileDragData, _, cx| {
                                            let paths = data.paths.clone();
                                            h.update(cx, |this, cx| {
                                                if this.drag_copy_mode {
                                                    this.execute_file_copy(
                                                        paths,
                                                        target.clone(),
                                                        cx,
                                                    );
                                                } else {
                                                    this.execute_file_move(
                                                        paths,
                                                        target.clone(),
                                                        cx,
                                                    );
                                                }
                                            });
                                        }
                                    }),
                            )
                            // Left Drop Zone
                            .child(
                                div()
                                    .absolute()
                                    .top_0()
                                    .left_0()
                                    .h_full()
                                    .w_1_2()
                                    .drag_over::<(String, usize)>({
                                        let h = h.clone();
                                        let p_id_cap = p_id.clone();
                                        move |style, (f_p, f_t), _, app| {
                                            let is_noop = f_p == &p_id_cap
                                                && (*f_t == t_ix || *f_t + 1 == t_ix);
                                            h.update(app, |v, _| {
                                                if is_noop {
                                                    v.drop_target_idx = None;
                                                    v.drop_target_pane = None;
                                                } else {
                                                    v.drop_target_idx = Some(t_ix);
                                                    v.drop_target_pane = Some(p_id_cap.clone());
                                                }
                                                // Clear split highlights when over a tab
                                                v.highlight_split_pane = None;
                                            });
                                            style
                                        }
                                    })
                                    .on_drop({
                                        let h = h.clone();
                                        let p_id_cap = p_id.clone();
                                        move |(from_p, from_t): &(String, usize), _, app| {
                                            let (f_p, f_t) = (from_p.clone(), *from_t);
                                            h.update(app, |this, cx| {
                                                if f_p == p_id_cap {
                                                    this.state
                                                        .reorder_tabs_id(&p_id_cap, f_t, t_ix);
                                                } else {
                                                    this.state
                                                        .move_tab_to_pane_id(&f_p, f_t, &p_id_cap);
                                                    if let Some(pane) =
                                                        this.state.layout.find_pane(&p_id_cap)
                                                    {
                                                        this.state.reorder_tabs_id(
                                                            &p_id_cap,
                                                            pane.tabs.len() - 1,
                                                            t_ix,
                                                        );
                                                    }
                                                }
                                                this.drop_target_idx = None;
                                                this.drop_target_pane = None;
                                                this.dragging_tab = None;
                                                this.reconcile_watchers(cx);
                                                this.save_workspace(cx);
                                                cx.notify();
                                            });
                                        }
                                    }),
                            )
                            // Right Drop Zone
                            .child(
                                div()
                                    .absolute()
                                    .top_0()
                                    .right_0()
                                    .h_full()
                                    .w_1_2()
                                    .drag_over::<(String, usize)>({
                                        let h = h.clone();
                                        let p_id_cap = p_id.clone();
                                        move |style, (f_p, f_t), _, app| {
                                            // For Right Zone at index t_ix. Target is t_ix + 1.
                                            let target = t_ix + 1;
                                            let is_noop = f_p == &p_id_cap
                                                && (*f_t == target || *f_t + 1 == target);
                                            h.update(app, |v, _| {
                                                if is_noop {
                                                    v.drop_target_idx = None;
                                                    v.drop_target_pane = None;
                                                } else {
                                                    v.drop_target_idx = Some(target);
                                                    v.drop_target_pane = Some(p_id_cap.clone());
                                                }
                                                // Clear split highlights when over a tab
                                                v.highlight_split_pane = None;
                                            });
                                            style
                                        }
                                    })
                                    .on_drop({
                                        let h = h.clone();
                                        let p_id_cap = p_id.clone();
                                        move |(from_p, from_t): &(String, usize), _, app| {
                                            let (f_p, f_t) = (from_p.clone(), *from_t);
                                            h.update(app, |this, cx| {
                                                if f_p == p_id_cap {
                                                    this.state.reorder_tabs_id(
                                                        &p_id_cap,
                                                        f_t,
                                                        t_ix + 1,
                                                    );
                                                } else {
                                                    this.state
                                                        .move_tab_to_pane_id(&f_p, f_t, &p_id_cap);
                                                    if let Some(pane) =
                                                        this.state.layout.find_pane(&p_id_cap)
                                                    {
                                                        this.state.reorder_tabs_id(
                                                            &p_id_cap,
                                                            pane.tabs.len() - 1,
                                                            t_ix + 1,
                                                        );
                                                    }
                                                }
                                                this.drop_target_idx = None;
                                                this.drop_target_pane = None;
                                                this.dragging_tab = None;
                                                this.reconcile_watchers(cx);
                                                this.save_workspace(cx);
                                                cx.notify();
                                            });
                                        }
                                    }),
                            )
                            // Visual Drop Indicator (Marker - Left side of this tab)
                            .child(
                                div()
                                    .absolute()
                                    .top_0()
                                    .left_0()
                                    .w(rems(0.125))
                                    .h_full()
                                    .bg(theme.accent)
                                    .when(
                                        self.drop_target_pane.as_ref() == Some(&p_id_cap)
                                            && self.drop_target_idx == Some(t_ix),
                                        |s| s.opacity(1.0),
                                    )
                                    .when(
                                        !(self.drop_target_pane.as_ref() == Some(&p_id_cap)
                                            && self.drop_target_idx == Some(t_ix)),
                                        |s| s.opacity(0.0),
                                    ),
                            )
                            // Visual Drop Indicator (Marker - Right side of this tab, only for the last one)
                            .when(t_ix == tabs_count - 1, |s| {
                                s.child(
                                    div()
                                        .absolute()
                                        .top_0()
                                        .right_0()
                                        .w(rems(0.125))
                                        .h_full()
                                        .bg(theme.accent)
                                        .when(
                                            self.drop_target_pane.as_ref() == Some(&p_id_cap)
                                                && self.drop_target_idx == Some(tabs_count),
                                            |s| s.opacity(1.0),
                                        )
                                        .when(
                                            !(self.drop_target_pane.as_ref() == Some(&p_id_cap)
                                                && self.drop_target_idx == Some(tabs_count)),
                                            |s| s.opacity(0.0),
                                        ),
                                )
                            }),
                    );
                }

                div()
                    .h(crate::ui::style::TAB_BAR_HEIGHT)
                    .w_full()
                    .relative()
                    .drag_over::<(String, usize)>({
                        let h = h.clone();
                        let p_id_cap = p_id.clone();
                        move |style, (f_p, f_t), _, app| {
                            h.update(app, |v, _| {
                                // If no specific tab or gap is already showing an indicator...
                                if v.drop_target_pane.as_ref() != Some(&p_id_cap) {
                                    if let Some(pane) = v.state.layout.find_pane(&p_id_cap) {
                                        let target = pane.tabs.len();
                                        let is_noop =
                                            f_p == &p_id_cap && *f_t == target.saturating_sub(1);
                                        if !is_noop {
                                            v.drop_target_idx = Some(target);
                                            v.drop_target_pane = Some(p_id_cap.clone());
                                        }
                                    }
                                }
                                // Clear split highlights when over the tab bar wrapper
                                v.highlight_split_pane = None;
                            });
                            style
                        }
                    })
                    .on_drop({
                        let h = h.clone();
                        let p_id_cap = p_id.clone();
                        move |(from_p, f_t): &(String, usize), _, app| {
                            let (f_p, f_t) = (from_p.clone(), *f_t);
                            h.update(app, |this, cx| {
                                // If we drop on the wrapper and it wasn't caught by a specific tab/gap
                                // then target the absolute end of the pane
                                if let Some(pane) = this.state.layout.find_pane(&p_id_cap) {
                                    let target = pane.tabs.len();
                                    if f_p == p_id_cap {
                                        this.state.reorder_tabs_id(&p_id_cap, f_t, target);
                                    } else {
                                        this.state.move_tab_to_pane_id(&f_p, f_t, &p_id_cap);
                                    }
                                }
                                this.drop_target_idx = None;
                                this.drop_target_pane = None;
                                this.dragging_tab = None;
                                this.reconcile_watchers(cx);
                                this.save_workspace(cx);
                                cx.notify();
                            });
                        }
                    })
                    .child(tab_bar)
            })
            .child({
                div()
                    .id(SharedString::from(format!("pane-content-{}", p_id.clone())))
                    .size_full()
                    .flex()
                    .flex_col()
                    .relative()
                    .child(render_top_bar(self, p_id.clone(), handle.clone(), cx))
                    .when(self.state.find_pane(&p_id).is_some_and(|pane| pane.active_tab().quick_search.is_some()), |pane| pane.child(self.render_quick_search_banner(&p_id, cx)))
                    .child(render_file_list(self, p_id.clone(), handle.clone(), cx))
                    // Visual highlights for splits
                    .when(self.highlight_split_pane == Some(p_id.clone()), |s| {
                        s.child(
                            div()
                                .absolute()
                                .size_full()
                                .when(self.highlight_left_zone, |s| {
                                    s.child(
                                        div()
                                            .absolute()
                                            .left_0()
                                            .h_full()
                                            .w_1_2()
                                            .bg(theme.selected)
                                            .border_r(rems(0.0625))
                                            .border_color(theme.accent),
                                    )
                                })
                                .when(self.highlight_right_zone, |s| {
                                    s.child(
                                        div()
                                            .absolute()
                                            .right_0()
                                            .h_full()
                                            .w_1_2()
                                            .bg(theme.selected)
                                            .border_l(rems(0.0625))
                                            .border_color(theme.accent),
                                    )
                                })
                                .when(self.highlight_top_zone, |s| {
                                    s.child(
                                        div()
                                            .absolute()
                                            .top_0()
                                            .w_full()
                                            .h_1_2()
                                            .bg(theme.selected)
                                            .border_b(rems(0.0625))
                                            .border_color(theme.accent),
                                    )
                                })
                                .when(self.highlight_bottom_zone, |s| {
                                    s.child(
                                        div()
                                            .absolute()
                                            .bottom_0()
                                            .w_full()
                                            .h_1_2()
                                            .bg(theme.selected)
                                            .border_t(rems(0.0625))
                                            .border_color(theme.accent),
                                    )
                                }),
                        )
                    })
                    // Split detection zones
                    .child(
                        div()
                            .absolute()
                            .left_0()
                            .h_full()
                            .w_1_4()
                            .drag_over::<(String, usize)>({
                                let h = h.clone();
                                let p_id_cap = p_id.clone();
                                move |style, _, _, app| {
                                    h.update(app, |v, _| {
                                        v.highlight_split_pane = Some(p_id_cap.clone());
                                        v.highlight_left_zone = true;
                                        v.highlight_right_zone = false;
                                        v.highlight_top_zone = false;
                                        v.highlight_bottom_zone = false;

                                        // Reset tab targeting when entering a split zone
                                        v.drop_target_idx = None;
                                        v.drop_target_pane = None;
                                    });
                                    style
                                }
                            })
                            .on_drop({
                                let h = h.clone();
                                let p_id_cap = p_id.clone();
                                move |(from_p, f_t): &(String, usize), _, app| {
                                    let (f_p, f_t) = (from_p.clone(), *f_t);
                                    h.update(app, |this, cx| {
                                        this.state.split_tab_to_axis(
                                            &f_p,
                                            f_t,
                                            &p_id_cap,
                                            crate::ui::workspace::LayoutAxis::Horizontal,
                                            false,
                                        );
                                        this.highlight_split_pane = None;
                                        this.dragging_tab = None;
                                        this.drop_target_idx = None;
                                        this.drop_target_pane = None;
                                        this.reconcile_watchers(cx);
                                        this.save_workspace(cx);
                                        cx.notify();
                                    });
                                }
                            }),
                    )
                    .child(
                        div()
                            .absolute()
                            .right_0()
                            .h_full()
                            .w_1_4()
                            .drag_over::<(String, usize)>({
                                let h = h.clone();
                                let p_id_cap = p_id.clone();
                                move |style, _, _, app| {
                                    h.update(app, |v, _| {
                                        v.highlight_split_pane = Some(p_id_cap.clone());
                                        v.highlight_right_zone = true;
                                        v.highlight_left_zone = false;
                                        v.highlight_top_zone = false;
                                        v.highlight_bottom_zone = false;

                                        // Reset tab targeting when entering a split zone
                                        v.drop_target_idx = None;
                                        v.drop_target_pane = None;
                                    });
                                    style
                                }
                            })
                            .on_drop({
                                let h = h.clone();
                                let p_id_cap = p_id.clone();
                                move |(from_p, f_t): &(String, usize), _, app| {
                                    let (f_p, f_t) = (from_p.clone(), *f_t);
                                    h.update(app, |this, cx| {
                                        this.state.split_tab_to_axis(
                                            &f_p,
                                            f_t,
                                            &p_id_cap,
                                            crate::ui::workspace::LayoutAxis::Horizontal,
                                            true,
                                        );
                                        this.highlight_split_pane = None;
                                        this.dragging_tab = None;
                                        this.drop_target_idx = None;
                                        this.drop_target_pane = None;
                                        this.reconcile_watchers(cx);
                                        this.save_workspace(cx);
                                        cx.notify();
                                    });
                                }
                            }),
                    )
                    .child(
                        div()
                            .absolute()
                            .top_0()
                            .w_full()
                            .h_1_4()
                            .drag_over::<(String, usize)>({
                                let h = h.clone();
                                let p_id_cap = p_id.clone();
                                move |style, _, _, app| {
                                    h.update(app, |v, _| {
                                        v.highlight_split_pane = Some(p_id_cap.clone());
                                        v.highlight_top_zone = true;
                                        v.highlight_left_zone = false;
                                        v.highlight_right_zone = false;
                                        v.highlight_bottom_zone = false;

                                        // Reset tab targeting when entering a split zone
                                        v.drop_target_idx = None;
                                        v.drop_target_pane = None;
                                    });
                                    style
                                }
                            })
                            .on_drop({
                                let h = h.clone();
                                let p_id_cap = p_id.clone();
                                move |(from_p, f_t): &(String, usize), _, app| {
                                    let (f_p, f_t) = (from_p.clone(), *f_t);
                                    h.update(app, |this, cx| {
                                        this.state.split_tab_to_axis(
                                            &f_p,
                                            f_t,
                                            &p_id_cap,
                                            crate::ui::workspace::LayoutAxis::Vertical,
                                            false,
                                        );
                                        this.highlight_split_pane = None;
                                        this.dragging_tab = None;
                                        this.drop_target_idx = None;
                                        this.drop_target_pane = None;
                                        this.reconcile_watchers(cx);
                                        this.save_workspace(cx);
                                        cx.notify();
                                    });
                                }
                            }),
                    )
                    .child(
                        div()
                            .absolute()
                            .bottom_0()
                            .w_full()
                            .h_1_4()
                            .drag_over::<(String, usize)>({
                                let h = h.clone();
                                let p_id_cap = p_id.clone();
                                move |style, _, _, app| {
                                    h.update(app, |v, _| {
                                        v.highlight_split_pane = Some(p_id_cap.clone());
                                        v.highlight_bottom_zone = true;
                                        v.highlight_left_zone = false;
                                        v.highlight_right_zone = false;
                                        v.highlight_top_zone = false;

                                        // Reset tab targeting when entering a split zone
                                        v.drop_target_idx = None;
                                        v.drop_target_pane = None;
                                    });
                                    style
                                }
                            })
                            .on_drop({
                                let h = h.clone();
                                let p_id_cap = p_id.clone();
                                move |(from_p, f_t): &(String, usize), _, app| {
                                    let (f_p, f_t) = (from_p.clone(), *f_t);
                                    h.update(app, |this, cx| {
                                        this.state.split_tab_to_axis(
                                            &f_p,
                                            f_t,
                                            &p_id_cap,
                                            crate::ui::workspace::LayoutAxis::Vertical,
                                            true,
                                        );
                                        this.highlight_split_pane = None;
                                        this.dragging_tab = None;
                                        this.drop_target_idx = None;
                                        this.drop_target_pane = None;
                                        this.reconcile_watchers(cx);
                                        this.save_workspace(cx);
                                        cx.notify();
                                    });
                                }
                            }),
                    )
            })
            .into_any_element()
    }
}
