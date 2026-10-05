use crate::ui::app::{
    network::{drive_letter_used, network_drive},
    OxydeApp,
};
use crate::ui::components::scrollbar::{
    render_scrollbar_with_pointer, ScrollbarAxis, ZED_TRACK_WIDTH,
};
use crate::ui::components::{
    input::TextInput,
    modal::{action_button, form_row, Modal},
};
use crate::ui::i18n::{FluentArgs, I18nContext, Message};
use crate::ui::scale::rem_pixels;
use crate::ui::theme::ThemeContext;
use gpui::prelude::*;
use gpui::*;

pub(crate) fn render_force_confirmation(
    view: &OxydeApp,
    handle: Entity<OxydeApp>,
    window: &mut Window,
    cx: &mut Context<OxydeApp>,
) -> impl IntoElement {
    let confirmation = view
        .network_force_confirmation
        .as_ref()
        .expect("force confirmation");
    let theme = *cx.theme();
    let mut args = FluentArgs::new();
    args.set(
        "drive",
        confirmation.paths.first().cloned().unwrap_or_default(),
    );
    let cancel = handle.clone();
    let force = handle.clone();
    let keyboard = handle.clone();
    let close = handle.clone();
    div()
        .absolute()
        .inset_0()
        .track_focus(&confirmation.focus)
        .on_key_down(move |event, window, cx| {
            keyboard.update(cx, |view, cx| match event.keystroke.key.as_str() {
                "escape" => view.resolve_network_force(false, window, cx),
                "tab" => {
                    if let Some(confirmation) = &view.network_force_confirmation {
                        let focus = if confirmation.confirm_focus.is_focused(window) {
                            &confirmation.focus
                        } else {
                            &confirmation.confirm_focus
                        };
                        window.focus(focus, cx);
                    }
                }
                "enter" | "space" => {
                    let force = view
                        .network_force_confirmation
                        .as_ref()
                        .is_some_and(|confirmation| confirmation.confirm_focus.is_focused(window));
                    view.resolve_network_force(force, window, cx);
                }
                _ => {}
            });
            cx.stop_propagation();
        })
        .on_action(|_: &crate::ui::app::Supprimer, _, cx| cx.stop_propagation())
        .on_action(|_: &crate::ui::app::Ouvrir, _, cx| cx.stop_propagation())
        .on_action(|_: &crate::ui::app::Renommer, _, cx| cx.stop_propagation())
        .on_action(|_: &crate::ui::app::Copier, _, cx| cx.stop_propagation())
        .on_action(|_: &crate::ui::app::Couper, _, cx| cx.stop_propagation())
        .on_action(|_: &crate::ui::app::Coller, _, cx| cx.stop_propagation())
        .on_action(|_: &crate::ui::app::ToutSelectionner, _, cx| cx.stop_propagation())
        .child(
            Modal::new(
                "network-force-confirmation",
                cx.i18n().text(Message::DisconnectThisNetworkDrive),
                div().child(cx.i18n().text_with_args(Message::NetworkForcePrompt, &args)),
            )
            .title_icon("icons/triangle-alert.svg")
            .header(
                div()
                    .w_full()
                    .flex()
                    .items_center()
                    .gap(rems(0.5))
                    .child(
                        svg()
                            .path("icons/triangle-alert.svg")
                            .size(rems(1.0))
                            .text_color(theme.error),
                    )
                    .child(
                        div()
                            .flex_1()
                            .text_size(rems(0.875))
                            .child(cx.i18n().text(Message::DisconnectThisNetworkDrive)),
                    )
                    .child(
                        div()
                            .id("network-force-close")
                            .p_1()
                            .on_click(move |_, window, cx| {
                                close.update(cx, |view, cx| {
                                    view.resolve_network_force(false, window, cx)
                                });
                            })
                            .child(
                                svg()
                                    .path("icons/x.svg")
                                    .size(rems(1.0))
                                    .text_color(theme.text_muted),
                            ),
                    ),
            )
            .action(
                action_button("network-force-cancel")
                    .bg(theme.control_hover)
                    .border(rems(0.0625))
                    .border_color(if confirmation.focus.is_focused(window) {
                        theme.accent
                    } else {
                        theme.control_hover
                    })
                    .child(cx.i18n().text(Message::Cancel))
                    .on_click(move |_, window, cx| {
                        cancel.update(cx, |view, cx| view.resolve_network_force(false, window, cx));
                    }),
            )
            .action(
                action_button("network-force-accept")
                    .track_focus(&confirmation.confirm_focus)
                    .bg(theme.error)
                    .text_color(theme.text_on_accent)
                    .border(rems(0.0625))
                    .border_color(if confirmation.confirm_focus.is_focused(window) {
                        theme.text
                    } else {
                        theme.error
                    })
                    .child(cx.i18n().text(Message::NetworkForceAction))
                    .on_click(move |_, window, cx| {
                        force.update(cx, |view, cx| view.resolve_network_force(true, window, cx));
                    }),
            ),
        )
}

fn update_letter_scroll(view: &mut OxydeApp, position: Point<Pixels>, cx: &mut App) {
    let Some(dialog) = &mut view.network_dialog else {
        return;
    };
    let Some((track, viewport, content)) = &dialog.letter_scroll_drag else {
        return;
    };
    let Some(origin) = view.scrollbar_origins.get(track) else {
        return;
    };
    let thumb = (viewport * viewport / content).max(rem_pixels(rems(2.5)).as_f32());
    let travel = (viewport - thumb).max(1.0);
    let position = position.y.as_f32() - origin.y - view.grab_offset;
    let offset = (position / travel).clamp(0.0, 1.0) * (content - viewport);
    dialog.letter_scroll.set_offset(point(px(0.0), px(-offset)));
    if let Some(handle) = &view.handle {
        cx.notify(handle.entity_id());
    }
}
fn field(
    label: SharedString,
    input: Entity<TextInput>,
    busy: bool,
    secret: bool,
    cx: &App,
) -> AnyElement {
    let value = input.read(cx).content();
    let body = if busy {
        div()
            .h(rems(2.0))
            .px_3()
            .flex()
            .items_center()
            .child(if secret {
                "*".repeat(value.chars().count())
            } else {
                value.to_string()
            })
            .into_any_element()
    } else {
        div().h(rems(2.0)).child(input).into_any_element()
    };
    form_row(label, body, cx).into_any_element()
}

pub(crate) fn render_network_dialog(
    view: &OxydeApp,
    handle: Entity<OxydeApp>,
    window: &mut Window,
    cx: &mut Context<OxydeApp>,
) -> impl IntoElement {
    let dialog = view.network_dialog.as_ref().expect("network dialog");
    let theme = *cx.theme();
    let disconnect = dialog.disconnect;
    let busy = dialog.busy;
    let disabled = busy
        || dialog.loading
        || if disconnect {
            dialog.selected.is_empty()
        } else {
            dialog.letter.is_none() || dialog.values[0].trim().is_empty()
        };
    let title = cx.i18n().text(if disconnect {
        Message::DisconnectNetworkDrive
    } else {
        Message::MapNetworkDrive
    });
    let outside = handle.clone();
    let mut body = div()
        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
            outside.update(cx, |view, cx| {
                if let Some(dialog) = &mut view.network_dialog {
                    if dialog.letter_open {
                        dialog.letter_open = false;
                        cx.notify();
                    }
                }
            })
        })
        .flex()
        .flex_col()
        .gap(rems(0.375))
        .text_size(rems(0.8125));
    if disconnect {
        body = body.child(cx.i18n().text(Message::NetworkDisconnectDescription));
        let mut list = div()
            .id("network-drive-list")
            .flex()
            .flex_col()
            .gap(rems(0.5))
            .max_h(rems(16.0))
            .overflow_y_scroll()
            .border(rems(0.0625))
            .border_color(theme.border)
            .rounded_md()
            .p(rems(0.75));
        if dialog.loading {
            list = list.child(cx.i18n().text(Message::NetworkLoading));
        } else if !dialog.drives.iter().any(network_drive) {
            list = list.child(
                div()
                    .py_6()
                    .text_color(theme.text_muted)
                    .child(cx.i18n().text(Message::NetworkNoDrives)),
            );
        } else {
            for (index, drive) in dialog
                .drives
                .iter()
                .filter(|drive| network_drive(drive))
                .enumerate()
            {
                let selected = dialog.selected.contains(&drive.path);
                let path = drive.path.clone();
                let h = handle.clone();
                list = list.child(
                    div()
                        .id(("disconnect-drive", index))
                        .track_focus(&dialog.drive_focus[index])
                        .flex()
                        .items_center()
                        .gap(rems(0.75))
                        .px_3()
                        .py_2()
                        .rounded_md()
                        .border(rems(0.0625))
                        .border_color(
                            if selected || dialog.drive_focus[index].is_focused(window) {
                                theme.accent
                            } else {
                                theme.border
                            },
                        )
                        .bg(if selected {
                            theme.selected
                        } else {
                            theme.surface
                        })
                        .when(!busy, |row| {
                            row.hover(|style| style.bg(theme.control_hover))
                        })
                        .on_click(move |_, window, cx| {
                            h.update(cx, |view, cx| {
                                if let Some(dialog) = &mut view.network_dialog {
                                    if !dialog.busy {
                                        window.focus(&dialog.drive_focus[index], cx);
                                        if !dialog.selected.remove(&path) {
                                            dialog.selected.insert(path.clone());
                                        }
                                        cx.notify();
                                    }
                                }
                            });
                        })
                        .child(
                            svg()
                                .path(if selected {
                                    "icons/check.svg"
                                } else {
                                    "icons/square.svg"
                                })
                                .size(rems(1.0))
                                .text_color(theme.accent),
                        )
                        .child(
                            svg()
                                .path("icons/hard-drive.svg")
                                .size(rems(1.125))
                                .text_color(theme.text_muted),
                        )
                        .child(
                            div()
                                .min_w_0()
                                .flex_1()
                                .flex()
                                .flex_col()
                                .child(drive.path.clone())
                                .child(
                                    div()
                                        .text_size(rems(0.75))
                                        .text_color(theme.text_muted)
                                        .text_ellipsis()
                                        .child(
                                            drive
                                                .remote_path
                                                .clone()
                                                .unwrap_or_else(|| drive.label.clone()),
                                        ),
                                ),
                        ),
                );
            }
        }
        body = body.child(list);
    } else {
        body = body
            .child(
                div()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(cx.i18n().text(Message::NetworkMapDescription)),
            )
            .child(
                div()
                    .text_size(rems(0.75))
                    .text_color(theme.text_muted)
                    .child(cx.i18n().text(Message::NetworkMapHint)),
            );
        let h = handle.clone();
        let mut letter = div().relative().flex().flex_col().child(
            div()
                .id("network-letter")
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .track_focus(&dialog.controls[0])
                .flex()
                .items_center()
                .justify_between()
                .border(rems(0.0625))
                .border_color(if dialog.controls[0].is_focused(window) {
                    theme.accent
                } else {
                    theme.border
                })
                .rounded_md()
                .px_3()
                .h(rems(2.0))
                .when(!busy && !dialog.loading, |d| {
                    d.hover(|s| s.bg(theme.control_hover))
                })
                .on_click(move |_, window, cx| {
                    h.update(cx, |view, cx| {
                        if let Some(dialog) = &mut view.network_dialog {
                            if !dialog.busy && !dialog.loading {
                                dialog.letter_open = !dialog.letter_open;
                                window.focus(&dialog.controls[0], cx);
                                cx.notify();
                            }
                        }
                    })
                })
                .child(
                    dialog
                        .letter
                        .clone()
                        .map(SharedString::from)
                        .unwrap_or_else(|| cx.i18n().text(Message::NetworkNoLetter)),
                )
                .child(
                    svg()
                        .path("icons/chevron-down.svg")
                        .size(rems(0.875))
                        .text_color(theme.text_muted),
                ),
        );
        if dialog.letter_open {
            let viewport_h = rem_pixels(rems(8.875)).as_f32();
            let content_h = rem_pixels(rems(1.5)).as_f32() * 26.0;
            let mut choices = div()
                .id("network-letter-options")
                .w((rem_pixels(rems(30.0))
                    .min(window.viewport_size().width - rem_pixels(rems(2.0)))
                    - rem_pixels(rems(8.375)))
                .max(rem_pixels(rems(4.0))))
                .bg(theme.menu_surface)
                .occlude()
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_scroll_wheel(|_, _, cx| cx.stop_propagation())
                .h(rems(9.0))
                .relative()
                .flex()
                .flex_col()
                .border(rems(0.0625))
                .border_color(theme.border)
                .rounded_md();
            let refresh = handle.clone();
            let mut rows = div()
                .id("network-letter-scroll")
                .size_full()
                .overflow_y_scroll()
                .track_scroll(&dialog.letter_scroll)
                .pr(ZED_TRACK_WIDTH)
                .on_scroll_wheel(move |_, _, cx| refresh.update(cx, |_, cx| cx.notify()));
            for (index, character) in ('A'..='Z').rev().enumerate() {
                let value = format!("{character}:");
                let used = drive_letter_used(&value, &dialog.drives);
                let description = dialog
                    .drives
                    .iter()
                    .find(|drive| drive.path.to_ascii_uppercase().starts_with(&value))
                    .map(|drive| {
                        drive.remote_path.clone().unwrap_or_else(|| {
                            if drive.label == "Local Disk" {
                                let mut args = FluentArgs::new();
                                args.set("drive", value.clone());
                                cx.i18n()
                                    .text_with_args(Message::LocalDrive, &args)
                                    .to_string()
                            } else {
                                drive.label.clone()
                            }
                        })
                    });
                let selected = dialog.letter.as_ref() == Some(&value);
                let h = handle.clone();
                rows = rows.child(
                    div()
                        .id(("network-letter-option", index))
                        .h(rems(1.5))
                        .flex_none()
                        .flex()
                        .items_center()
                        .gap(rems(0.5))
                        .px_3()
                        .text_color(if used {
                            theme.icon_disabled
                        } else {
                            theme.text
                        })
                        .when(selected, |row| row.bg(theme.selected))
                        .when(!used, |row| {
                            row.hover(|style| style.bg(theme.control_hover))
                        })
                        .on_click(move |_, _, cx| {
                            h.update(cx, |view, cx| {
                                if let Some(dialog) = &mut view.network_dialog {
                                    if !used && !dialog.busy {
                                        dialog.letter = Some(value.clone());
                                        dialog.letter_open = false;
                                        cx.notify();
                                    }
                                }
                            })
                        })
                        .child(format!("{character}:"))
                        .when_some(description, |row, description| {
                            row.child(
                                div()
                                    .flex_1()
                                    .text_align(TextAlign::Right)
                                    .text_size(rems(0.6875))
                                    .text_ellipsis()
                                    .child(description),
                            )
                        }),
                );
            }
            choices = choices.child(rows).child(render_scrollbar_with_pointer(
                theme,
                handle.clone(),
                ScrollbarAxis::Vertical,
                viewport_h,
                content_h,
                dialog.letter_scroll.offset().y.as_f32().abs(),
                dialog.letter_scroll_drag.is_some(),
                "network-letters".into(),
                None,
                |view, track, viewport, content, position, cx| {
                    if let Some(dialog) = &mut view.network_dialog {
                        dialog.letter_scroll_drag = Some((track, viewport, content));
                        update_letter_scroll(view, position, cx);
                    }
                },
            ));
            let mouse = handle.clone();
            choices = choices.child(
                canvas(
                    |_, _, _| (),
                    move |_, _, window, _| {
                        let mouse_move = mouse.clone();
                        window.on_mouse_event(move |event: &MouseMoveEvent, phase, _, cx| {
                            if phase == DispatchPhase::Capture {
                                mouse_move.update(cx, |view, cx| {
                                    if view
                                        .network_dialog
                                        .as_ref()
                                        .is_some_and(|dialog| dialog.letter_scroll_drag.is_some())
                                    {
                                        if event.dragging() {
                                            update_letter_scroll(view, event.position, cx);
                                        } else if let Some(dialog) = &mut view.network_dialog {
                                            dialog.letter_scroll_drag = None;
                                            cx.notify();
                                        }
                                    }
                                });
                            }
                        });
                        let mouse_up = mouse.clone();
                        window.on_mouse_event(move |event: &MouseUpEvent, phase, _, cx| {
                            if phase == DispatchPhase::Capture && event.button == MouseButton::Left
                            {
                                mouse_up.update(cx, |view, cx| {
                                    if let Some(dialog) = &mut view.network_dialog {
                                        if dialog.letter_scroll_drag.take().is_some() {
                                            cx.notify();
                                        }
                                    }
                                });
                            }
                        });
                    },
                )
                .absolute()
                .size_0(),
            );
            letter = letter.child(
                deferred(
                    anchored()
                        .position_mode(AnchoredPositionMode::Local)
                        .position(point(px(0.0), rem_pixels(rems(2.25))))
                        .snap_to_window_with_margin(rem_pixels(rems(0.5)))
                        .child(choices),
                )
                .with_priority(1),
            );
        }
        body = body
            .child(div().mt(rems(0.75)).child(form_row(
                cx.i18n().text(Message::NetworkLetter),
                letter,
                cx,
            )))
            .child(field(
                cx.i18n().text(Message::NetworkFolder),
                dialog.inputs[0].clone(),
                busy,
                false,
                cx,
            ))
            .child(div().my(rems(0.5)).h(rems(0.0625)).bg(theme.border))
            .child(field(
                cx.i18n().text(Message::NetworkUsername),
                dialog.inputs[1].clone(),
                busy,
                false,
                cx,
            ))
            .child(field(
                cx.i18n().text(Message::NetworkPassword),
                dialog.inputs[2].clone(),
                busy,
                true,
                cx,
            ));
        let h = handle.clone();
        body = body.child(
            div()
                .id("network-reconnect")
                .track_focus(&dialog.controls[1])
                .flex()
                .items_center()
                .gap(rems(0.5))
                .mt(rems(0.5))
                .py_1()
                .rounded_sm()
                .border(rems(0.0625))
                .border_color(if dialog.controls[1].is_focused(window) {
                    theme.accent
                } else {
                    theme.surface.alpha(0.0)
                })
                .on_click(move |_, window, cx| {
                    h.update(cx, |view, cx| {
                        if let Some(dialog) = &mut view.network_dialog {
                            if !dialog.busy {
                                dialog.reconnect = !dialog.reconnect;
                                window.focus(&dialog.controls[1], cx);
                                cx.notify();
                            }
                        }
                    })
                })
                .child(
                    div()
                        .size(rems(1.0))
                        .rounded_sm()
                        .border(rems(0.0625))
                        .border_color(theme.accent)
                        .when(dialog.reconnect, |d| {
                            d.bg(theme.accent).child(
                                svg()
                                    .path("icons/check.svg")
                                    .size(rems(0.875))
                                    .text_color(theme.text_on_accent),
                            )
                        }),
                )
                .child(cx.i18n().text(Message::NetworkReconnect)),
        );
    }
    if dialog.loading && !disconnect {
        body = body.child(
            div()
                .text_color(theme.text_muted)
                .child(cx.i18n().text(Message::NetworkLoading)),
        );
    }
    if busy {
        body = body.child(
            div()
                .text_color(theme.text_muted)
                .child(cx.i18n().text(Message::NetworkWorking)),
        );
    }
    if let Some((key, detail)) = &dialog.error {
        let mut args = FluentArgs::new();
        args.set("detail", detail.clone());
        body = body.child(
            div()
                .text_color(theme.error)
                .child(cx.i18n().text_with_args(*key, &args)),
        );
    }
    let h = handle.clone();
    let close = handle.clone();
    let header = div()
        .id("network-dialog-header")
        .flex()
        .w_full()
        .items_center()
        .gap(rems(0.5))
        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
            h.update(cx, |view, cx| {
                if let Some(dialog) = &mut view.network_dialog {
                    dialog.letter_open = false;
                    cx.notify();
                }
            })
        })
        .child(
            svg()
                .path(if disconnect {
                    "icons/server-off.svg"
                } else {
                    "icons/server.svg"
                })
                .size(rems(1.0))
                .text_color(theme.text_muted),
        )
        .child(
            div()
                .flex_1()
                .text_size(rems(0.875))
                .font_weight(FontWeight::MEDIUM)
                .child(title.clone()),
        )
        .child(
            div()
                .id("network-dialog-close")
                .p_1()
                .rounded_md()
                .hover(|s| s.bg(theme.control_hover))
                .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                    cx.stop_propagation();
                    close.update(cx, |view, cx| view.cancel_network_dialog(window, cx));
                })
                .child(
                    svg()
                        .path("icons/x.svg")
                        .size(rems(1.0))
                        .text_color(theme.text_muted),
                ),
        );
    let cancel = handle.clone();
    let submit = handle.clone();
    let backdrop = handle.clone();
    let modal = Modal::new("network-dialog", title, body)
        .on_backdrop_dismiss(move |_, cx| {
            backdrop.update(cx, |view, cx| {
                if let Some(dialog) = &mut view.network_dialog {
                    if dialog.letter_open {
                        dialog.letter_open = false;
                        cx.notify();
                    }
                }
            })
        })
        .width(rems(30.0))
        .header(header)
        .action(
            action_button("network-cancel")
                .track_focus(&dialog.controls[2])
                .bg(theme.control_hover)
                .rounded_md()
                .text_color(theme.text_muted)
                .border(rems(0.0625))
                .border_color(if dialog.controls[2].is_focused(window) {
                    theme.accent
                } else {
                    theme.border
                })
                .when(!busy, |d| d.hover(|s| s.bg(theme.control_hover)))
                .when(busy, |d| d.opacity(0.5))
                .child(cx.i18n().text(Message::Cancel))
                .on_click(move |_, window, cx| {
                    cancel.update(cx, |view, cx| view.cancel_network_dialog(window, cx))
                }),
        )
        .action(
            action_button("network-submit")
                .track_focus(&dialog.controls[3])
                .rounded_md()
                .bg(theme.accent)
                .text_color(theme.text_on_accent)
                .border(rems(0.0625))
                .border_color(if dialog.controls[3].is_focused(window) {
                    theme.text_on_accent
                } else {
                    theme.accent
                })
                .when(disabled, |d| d.opacity(0.5))
                .when(!disabled, |d| d.hover(|s| s.opacity(0.85)))
                .child(cx.i18n().text(if disconnect {
                    Message::NetworkDisconnect
                } else {
                    Message::NetworkFinish
                }))
                .on_click(move |_, _, cx| {
                    if !disabled {
                        submit.update(cx, |view, cx| view.submit_network_dialog(cx));
                    }
                }),
        );
    let keyboard = handle.clone();
    div()
        .absolute()
        .inset_0()
        .track_focus(&dialog.focus)
        .on_key_down(move |event, window, cx| {
            keyboard.update(cx, |view, cx| {
                let Some(dialog) = &mut view.network_dialog else {
                    return;
                };
                let key = event.keystroke.key.as_str();
                if dialog.busy {
                    cx.stop_propagation();
                    return;
                }
                match key {
                    "escape" => {
                        if dialog.letter_open {
                            dialog.letter_open = false;
                            cx.notify();
                        } else {
                            view.cancel_network_dialog(window, cx);
                        }
                    }
                    "tab" => {
                        let mut order = Vec::new();
                        if !dialog.disconnect {
                            order.push(dialog.controls[0].clone());
                            order.extend(
                                dialog
                                    .inputs
                                    .iter()
                                    .map(|input| input.read(cx).focus_handle.clone()),
                            );
                            order.push(dialog.controls[1].clone());
                        } else {
                            order.extend(dialog.drive_focus.iter().cloned());
                        }
                        order.extend(dialog.controls[2..].iter().cloned());
                        let current = order.iter().position(|focus| focus.is_focused(window));
                        let next = if event.keystroke.modifiers.shift {
                            current.map_or(order.len() - 1, |current| {
                                (current + order.len() - 1) % order.len()
                            })
                        } else {
                            current.map_or(0, |current| (current + 1) % order.len())
                        };
                        window.focus(&order[next], cx);
                    }
                    "enter" => {
                        if dialog.controls[2].is_focused(window) {
                            view.cancel_network_dialog(window, cx);
                        } else if !dialog.disconnect
                            && dialog.controls[0].is_focused(window)
                            && !dialog.busy
                            && !dialog.loading
                        {
                            dialog.letter_open = !dialog.letter_open;
                            cx.notify();
                        } else {
                            view.submit_network_dialog(cx);
                        }
                    }
                    "space" if dialog.controls[2].is_focused(window) => {
                        view.cancel_network_dialog(window, cx)
                    }
                    "space" if dialog.controls[3].is_focused(window) => {
                        view.submit_network_dialog(cx)
                    }
                    "space" | "down" | "up"
                        if dialog.disconnect
                            && !dialog.busy
                            && !dialog.drive_focus.is_empty()
                            && (dialog.focus.is_focused(window)
                                || dialog
                                    .drive_focus
                                    .iter()
                                    .any(|focus| focus.is_focused(window))) =>
                    {
                        let index = dialog
                            .drive_focus
                            .iter()
                            .position(|focus| focus.is_focused(window))
                            .unwrap_or(0);
                        if key == "space" {
                            if let Some(drive) = dialog
                                .drives
                                .iter()
                                .filter(|drive| network_drive(drive))
                                .nth(index)
                            {
                                if !dialog.selected.remove(&drive.path) {
                                    dialog.selected.insert(drive.path.clone());
                                }
                            }
                        } else {
                            let length = dialog.drive_focus.len();
                            let next = if key == "down" {
                                (index + 1) % length
                            } else {
                                (index + length - 1) % length
                            };
                            window.focus(&dialog.drive_focus[next], cx);
                        }
                        cx.notify();
                    }
                    "space" if !dialog.busy && dialog.controls[1].is_focused(window) => {
                        dialog.reconnect = !dialog.reconnect;
                        cx.notify();
                    }
                    "space" | "down" | "up"
                        if !dialog.busy
                            && !dialog.loading
                            && dialog.controls[0].is_focused(window) =>
                    {
                        if key == "space" {
                            dialog.letter_open = !dialog.letter_open;
                        } else {
                            let available: Vec<_> = ('A'..='Z')
                                .rev()
                                .map(|c| format!("{c}:"))
                                .filter(|letter| !drive_letter_used(letter, &dialog.drives))
                                .collect();
                            if !available.is_empty() {
                                let current = available
                                    .iter()
                                    .position(|letter| Some(letter) == dialog.letter.as_ref())
                                    .unwrap_or(0);
                                let next = if key == "down" {
                                    (current + 1) % available.len()
                                } else {
                                    (current + available.len() - 1) % available.len()
                                };
                                dialog.letter = Some(available[next].clone());
                            }
                        }
                        cx.notify();
                    }
                    _ => return,
                }
                cx.stop_propagation();
            })
        })
        .on_action(|_: &crate::ui::app::Ouvrir, _, cx| cx.stop_propagation())
        .on_action(|_: &crate::ui::app::Supprimer, _, cx| cx.stop_propagation())
        .on_action(|_: &crate::ui::app::Renommer, _, cx| cx.stop_propagation())
        .on_action(|_: &crate::ui::app::Copier, _, cx| cx.stop_propagation())
        .on_action(|_: &crate::ui::app::Couper, _, cx| cx.stop_propagation())
        .on_action(|_: &crate::ui::app::Coller, _, cx| cx.stop_propagation())
        .on_action(|_: &crate::ui::app::ToutSelectionner, _, cx| cx.stop_propagation())
        .on_action(|_: &crate::ui::app::Annuler, _, cx| cx.stop_propagation())
        .on_action(|_: &crate::ui::app::Refaire, _, cx| cx.stop_propagation())
        .on_action(|_: &crate::ui::app::Actualiser, _, cx| cx.stop_propagation())
        .child(modal)
}
