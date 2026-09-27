use crate::ui::app::{DragTarget, FileDragData, OxydeApp};
pub use crate::ui::theme::ThemeContext;
pub use gpui::prelude::*;
use gpui::*;

impl OxydeApp {
    pub fn clamp_scrolls(&mut self) {
        // Clamp Sidebar horizontal scroll
        let sidebar_viewport_w = self.sidebar_width;
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

            // Constants matching file_list.rs
            let type_w = 100.0;
            let size_w = 100.0;
            let date_w = 180.0;
            let fixed_w = type_w + size_w + date_w;
            let name_w = (pane_size.width - fixed_w).max(80.0);
            let total_w = name_w + fixed_w;

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
            let content_h = item_count as f32 * 28.0;
            let viewport_h = content_h.min(360.0);
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
                crate::ui::app::ContextMenuContent::Items(items, _) => items.len(),
                crate::ui::app::ContextMenuContent::NativeItems { items, .. } => items.len(),
                crate::ui::app::ContextMenuContent::NameFilter { .. } => 0,
            };
            if item_count > 0 {
                let content_h = item_count as f32 * 28.0;
                let viewport_h = content_h.min(480.0);
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
                                h.update(cx, |this, _| {
                                    this.pane_sizes.insert(p_id.clone(), size);
                                    this.pane_origins.insert(p_id.clone(), origin);
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
                let mut container = div().flex_1().size_full().flex().child(
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
                                    .w(px(4.0)) // Wider grab area
                                    .h_full()
                                    .cursor(CursorStyle::ResizeLeftRight)
                                    .child(
                                        div()
                                            .size_full()
                                            .bg(cx.theme().border)
                                            .hover(|s| s.bg(cx.theme().accent)),
                                    )
                                    .on_mouse_down(MouseButton::Left, {
                                        let h = handle.clone();
                                        let s_id = id.clone();
                                        let w_copy = weights.clone();
                                        let idx = i - 1;
                                        move |e, _, cx| {
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
                                .h(px(4.0))
                                .w_full()
                                .cursor(CursorStyle::ResizeUpDown)
                                .child(
                                    div()
                                        .size_full()
                                        .bg(cx.theme().border)
                                        .hover(|s| s.bg(cx.theme().accent)),
                                )
                                .on_mouse_down(MouseButton::Left, {
                                    let h = handle.clone();
                                    let s_id = id.clone();
                                    let w_copy = weights.clone();
                                    let idx = i - 1;
                                    move |e, _, cx| {
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
                        container = container.child(separator);
                    }
                    container = container.child(
                        div()
                            .flex_basis(relative(weights[i] / total_weight.max(0.1)))
                            .flex_grow(1.0)
                            .flex_shrink(1.0)
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

        div()
            .id(SharedString::from(format!(
                "pane-container-{}",
                p_id.clone()
            )))
            .relative()
            .flex_1()
            .h_full()
            .flex()
            .flex_col()
            .child({
                let active_idx = pane.active_tab_index;
                let tabs_count = pane.tabs.len();
                let is_drop_target_at_end = self.drop_target_pane.as_ref() == Some(&p_id)
                    && self.drop_target_idx == Some(tabs_count);

                let mut tab_bar = TabBar::new(SharedString::from(format!("tab-bar-{}", p_id)))
                    .on_new_tab({
                        let h = h.clone();
                        let p_id_cap = p_id.clone();
                        move |_, cx| {
                            h.update(cx, |this, cx| {
                                this.add_tab(&p_id_cap, "C:/".to_string(), true, cx);
                            });
                        }
                    })
                    .drop_target_indicator(is_drop_target_at_end);

                for (t_ix, tab_data) in pane.tabs.iter().enumerate() {
                    let is_tab_active = t_ix == active_idx;
                    let h = h.clone();
                    let p_id_cap = p_id.clone();

                    let is_root =
                        tab_data.path.len() <= 3 && tab_data.path.chars().nth(1) == Some(':');
                    let label = if is_root {
                        let mut drive_name = None;
                        for item in &self.state.sidebar_items {
                            if let crate::ui::workspace::SidebarItem::Drive { drive, .. } = item {
                                let norm_drive = drive.path.trim_end_matches(['/', '\\']);
                                let norm_tab = tab_data.path.trim_end_matches(['/', '\\']);
                                if norm_drive.eq_ignore_ascii_case(norm_tab) {
                                    drive_name = Some(if drive.label.is_empty() {
                                        format!("Disque local ({})", norm_drive)
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
                            .unwrap_or("Unknown")
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

                    let tab = Tab::new(
                        SharedString::from(format!("tab-{}-{}", p_id, t_ix)),
                        label.clone(),
                    )
                    .selected(is_tab_active)
                    .position(position)
                    .icon("icons/folder.svg")
                    .on_click({
                        let h = h.clone();
                        let p_id_cap = p_id.clone();
                        move |_, cx| {
                            h.update(cx, |this, cx| this.switch_tab(&p_id_cap, t_ix, cx));
                        }
                    })
                    .on_right_click({
                        let h = h.clone();
                        let p_id_cap = p_id.clone();
                        move |_, cx, pos| {
                            h.update(cx, |this, cx| {
                                use crate::ui::app::{ContextMenuContent, ContextMenuInstance};
                                this.context_menus.clear();
                                this.context_menus.push(ContextMenuInstance {
                                    position: pos,
                                    content: ContextMenuContent::Items(
                                        crate::ui::components::tab_context_menu_items(
                                            h.clone(),
                                            p_id_cap.clone(),
                                            t_ix,
                                        ),
                                        None,
                                    ),
                                    scroll_handle: gpui::UniformListScrollHandle::new(),
                                    width: px(0.0),
                                    open_left: false,
                                });
                                cx.notify();
                            });
                        }
                    })
                    .on_middle_click({
                        let h = h.clone();
                        let p_id_cap = p_id.clone();
                        move |_, cx| {
                            h.update(cx, |this, cx| this.close_tab(&p_id_cap, t_ix, cx));
                        }
                    })
                    .when(tabs_count > 1, |s| {
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
                            cx.new(|_| TabDragPreview { label: lbl.clone() })
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
                                            style.bg(rgba(0x0078d430))
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
                                    .w(px(2.0))
                                    .h_full()
                                    .bg(rgb(0x0078d4))
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
                                        .w(px(2.0))
                                        .h_full()
                                        .bg(rgb(0x0078d4))
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

                // Drop zone at the end of tabs (Transparent behavior-only div)
                tab_bar = tab_bar.child(
                    div()
                        .w(px(8.0)) // Close to tabs
                        .h_full()
                        .drag_over::<(String, usize)>({
                            let h = h.clone();
                            let p_id_cap = p_id.clone();
                            move |style, (from_p, from_t), _, app| {
                                let target = tabs_count;
                                let is_noop =
                                    from_p == &p_id_cap && *from_t == target.saturating_sub(1);

                                h.update(app, |v, _| {
                                    if !is_noop {
                                        v.drop_target_idx = Some(target);
                                        v.drop_target_pane = Some(p_id_cap.clone());
                                    } else {
                                        v.drop_target_idx = None;
                                        v.drop_target_pane = None;
                                    }
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
                                        this.state.reorder_tabs_id(&p_id_cap, f_t, tabs_count);
                                    } else {
                                        this.state.move_tab_to_pane_id(&f_p, f_t, &p_id_cap);
                                    }
                                    this.drop_target_idx = None;
                                    this.drop_target_pane = None;
                                    this.dragging_tab = None;
                                    cx.notify();
                                });
                            }
                        }),
                );

                div()
                    .h(px(32.0))
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
                                            .bg(rgba(0x0078d422))
                                            .border_r_1()
                                            .border_color(rgb(0x0078d4)),
                                    )
                                })
                                .when(self.highlight_right_zone, |s| {
                                    s.child(
                                        div()
                                            .absolute()
                                            .right_0()
                                            .h_full()
                                            .w_1_2()
                                            .bg(rgba(0x0078d422))
                                            .border_l_1()
                                            .border_color(rgb(0x0078d4)),
                                    )
                                })
                                .when(self.highlight_top_zone, |s| {
                                    s.child(
                                        div()
                                            .absolute()
                                            .top_0()
                                            .w_full()
                                            .h_1_2()
                                            .bg(rgba(0x0078d422))
                                            .border_b_1()
                                            .border_color(rgb(0x0078d4)),
                                    )
                                })
                                .when(self.highlight_bottom_zone, |s| {
                                    s.child(
                                        div()
                                            .absolute()
                                            .bottom_0()
                                            .w_full()
                                            .h_1_2()
                                            .bg(rgba(0x0078d422))
                                            .border_t_1()
                                            .border_color(rgb(0x0078d4)),
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
                                        cx.notify();
                                    });
                                }
                            }),
                    )
            })
            .into_any_element()
    }
}
