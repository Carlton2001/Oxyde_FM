mod letters;
use crate::ui::app::{
    network::{drive_letter_used, network_drive},
    OxydeApp,
};
use crate::ui::components::{
    confirmation::{self, Tone},
    dialog,
    input::TextInput,
    modal::{form_row, Modal},
};
use crate::ui::i18n::{FluentArgs, I18nContext, Message};
use crate::ui::scale::rem_pixels;
use crate::ui::theme::ThemeContext;
use gpui::prelude::*;
use gpui::*;
use crate::ui::elements::button::ButtonVariant;

pub(crate) fn render_force_confirmation(
    view: &OxydeApp,
    handle: Entity<OxydeApp>,
    _window: &mut Window,
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
    // Cancel holds the keyboard first: its focus is the prompt's.
    div()
        .absolute()
        .inset_0()
        .on_key_down(move |event, window, cx| {
            keyboard.update(cx, |view, cx| match event.keystroke.key.as_str() {
                "escape" => view.resolve_network_force(false, window, cx),
                "tab" => {
                    if let Some(confirmation) = &view.network_force_confirmation {
                        let order = [confirmation.focus.clone(), confirmation.confirm_focus.clone()];
                        dialog::cycle(&order, event.keystroke.modifiers.shift, window, cx);
                    }
                }
                _ => {}
            });
            cx.stop_propagation();
        })
        .child(
            Modal::new(
                "network-force-confirmation",
                cx.i18n().text(Message::DisconnectThisNetworkDrive),
                confirmation::body(
                    "icons/triangle-alert.svg",
                    Tone::Danger,
                    cx.i18n().text_with_args(Message::NetworkForcePrompt, &args),
                    Vec::new(),
                    None,
                    theme,
                ),
            )
            .title_icon("icons/triangle-alert.svg")
            .on_close(move |window, cx| {
                close.update(cx, |view, cx| view.resolve_network_force(false, window, cx));
            })
            .action(
                dialog::button("network-force-cancel", cx.i18n().text(Message::Cancel), ButtonVariant::Secondary)
                    .focus(&confirmation.focus)
                    .on_press(move |window, cx| {
                        cancel.update(cx, |view, cx| view.resolve_network_force(false, window, cx));
                    }),
            )
            .action(
                dialog::button("network-force-accept", cx.i18n().text(Message::NetworkForceAction), Tone::Danger.variant())
                    .focus(&confirmation.confirm_focus)
                    .on_press(move |window, cx| {
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
    let track_size = crate::ui::components::select_menu::scrollbar_track_size(*viewport);
    let offset = crate::ui::components::scrollbar::drag_scrolled(
        *viewport, *content, track_size, position.y.as_f32() - origin.y, view.grab_offset);
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
        crate::ui::components::input::read_only_field(
            if secret {
                "*".repeat(value.chars().count())
            } else {
                value.to_string()
            },
            cx,
        )
        .into_any_element()
    } else {
        input.into_any_element()
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
                            crate::ui::elements::checkbox::checkbox(theme, selected, false),
                        )
                        .child(
                            svg()
                                .path("icons/hard-drive.svg")
                                .size(rems(1.125))
                                .text_color(theme.text),
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
        let letter_bounds = handle.clone();
        let mut letter = div().relative().flex().flex_col().child(
            crate::ui::components::select_menu::trigger(
                "network-letter",
                dialog.letter.clone().map(SharedString::from).unwrap_or_else(|| cx.i18n().text(Message::NetworkNoLetter)),
                dialog.controls[0].is_focused(window),
                !busy && !dialog.loading,
                cx,
            )
                .child(canvas(|bounds, _, _| bounds, move |bounds, _, window, cx| {
                    let bounds = crate::ui::components::popup::owner_bounds(window, cx, bounds);
                    letter_bounds.update(cx, |view, cx| {
                        if let Some(dialog) = &mut view.network_dialog {
                            if dialog.letter_bounds != Some(bounds) { dialog.letter_bounds = Some(bounds); if dialog.letter_open { cx.notify(); } }
                        }
                    });
                }).absolute().inset_0().size_full())
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .track_focus(&dialog.controls[0])
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
                }),
        );
        if dialog.letter_open {
            let native = crate::ui::components::popup::supported(window, cx);
            if native {
                let bounds = dialog.letter_bounds.unwrap_or_default();
                crate::ui::components::popup::show("network-letters", crate::ui::components::popup::PopupAnchor::element(bounds), handle.clone(), window, cx,
                    |view| view.network_dialog.as_ref().is_some_and(|dialog| dialog.letter_open && !dialog.disconnect),
                    |view, _, cx| letters::render(view, cx.entity(), cx),
                    |view, _, cx| { if let Some(dialog) = &mut view.network_dialog { dialog.letter_open = false; dialog.letter_scroll_drag = None; cx.notify(); } });
            }
            if !native {
            let choices = letters::render(view, handle.clone(), cx);
            letter = letter.child(
                deferred(
                    anchored()
                        .position_mode(AnchoredPositionMode::Local)
                        .position(point(px(0.0), rem_pixels(rems(2.25) + crate::ui::components::select_menu::LIST_GAP)))
                        .snap_to_window_with_margin(rem_pixels(rems(0.5)))
                        .child(choices),
                )
                .with_priority(1),
            );
        
            }
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
            .child(crate::ui::elements::separator::horizontal(theme.border))
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
                .child(crate::ui::elements::focus_ring::focus_ring(
                    crate::ui::elements::checkbox::checkbox(theme, dialog.reconnect, false),
                    crate::ui::elements::focus_ring::keyboard_focused(&dialog.controls[1], window),
                    false,
                    theme,
                ))
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
        body = body.child(crate::ui::components::modal::field_error(cx.i18n().text_with_args(*key, &args), theme));
    }
    let cancel = handle.clone();
    let submit = handle.clone();
    let close = handle.clone();
    let modal = Modal::new("network-dialog", title, body)
        .title_icon(if disconnect { "icons/server-off.svg" } else { "icons/server.svg" })
        .on_close(move |window, cx| close.update(cx, |view, cx| view.cancel_network_dialog(window, cx)))
        .action(
            dialog::button("network-cancel", cx.i18n().text(Message::Cancel), ButtonVariant::Secondary)
                .focus(&dialog.controls[2])
                .enabled(!busy)
                .on_press(move |window, cx| cancel.update(cx, |view, cx| view.cancel_network_dialog(window, cx))),
        )
        .action(
            dialog::button(
                "network-submit",
                cx.i18n().text(if disconnect { Message::NetworkDisconnect } else { Message::NetworkFinish }),
                ButtonVariant::Primary,
            )
            .focus(&dialog.controls[3])
            .enabled(!disabled)
            .on_press(move |_, cx| submit.update(cx, |view, cx| view.submit_network_dialog(cx))),
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
                        dialog::cycle(&order, event.keystroke.modifiers.shift, window, cx);
                    }
                    "enter" => {
                        if !dialog.disconnect
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
        .child(modal)
}
