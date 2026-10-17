use super::*;

impl Render for OxydeApp {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let reference = crate::ui::scale::normalized_reference(self.config.ui_rem_size);
        crate::ui::scale::set_reference(reference);
        _window.set_rem_size(px(reference as f32));
        let zoom_handle = cx.entity();
        let theme = *cx.theme();
        use crate::ui::components::sidebar::render_sidebar;
        use crate::ui::style::TNUM;

        let handle = self.handle.clone().expect("handle set");
        let window_handle = _window.window_handle();
        self.prepare_created_item_rename(_window, cx);

        let size = _window.viewport_size();
        if self.network_force_focus_pending {
            self.network_force_focus_pending = false;
            if let Some(confirmation) = &self.network_force_confirmation {
                _window.focus(&confirmation.focus, cx);
            }
        }
        if self.network_focus_restore {
            self.network_focus_restore = false;
            _window.focus(&self.focus_handle, cx);
        }
        if let Some(dialog) = self.network_dialog.as_ref().filter(|dialog| dialog.busy) {
            if !dialog.focus.is_focused(_window) { _window.focus(&dialog.focus, cx); }
        }
        let win_w = size.width.as_f32();
        let win_h = size.height.as_f32();

        self.list_width = (win_w - self.sidebar_display_width() - logical(3.0)).max(1.0);
        self.list_height = (win_h - crate::ui::components::title_bar::titlebar_h()).max(1.0);

        if let Some(DragTarget::SelectionRectangle {
            ref pane_id,
            current_pos,
            ..
        }) = self.drag_target
        {
            let p_id = pane_id.clone();
            let pos = current_pos;
            self.handle_selection_drag(p_id, pos, cx);
        }
        self.shift_delete = _window.modifiers().shift;
        self.measure_metadata_columns(_window, cx);
        self.clamp_scrolls();
        self.follow_active_path(cx);

        let settings_keyboard = handle.clone();
        let mut root = div()
            .on_key_down(move |event, window, cx| {
                let handled = settings_keyboard.update(cx, |this, cx| this.handle_file_key(event, window, cx));
                if handled { cx.stop_propagation(); return; }
                let peek = settings_keyboard.update(cx, |this, cx| {
                    if this.focus_handle.is_focused(window) && this.peek_shortcut_matches(&event.keystroke) {
                        this.peek_selected(cx);
                        true
                    } else { false }
                });
                if peek { cx.stop_propagation(); return; }
                if event.keystroke.key == "escape" {
                    settings_keyboard.update(cx, |this, cx| {
                        if this.pinned_rail_open {
                            this.pinned_rail_open = false;
                            cx.stop_propagation();
                            cx.notify();
                            return;
                        }
                        if !this.context_menus.is_empty() {
                            this.context_menus.clear();
                            cx.stop_propagation();
                            cx.notify();
                            return;
                        }
                        if this.settings_open {
                            if this.settings_picker.is_some() {
                                this.settings_picker = None;
                                this.settings_submenu_bounds = None;
                            } else {
                                this.settings_open = false;
                                this.settings_about_open = false;
                            }
                            cx.stop_propagation();
                            cx.notify();
                        }
                    });
                }
            })
            .track_focus(&self.focus_handle)
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().text)
            .font_family("Inter")
            .flex()
            .flex_col()
            .text_size(rems(0.8125))
            .font_weight(crate::ui::style::DEFAULT_FONT_WEIGHT)
            .child(canvas(|_, _, _| (), move |_, _, window, _| {
                // Low-level capture listeners must be installed during paint.
                let zoom_handle = zoom_handle.clone();
                window.on_mouse_event(move |event: &ScrollWheelEvent, phase, window, cx| {
                    if phase == DispatchPhase::Capture && event.modifiers.control {
                        let delta = event.delta.pixel_delta(window.line_height());
                        if delta.y != px(0.0) {
                            zoom_handle.update(cx, |this, cx| {
                                let current = this.ui_rem_size();
                                let next = if delta.y > px(0.0) { current + 1 } else { current.saturating_sub(1) };
                                this.set_ui_rem_size(next, window, cx);
                            });
                            cx.stop_propagation();
                        }
                    }
                });
            }).absolute().size_full())
            .on_action(cx.listener(Self::on_action_tout_selectionner))
            .on_action(cx.listener(Self::on_action_annuler))
            .on_action(cx.listener(Self::on_action_refaire))
            .on_action(cx.listener(|this, _: &Actualiser, window, cx| this.refresh_active_view(window, cx)))
            .on_action(cx.listener(Self::on_action_copier))
            .on_action(cx.listener(|this, _: &CopierNom, _, cx| this.execute_copy_text(true, cx)))
            .on_action(cx.listener(|this, _: &CopierChemin, _, cx| this.execute_copy_text(false, cx)))
            .on_action(cx.listener(Self::on_action_couper))
            .on_action(cx.listener(Self::on_action_coller))
            .on_action(cx.listener(Self::on_action_ouvrir))
            .on_action(cx.listener(Self::on_action_renommer))
            .on_action(cx.listener(Self::on_action_supprimer));

        root = root.on_modifiers_changed({
            let h = handle.clone();
            move |e: &ModifiersChangedEvent, _, cx| {
                let modifier = if e.modifiers.control {
                    Some(true) // Ctrl → always copy
                } else if e.modifiers.shift {
                    Some(false) // Shift → always move
                } else {
                    None // auto → determined by drive context
                };
                h.update(cx, |this, cx| {
                    if this.shift_delete != e.modifiers.shift {
                        this.shift_delete = e.modifiers.shift;
                        cx.notify();
                    }
                    if this.drag_modifier_copy != modifier {
                        this.drag_modifier_copy = modifier;
                        this.update_drag_copy_mode(cx);
                    }
                });
            }
        });

        root.text_style().font_features = Some(TNUM.clone());

        root = root
            .on_drag_move::<FileDragData>({
                let h = handle.clone();
                move |_, _, cx| h.update(cx, |this, cx| this.set_file_drag_new_tab(false, cx))
            })
            .child(canvas(|bounds, _, _| bounds, {
                let h = handle.clone();
                move |_, _, window, _| {
                    let cancel_handle = h.clone();
                    window.on_mouse_event(move |event: &MouseDownEvent, phase, window, cx| {
                        if phase == DispatchPhase::Capture {
                            cancel_handle.update(cx, |this, _| {
                                this.suppress_item_selection_click = false;
                            });
                        }
                        if phase == DispatchPhase::Capture && event.button == MouseButton::Right
                            && cancel_handle.update(cx, |this, cx| this.cancel_file_drag(window, cx)) {
                            cx.stop_propagation();
                        }
                    });
                    let release_handle = h.clone();
                    window.on_mouse_event(move |event: &MouseUpEvent, phase, _, cx| {
                        if phase == DispatchPhase::Capture && event.button == MouseButton::Left {
                            release_handle.update(cx, |this, cx| {
                                if this.item_selection_drag {
                                    // Let GPUI observe the release and clear its pending press.
                                    // Suppress only the resulting item click, not event propagation.
                                    if let Some(DragTarget::SelectionRectangle { pane_id, .. }) = &this.drag_target {
                                        this.handle_selection_drag(pane_id.clone(), event.position, cx);
                                    }
                                    this.finish_pointer_drag(cx);
                                    this.suppress_item_selection_click = true;
                                } else {
                                    this.item_press = None;
                                }
                            });
                        }
                    });
                    let h = h.clone();
                    window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, cx| {
                        if phase == DispatchPhase::Capture {
                            h.update(cx, |this, cx| {
                                this.handle_pointer_move(event, window, cx);
                            });
                        }
                    });
                }
            }).absolute().size_full())
            .on_mouse_up_out(MouseButton::Left, {
                let h = handle.clone();
                move |_, _, cx| h.update(cx, |this, cx| this.finish_pointer_drag(cx))
            })
            .on_mouse_up(MouseButton::Left, {
                let h = handle.clone();
                move |_, _window, cx| {
                    h.update(cx, |this, cx| {
                        this.finish_pointer_drag(cx);
                    });
                }
            })
            .on_mouse_down(MouseButton::Navigate(NavigationDirection::Back), {
                let h = handle.clone();
                move |_, _, cx| {
                    h.update(cx, |v, cx| {
                        let p_id = v.state.active_pane_id.clone();
                        v.go_back(&p_id, cx)
                    });
                }
            })
            .on_mouse_down(MouseButton::Navigate(NavigationDirection::Forward), {
                let h = handle.clone();
                move |_, _, cx| {
                    h.update(cx, |v, cx| {
                        let p_id = v.state.active_pane_id.clone();
                        v.go_forward(&p_id, cx)
                    });
                }
            })
            .child({
                // ── Custom title bar ──────────────────────────────────────
                use crate::ui::components::title_bar::render_title_bar;
                render_title_bar(self, handle.clone(), _window.is_maximized(), cx)
            })
            .child({
                // ── Horizontal content area (sidebar + panes) ─────────────
                let layout_tree = self.state.layout.clone();
                div()
                    .flex()
                    .flex_1()
                    .w_full()
                    .overflow_hidden()
                    .child(
                        div()
                            .w(px(self.sidebar_display_width()))
                            .h_full()
                            .flex_none()
                            .child(render_sidebar(self, handle.clone(), cx)),
                    )
                    .child(
                        div()
                            .id("sidebar-resize-handle")
                            .group("sidebar-resize-handle")
                            .relative()
                            .w(rems(0.0625))
                            .bg(theme.border)
                            .cursor(CursorStyle::ResizeLeftRight)
                            .h_full()
                            .flex_none()
                            .flex()
                            .justify_center()
                            .child(
                                deferred(div()
                                    .group("sidebar-resize-hit-area")
                                    .absolute()
                                    .left(rems(-0.0625))
                                    .w(rems(0.1875))
                                    .h_full()
                                    .occlude()
                                    .cursor(CursorStyle::ResizeLeftRight)
                                    .child(
                                        div()
                                            .absolute()
                                            .left(rems(0.0625))
                                            .top_0()
                                            .bottom_0()
                                            .w(rems(0.0625))
                                            .group_hover("sidebar-resize-hit-area", |s| {
                                                s.bg(theme.accent.opacity(0.6))
                                            }),
                                    )
                                    .on_mouse_down(MouseButton::Left, {
                                        let h = handle.clone();
                                        move |e, _, cx| {
                                            cx.stop_propagation();
                                            h.update(cx, |this, cx| {
                                                this.drag_target = Some(DragTarget::SidebarResize);
                                                this.grab_offset = e.position.x.as_f32() - this.sidebar_display_width();
                                                cx.notify();
                                            });
                                        }
                                    })),
                            )
                            .child(
                                div()
                                    .w(rems(0.0625))
                                    .h_full()
                                    .bg(theme.border)
                                    .group_hover("sidebar-resize-handle", |s| {
                                        s.bg(theme.accent.opacity(0.6))
                                    }),
                            )
                            .on_mouse_down(MouseButton::Left, {
                                let h = handle.clone();
                                move |e, _, cx| {
                                    h.update(cx, |this, _| {
                                        this.drag_target = Some(DragTarget::SidebarResize);
                                        this.grab_offset =
                                            e.position.x.as_f32() - this.sidebar_display_width();
                                    });
                                }
                            }),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .h_full()
                            .overflow_hidden()
                            .child(self.render_layout_node(&layout_tree, handle.clone(), cx)),
                    )
            });

        root = self.render_context_menus(root, handle.clone(), window_handle, win_h, _window, cx);

        // SETTINGS OVERLAY

        if self.sidebar_collapsed && self.pinned_rail_open { root = root.child(crate::ui::components::sidebar::render_pinned_menu(self, handle.clone(), _window, cx)); }
        if self.settings_open {
            use crate::ui::components::settings::render_settings_panel;
            root = root.child(deferred(render_settings_panel(self, handle.clone(), _window, cx)));
        }

        self.ensure_dialog_window(dialog_windows::DialogKind::Trash, _window, cx);
        self.ensure_dialog_window(dialog_windows::DialogKind::Conflict, _window, cx);
        // Modal prompts are fixed native windows centered over this one.
        for kind in dialog_windows::DialogKind::FIXED {
            self.ensure_dialog_window(kind, _window, cx);
        }

        root = root.child(deferred(self.render_toasts(_window, cx)));
        root
    }
}
