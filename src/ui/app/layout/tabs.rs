use crate::ui::app::{FileDragData, OxydeApp};
use crate::ui::elements::motion;
use crate::ui::components::tabs::{Tab, TabBar, TabDragPreview, TabPosition};
use crate::ui::i18n::{FluentArgs, I18nContext, Message};
use super::ThemeContext;
use gpui::prelude::*;
use gpui::*;

fn update_tab_drop_target(
    drop_target_pane: &mut Option<String>,
    drop_target_idx: &mut Option<usize>,
    source_pane: &str,
    source_index: usize,
    target_pane: &str,
    target_index: usize,
) -> bool {
    let is_noop = source_pane == target_pane
        && (source_index == target_index || source_index.checked_add(1) == Some(target_index));
    let next_index = (!is_noop).then_some(target_index);
    let next_pane = (!is_noop).then_some(target_pane);
    let changed = *drop_target_idx != next_index || drop_target_pane.as_deref() != next_pane;
    *drop_target_idx = next_index;
    if drop_target_pane.as_deref() != next_pane {
        *drop_target_pane = next_pane.map(str::to_owned);
    }
    changed
}

fn tab_drop_zone_contains_pointer(
    bounds: Bounds<Pixels>,
    viewport: Bounds<Pixels>,
    position: Point<Pixels>,
) -> bool {
    bounds.contains(&position) && viewport.contains(&position)
}

impl OxydeApp {
    pub(super) fn render_pane_tabs(
        &self,
        pane: &crate::ui::workspace::PaneState,
        handle: Entity<Self>,
        cx: &mut Context<Self>,
    ) -> Div {
        let h = handle.clone();
        let p_id = pane.id.clone();
        let theme = *cx.theme();

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

            let is_root =
                tab_data.path.len() <= 3 && tab_data.path.chars().nth(1) == Some(':');
            let is_virtual = matches!(tab_data.path.as_str(), "__network_vincinity__" | "trash://")
                || crate::ui::app::search::search_label(&tab_data.path, cx.i18n()).is_some();
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
                TabPosition::Middle
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
            // Internal locations show their translated name, never their technical path.
            .when(self.context_menus.is_empty(), |tab| tab.tooltip_path(if is_virtual { label.clone() } else { tab_data.path.clone() }))
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
            native_bounds: None,
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
                    .flex_basis(rems(12.0))
                    .flex_shrink(1.0)
                    .child(tab)
                    // File Drop Zone (drop files onto tab to move them to that tab's directory)
                    .child(
                        div()
                            .absolute()
                            .inset_0()
                            .on_drag_move::<FileDragData>({
                                let h = h.clone();
                                let pane_id = p_id.clone();
                                move |event, _, cx| {
                                    h.update(cx, |this, cx| {
                                        if this.state.find_pane(&pane_id).is_some_and(|pane| tab_drop_zone_contains_pointer(event.bounds, pane.tab_strip.scroll.bounds(), event.event.position)) {
                                            this.set_file_drag_new_tab(false, cx);
                                            if this.state.active_pane_id != pane_id || this.state.find_pane(&pane_id).is_some_and(|pane| pane.active_tab_index != t_ix) {
                                                this.switch_tab(&pane_id, t_ix, cx);
                                            }
                                        }
                                    });
                                }
                            })
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
                                        this.execute_file_drop(paths, target.clone(), cx);
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
                            .on_drag_move::<(String, usize)>({
                                let h = h.clone();
                                let p_id_cap = p_id.clone();
                                move |event, _, app| {
                                    let (source_pane, source_index) = event.drag(app).clone();
                                    h.update(app, |v, cx| {
                                        let Some(pane) = v.state.layout.find_pane(&p_id_cap) else {
                                            return;
                                        };
                                        if !tab_drop_zone_contains_pointer(
                                            event.bounds,
                                            pane.tab_strip.scroll.bounds(),
                                            event.event.position,
                                        ) {
                                            return;
                                        }
                                        let changed = update_tab_drop_target(
                                            &mut v.drop_target_pane,
                                            &mut v.drop_target_idx,
                                            &source_pane,
                                            source_index,
                                            &p_id_cap,
                                            t_ix,
                                        );
                                        let had_split_highlight = v.highlight_split_pane.take().is_some();
                                        if changed || had_split_highlight {
                                            cx.notify();
                                        }
                                    });
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
                            .on_drag_move::<(String, usize)>({
                                let h = h.clone();
                                let p_id_cap = p_id.clone();
                                move |event, _, app| {
                                    let (source_pane, source_index) = event.drag(app).clone();
                                    h.update(app, |v, cx| {
                                        let Some(pane) = v.state.layout.find_pane(&p_id_cap) else {
                                            return;
                                        };
                                        if !tab_drop_zone_contains_pointer(
                                            event.bounds,
                                            pane.tab_strip.scroll.bounds(),
                                            event.event.position,
                                        ) {
                                            return;
                                        }
                                        let changed = update_tab_drop_target(
                                            &mut v.drop_target_pane,
                                            &mut v.drop_target_idx,
                                            &source_pane,
                                            source_index,
                                            &p_id_cap,
                                            t_ix + 1,
                                        );
                                        let had_split_highlight = v.highlight_split_pane.take().is_some();
                                        if changed || had_split_highlight {
                                            cx.notify();
                                        }
                                    });
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
                    // Insertion slots measured for the gliding drop marker.
                    .child(slot_probe(&pane.tab_strip, t_ix, false))
                    .when(t_ix == tabs_count - 1, |s| s.child(slot_probe(&pane.tab_strip, tabs_count, true))),
            );
        }

        div()
            .h(crate::ui::style::TAB_BAR_HEIGHT)
            .w_full()
            .relative()
            .on_drag_move::<FileDragData>({
                let h = h.clone();
                move |event, _, cx| {
                    if event.bounds.contains(&event.event.position) {
                        h.update(cx, |this, cx| {
                            let value = !this.tab_drop_directories(event.drag(cx)).is_empty();
                            this.set_file_drag_new_tab(value, cx);
                        });
                    }
                }
            })
            .on_drag_move::<(String, usize)>({
                let h = h.clone();
                let p_id_cap = p_id.clone();
                move |event, _, app| {
                    if !event.bounds.contains(&event.event.position) {
                        return;
                    }
                    let (source_pane, source_index) = event.drag(app).clone();
                    h.update(app, |v, cx| {
                        if let Some(pane) = v.state.layout.find_pane(&p_id_cap) {
                            // Capture dispatch visits this wrapper before the tab halves.
                            // Free space and the + button therefore fall back to the end.
                            let changed = update_tab_drop_target(
                                &mut v.drop_target_pane,
                                &mut v.drop_target_idx,
                                &source_pane,
                                source_index,
                                &p_id_cap,
                                pane.tabs.len(),
                            );
                            let had_split_highlight = v.highlight_split_pane.take().is_some();
                            if changed || had_split_highlight {
                                cx.notify();
                            }
                        }
                    });
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
            .on_drop({
                let h = h.clone();
                let pane_id = p_id.clone();
                move |data: &FileDragData, _, cx| {
                    let directories = h.read(cx).tab_drop_directories(data);
                    h.update(cx, |this, cx| {
                        for path in directories { this.add_tab(&pane_id, path, false, cx); }
                    });
                }
            })
            .child(tab_bar)
            .child({
                let origin = pane.tab_strip.drop_origin.clone();
                canvas(|_, _, _| (), move |bounds, _, _, _| origin.set(bounds.left().as_f32()))
                    .absolute()
                    .inset_0()
            })
            .when(self.dragging_tab.is_some(), |bar| {
                let target = (self.drop_target_pane.as_ref() == Some(&p_id))
                    .then_some(self.drop_target_idx)
                    .flatten();
                bar.child(drop_marker(&pane.tab_strip, &p_id, target, theme))
            })
    }
}

/// Records where a marker for `slot` would sit: a tab's left edge, or the last tab's right edge.
fn slot_probe(strip: &crate::ui::workspace::TabStripState, slot: usize, end: bool) -> impl IntoElement {
    let slots = strip.drop_slots.clone();
    canvas(|_, _, _| (), move |bounds, _, _, _| {
        let x = if end {
            (bounds.right() - crate::ui::scale::rem_pixels(rems(0.125))).as_f32()
        } else {
            bounds.left().as_f32()
        };
        let mut slots = slots.borrow_mut();
        if slots.len() <= slot {
            slots.resize(slot + 1, x);
        }
        slots[slot] = x;
    })
    .absolute()
    .inset_0()
}

/// One marker per strip, gliding between insertion slots and fading when no slot is targeted.
fn drop_marker(
    strip: &crate::ui::workspace::TabStripState,
    pane_id: &str,
    target: Option<usize>,
    theme: crate::ui::theme::OxydeTheme,
) -> impl IntoElement {
    let slot_x = target.and_then(|slot| strip.drop_slots.borrow().get(slot).copied());
    if let Some(x) = slot_x {
        strip.drop_marker_x.set(x - strip.drop_origin.get());
    }
    let id = |part: &str| SharedString::from(format!("tab-drop-marker-{part}-{pane_id}"));
    div()
        .absolute()
        .top_0()
        .w(rems(0.125))
        .h_full()
        .bg(theme.accent)
        .with_spring(id("x"), motion::glide(strip.drop_marker_x.get()), |marker, x| marker.left(px(x)))
        .with_spring(id("presence"), motion::presence(slot_x.is_some()).from(0.0), |marker, v| {
            marker.map_element(|marker| marker.opacity(v))
        })
}

#[cfg(test)]
mod tab_drop_tests {
    use super::update_tab_drop_target;
    use crate::ui::workspace::WorkspaceState;

    #[test]
    fn moving_from_a_tab_boundary_to_free_space_targets_the_actual_append_slot() {
        let mut state = WorkspaceState::new();
        let source = state.active_pane_id.clone();
        state.add_tab(&source, "dragged".into(), true);
        let dragged = state.find_pane(&source).unwrap().tabs[1].id.clone();
        let destination = state.add_panel_right().unwrap();
        state.add_tab(&destination, "second".into(), true);
        state.add_tab(&destination, "third".into(), true);
        let append_slot = state.find_pane(&destination).unwrap().tabs.len();
        let mut target_pane = Some(destination.clone());
        let mut target_index = Some(1);

        update_tab_drop_target(
            &mut target_pane,
            &mut target_index,
            &source,
            1,
            &destination,
            append_slot,
        );
        assert_eq!(target_pane.as_deref(), Some(destination.as_str()));
        assert_eq!(target_index, Some(append_slot));

        state.move_tab_to_pane_id(&source, 1, &destination);
        assert_eq!(
            state.find_pane(&destination).unwrap().tabs[target_index.unwrap()].id,
            dragged
        );
    }
    #[test]
    fn same_pane_free_space_marker_matches_the_reordered_tab() {
        let mut state = WorkspaceState::new();
        let pane_id = state.active_pane_id.clone();
        for path in ["second", "third", "fourth"] {
            state.add_tab(&pane_id, path.into(), true);
        }
        let dragged = state.find_pane(&pane_id).unwrap().tabs[0].id.clone();
        let append_slot = state.find_pane(&pane_id).unwrap().tabs.len();
        let mut target_pane = Some(pane_id.clone());
        let mut target_index = Some(2);
        update_tab_drop_target(
            &mut target_pane,
            &mut target_index,
            &pane_id,
            0,
            &pane_id,
            append_slot,
        );
        assert_eq!(target_index, Some(append_slot));
        state.reorder_tabs_id(&pane_id, 0, target_index.unwrap());
        assert_eq!(
            state.find_pane(&pane_id).unwrap().tabs.last().unwrap().id,
            dragged
        );
    }

    #[test]
    fn tab_half_refines_wrapper_target_and_noop_clears_both_fields() {
        let mut target_pane = None;
        let mut target_index = None;
        // Mouse capture handles the wrapper, then the half of the hovered tab.
        update_tab_drop_target(
            &mut target_pane,
            &mut target_index,
            "source",
            0,
            "target",
            4,
        );
        update_tab_drop_target(
            &mut target_pane,
            &mut target_index,
            "source",
            0,
            "target",
            1,
        );
        assert_eq!(target_pane.as_deref(), Some("target"));
        assert_eq!(target_index, Some(1));
        // The next event over the + button/free space restores the append slot.
        update_tab_drop_target(
            &mut target_pane,
            &mut target_index,
            "source",
            0,
            "target",
            4,
        );
        assert_eq!(target_index, Some(4));
        // Moving the last source tab to its own pane's end has no effect.
        update_tab_drop_target(
            &mut target_pane,
            &mut target_index,
            "source",
            3,
            "source",
            4,
        );
        assert!(target_pane.is_none());
        assert!(target_index.is_none());
        // Both adjacent boundaries around the source tab are also no-ops.
        for slot in [1, 2] {
            update_tab_drop_target(&mut target_pane, &mut target_index, "other", 0, "source", 4);
            update_tab_drop_target(
                &mut target_pane,
                &mut target_index,
                "source",
                1,
                "source",
                slot,
            );
            assert!(target_pane.is_none());
            assert!(target_index.is_none());
        }
    }

    #[test]
    fn overflow_tab_targets_are_clipped_to_the_visible_scroll_viewport() {
        use super::tab_drop_zone_contains_pointer;
        use gpui::{point, px, size, Bounds};
        let viewport = Bounds::new(point(px(40.0), px(20.0)), size(px(200.0), px(32.0)));
        let left_half = Bounds::new(point(px(0.0), px(20.0)), size(px(60.0), px(32.0)));
        assert!(!tab_drop_zone_contains_pointer(
            left_half,
            viewport,
            point(px(20.0), px(30.0))
        ));
        assert!(tab_drop_zone_contains_pointer(
            left_half,
            viewport,
            point(px(50.0), px(30.0))
        ));
        let offscreen_half = Bounds::new(point(px(260.0), px(20.0)), size(px(60.0), px(32.0)));
        assert!(!tab_drop_zone_contains_pointer(
            offscreen_half,
            viewport,
            point(px(280.0), px(30.0))
        ));
        assert!(!tab_drop_zone_contains_pointer(
            offscreen_half,
            viewport,
            point(px(260.0), px(20.0))
        ));
        assert!(!tab_drop_zone_contains_pointer(
            left_half,
            viewport,
            point(px(50.0), px(60.0))
        ));
    }
}
