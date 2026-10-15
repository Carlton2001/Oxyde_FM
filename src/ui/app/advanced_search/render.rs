use super::super::ContextMenuContent;
use super::AdvancedSearchWindow;
use crate::ui::{
    i18n::{I18nContext, Message},
    theme::ThemeContext,
};
use gpui::prelude::*;
use gpui::*;

impl AdvancedSearchWindow {
    pub(crate) fn render_advanced_search(
        &self,
        handle: Entity<Self>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let d = self.advanced_search.as_ref().unwrap();
        let theme = *cx.theme();
        let units = match cx.i18n().language() {
            crate::ui::i18n::Language::French => ["o", "Ko", "Mo", "Go", "To"],
            _ => ["B", "KB", "MB", "GB", "TB"],
        };
        let mut body = div()
            .flex()
            .flex_col()
            .gap(rems(0.75))
            .when(d.help, |body| body.h_full().min_h_0());
        let mut tabs = div()
            .flex()
            .flex_none()
            .items_center()
            .px(rems(1.25))
            .bg(theme.dialog_tabs)
            .gap(rems(0.25))
            .border_b(rems(0.0625))
            .border_color(theme.border);
        for (help, message) in [
            (false, Message::AdvancedSettings),
            (true, Message::AdvancedHelp),
        ] {
            let h = handle.clone();
            tabs = tabs.child(
                div()
                    .id(if help {
                        "advanced-help"
                    } else {
                        "advanced-settings"
                    })
                    .px(rems(1.0))
                    .py(rems(0.75))
                    .text_size(rems(0.8125))
                    .font_weight(FontWeight::MEDIUM)
                    .flex()
                    .items_center()
                    .justify_center()
                    .when(d.help != help, |tab| {
                        tab.hover(move |style| style.bg(theme.hover).text_color(theme.text))
                    })
                    .gap(rems(0.5))
                    .child(
                        svg()
                            .path(if help {
                                "icons/regex.svg"
                            } else {
                                "icons/target.svg"
                            })
                            .size(rems(0.875))
                            .text_color(if d.help == help {
                                theme.accent
                            } else {
                                theme.text_muted
                            }),
                    )
                    .track_focus(&d.controls[if help { 7 } else { 6 }])
                    .rounded_none()
                    .border_b(rems(0.0625))
                    .border_color(if d.help == help {
                        theme.accent
                    } else {
                        transparent_black()
                    })
                    .text_color(if d.help == help {
                        theme.accent
                    } else {
                        theme.text_muted
                    })
                    .child(cx.i18n().text(message))
                    .on_click(move |_, _, cx| {
                        h.update(cx, |this, cx| {
                            if let Some(d) = &mut this.advanced_search {
                                d.help = help;
                            }
                            cx.notify();
                        });
                    }),
            );
        }
        if d.help {
            body = self.render_advanced_help(body, handle.clone(), cx);
        } else {
            body = self.render_advanced_settings(body, handle.clone(), units, window, cx);
        }
        if d.error {
            body = body.child(
                div()
                    .text_color(theme.error)
                    .child(cx.i18n().text(Message::AdvancedInvalid)),
            );
        }
        let reset = handle.clone();
        let cancel = handle.clone();
        let submit = handle.clone();
        let keyboard = handle.clone();
        let frame = div()
            .relative()
            .w_full()
            .when(d.help, |frame| frame.h_full())
            .when(!d.help, |frame| {
                frame.child(
                    canvas(
                        |bounds, _, _| bounds,
                        |bounds, _, window, _| {
                            // Fit the native content size to the form, not a saved oversized viewport.
                            if (window.viewport_size().height - bounds.size.height).abs() > px(1.0)
                            {
                                window.on_next_frame(move |window, _| window.resize(bounds.size));
                            }
                        },
                    )
                    .absolute()
                    .size_full(),
                )
            })
            .flex()
            .flex_col()
            .child(crate::ui::components::tool_window::header(
                "icons/search.svg",
                cx.i18n().text(Message::ToolSearch),
                window.is_maximized(),
                false,
                cx,
                |window, cx| { crate::ui::components::tool_window::close_transients(window, cx); window.remove_window() },
            ))
            .child(tabs)
            .child(
                div()
                    .id("advanced-form-scroll")
                    .when(d.help, |body| body.flex_1().min_h_0())
                    .when(!d.help, |body| body.flex_none())
                    .overflow_y_scroll()
                    .px(rems(1.0))
                    .py(rems(1.25))
                    .child(body),
            )
            .child(
                crate::ui::components::tool_window::footer(cx)
                    .child(
                        div().flex_1().flex().child(
                            crate::ui::components::tool_window::action_button(
                                "advanced-reset",
                                "".into(),
                                false,
                                cx,
                            )
                            .track_focus(&d.controls[11])
                            .child(cx.i18n().text(Message::AdvancedReset))
                            .on_click(move |_, window, cx| {
                                reset.update(cx, |this, cx| {
                                    let Some(d) = &mut this.advanced_search else {
                                        return;
                                    };
                                    d.flags = [false, false, true, false, false, false];
                                    d.unit = 1;
                                    d.error = false;
                                    for i in [0, 2, 3, 4, 5, 6] {
                                        d.values[i].clear();
                                        d.inputs[i].update(cx, |input, cx| {
                                            input.set_content(String::new(), cx)
                                        });
                                    }
                                    let focus = d.inputs[0].read(cx).focus_handle.clone();
                                    window.focus(&focus, cx);
                                    cx.notify();
                                });
                            }),
                        ),
                    )
                    .child(
                        crate::ui::components::tool_window::action_button(
                            "advanced-cancel",
                            "".into(),
                            false,
                            cx,
                        )
                        .track_focus(&d.controls[12])
                        .child(cx.i18n().text(Message::Cancel))
                        .on_click(move |_, window, cx| {
                            cancel.update(cx, |this, cx| this.close_advanced_search(window, cx));
                        }),
                    )
                    .child(
                        crate::ui::components::tool_window::action_button(
                            "advanced-submit",
                            "".into(),
                            true,
                            cx,
                        )
                        .track_focus(&d.controls[13])
                        .child(
                            svg()
                                .path("icons/search.svg")
                                .size(rems(0.875))
                                .text_color(theme.text_on_accent),
                        )
                        .child(cx.i18n().text(Message::AdvancedSearch))
                        .on_click(move |_, window, cx| {
                            submit.update(cx, |this, cx| this.submit_advanced_search(window, cx));
                        }),
                    ),
            );
        let mut layer = div()
            .relative()
            .size_full()
            .track_focus(&d.focus)
            // Block file actions at the modal boundary while allowing TextInput
            // actions and unhandled keystrokes to reach its text input handler.
            .on_action(|_: &super::super::Ouvrir, _, cx| cx.stop_propagation())
            .on_action(|_: &super::super::Copier, _, cx| cx.stop_propagation())
            .on_action(|_: &super::super::Couper, _, cx| cx.stop_propagation())
            .on_action(|_: &super::super::Coller, _, cx| cx.stop_propagation())
            .on_action(|_: &super::super::Renommer, _, cx| cx.stop_propagation())
            .on_action(|_: &super::super::Supprimer, _, cx| cx.stop_propagation())
            .on_action(|_: &super::super::ToutSelectionner, _, cx| cx.stop_propagation())
            .on_action(|_: &super::super::Annuler, _, cx| cx.stop_propagation())
            .on_action(|_: &super::super::Refaire, _, cx| cx.stop_propagation())
            .on_action(|_: &super::super::Actualiser, _, cx| cx.stop_propagation())
            .on_key_down(move |event, window, cx| {
                keyboard.update(cx, |this, cx| {
                    if event.keystroke.key == "escape" && !this.context_menus.is_empty() {
                        this.context_menus.clear();
                        cx.stop_propagation();
                        cx.notify();
                        return;
                    }
                    if let Some(d) = &mut this.advanced_search {
                        if let Some((index, date)) = d.calendar {
                            let delta = match event.keystroke.key.as_str() {
                                "left" => -1,
                                "right" => 1,
                                "up" => -7,
                                "down" => 7,
                                _ => 0,
                            };
                            if delta != 0 {
                                d.calendar = Some((index, date + chrono::Duration::days(delta)));
                            } else if event.keystroke.key == "enter" {
                                let value = date.format("%Y-%m-%d").to_string();
                                d.values[index] = value.clone();
                                d.inputs[index]
                                    .update(cx, |input, cx| input.set_content(value, cx));
                                d.calendar = None;
                            } else if event.keystroke.key == "escape" {
                                d.calendar = None;
                            }
                            cx.stop_propagation();
                            cx.notify();
                            return;
                        }
                    }
                    match event.keystroke.key.as_str() {
                        "escape" => {
                            if this
                                .advanced_search
                                .as_ref()
                                .is_some_and(|d| d.calendar.is_some())
                            {
                                this.advanced_search.as_mut().unwrap().calendar = None;
                                cx.notify();
                            } else {
                                this.close_advanced_search(window, cx);
                            }
                        }
                        "space" | "enter"
                            if this.advanced_search.as_ref().is_some_and(|d| {
                                d.controls.iter().any(|f| f.is_focused(window))
                            }) =>
                        {
                            let d = this.advanced_search.as_mut().unwrap();
                            let control = d
                                .controls
                                .iter()
                                .position(|f| f.is_focused(window))
                                .unwrap();
                            match control {
                                0..=5 => d.flags[control] = !d.flags[control],
                                6 | 7 => d.help = control == 7,
                                8 => d.unit = (d.unit + 1) % 5,
                                9 | 10 => {
                                    d.calendar =
                                        Some((control - 4, chrono::Local::now().date_naive()))
                                }
                                11 => {
                                    d.flags = [false, false, true, false, false, false];
                                    d.unit = 1;
                                    d.error = false;
                                    for i in [0, 2, 3, 4, 5, 6] {
                                        d.values[i].clear();
                                        d.inputs[i].update(cx, |input, cx| {
                                            input.set_content(String::new(), cx)
                                        });
                                    }
                                }
                                12 => this.close_advanced_search(window, cx),
                                13 => this.submit_advanced_search(window, cx),
                                _ => {}
                            }
                            cx.notify();
                        }
                        "enter" => this.submit_advanced_search(window, cx),
                        "tab" => {
                            if let Some(d) = &this.advanced_search {
                                let mut order = vec![d.controls[6].clone(), d.controls[7].clone()];
                                if !d.help {
                                    order.extend([
                                        d.controls[0].clone(),
                                        d.inputs[0].read(cx).focus_handle.clone(),
                                        d.inputs[1].read(cx).focus_handle.clone(),
                                        d.controls[2].clone(),
                                        d.controls[3].clone(),
                                        d.controls[1].clone(),
                                        d.inputs[2].read(cx).focus_handle.clone(),
                                        d.inputs[3].read(cx).focus_handle.clone(),
                                        d.inputs[4].read(cx).focus_handle.clone(),
                                        d.controls[8].clone(),
                                        d.inputs[5].read(cx).focus_handle.clone(),
                                        d.controls[9].clone(),
                                        d.inputs[6].read(cx).focus_handle.clone(),
                                        d.controls[10].clone(),
                                        d.controls[4].clone(),
                                        d.controls[5].clone(),
                                    ]);
                                }
                                order.extend(d.controls[11..].iter().cloned());
                                let current = order.iter().position(|f| f.is_focused(window));
                                let next = if event.keystroke.modifiers.shift {
                                    current.map_or(order.len() - 1, |i| {
                                        (i + order.len() - 1) % order.len()
                                    })
                                } else {
                                    current.map_or(0, |i| (i + 1) % order.len())
                                };
                                window.focus(&order[next], cx);
                            }
                        }
                        _ => return,
                    }
                    cx.stop_propagation();
                });
            })
            .child(frame);
        layer = self.render_advanced_calendar(layer, handle, cx);
        layer.into_any_element()
    }
}

impl Render for AdvancedSearchWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = *cx.theme();
        window.set_window_title(cx.i18n().text(Message::ToolSearch).as_ref());
        if let Some(owner) = self.owner.upgrade() {
            window.set_rem_size(px(owner.read(cx).ui_rem_size() as f32));
        }
        let handle = cx.entity();
        let mut root = div()
            .size_full()
            .relative()
            .bg(theme.active_background)
            .text_color(theme.text)
            .font_family("Inter")
            .font_weight(crate::ui::style::DEFAULT_FONT_WEIGHT)
            .text_size(rems(0.8125))
            .line_height(relative(1.25));
        if self.advanced_search.is_some() {
            root = root.child(self.render_advanced_search(handle.clone(), window, cx));
        }
        // These menus belong to this window, including its text input actions.
        {
            for (index, menu) in self.context_menus.iter().enumerate() {
                let ContextMenuContent::Items(items, _) = &menu.content else {
                    continue;
                };
                let close = handle.clone();
                #[cfg(target_os = "windows")]
                if crate::ui::components::popup::supported(window, cx) {
                    let scroll = menu.scroll_handle.clone();
                    crate::ui::components::popup::text_menu(
                        format!("advanced-text-menu-{index}"), crate::ui::components::popup::PopupAnchor::point(menu.position, false),
                        close.clone(), handle.clone(), items.clone(), scroll.clone(), window, cx,
                        move |view| view.context_menus.get(index).is_some_and(|menu| std::rc::Rc::ptr_eq(&menu.scroll_handle.0, &scroll.0)),
                        |view, _, cx| { view.context_menus.clear(); cx.notify(); },
                    );
                    continue;
                }
                                {
                let width = crate::ui::scale::rem_pixels(rems(13.0));
                let height = crate::ui::scale::rem_pixels(rems(items.len() as f32 * 1.625 + 1.0));
                let viewport = window.viewport_size();
                let position = point(
                    menu.position.x.min((viewport.width - width).max(px(0.0))),
                    menu.position.y.min((viewport.height - height).max(px(0.0))),
                );
                root = root.child(deferred(
                    div().absolute().left(position.x).top(position.y).child(
                        crate::ui::components::dropdown::GenericDropdown {
                            id: ("advanced-menu", index).into(),
                            items: items.clone(),
                            reserve_icon_space: true,
                            is_context_menu: true,
                            bottom_padding: 4.0,
                            scroll_handle: menu.scroll_handle.clone(),
                            h_app: handle.clone(),
                            on_drag: std::rc::Rc::new(|_, _, _, _, _| {}),
                            on_close: std::rc::Rc::new(move |_, cx| {
                                close.update(cx, |this, cx| {
                                    this.context_menus.clear();
                                    cx.notify();
                                });
                            }),
                            on_outside: None,
                            is_dragging: false,
                            width: Some(width.as_f32()),
                            max_height: viewport.height.as_f32(),
                            header: None,
                            on_item_hover: None,
                            on_width_calculated: None,
                            on_hover_change: None,
                        },
                    ),
                ));
                }
            }
        }
        root
    }
}
