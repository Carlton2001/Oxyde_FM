use super::*;
use crate::ui::components::modal::{action_button, Modal};
use crate::ui::theme::ThemeContext;
use crate::ui::elements::button::{button_style, ButtonVariant};

fn toggle(value: bool, theme: crate::ui::theme::OxydeTheme) -> impl IntoElement {
    div()
        .w(rems(1.875))
        .h(rems(1.125))
        .rounded_full()
        .bg(if value { theme.accent } else { theme.border })
        .flex()
        .items_center()
        .flex_none()
        .px(rems(0.125))
        .when(value, |d| d.justify_end())
        .child(
            div()
                .size(rems(0.875))
                .rounded_full()
                .bg(theme.surface)
                .flex_none(),
        )
}


pub(crate) fn render(
    view: &OxydeApp,
    handle: Entity<OxydeApp>,
    window: &mut Window,
    cx: &mut Context<OxydeApp>,
) -> impl IntoElement {
    let dialog = view.trash_settings.as_ref().unwrap();
    let theme = *cx.theme();
    let disabled = dialog.loading || dialog.busy;
    let size = crate::ui::format::format_size(dialog.usage.0, false, false, cx.i18n().language());
    let empty = handle.clone();
    let cancel = handle.clone();
    let save = handle.clone();
    let close = handle.clone();
    let keyboard = handle.clone();
    let title_close = handle.clone();
    let body = render_body(dialog, &handle, empty, size, window, cx);
    div()
        .absolute()
        .inset_0()
        .track_focus(&dialog.focus)
        .on_key_down(move |event, window, cx| {
            // Character input is dispatched after key handlers by GPUI.
            // Only consume the modal shortcuts so TextInput can receive typing.
            if !matches!(event.keystroke.key.as_str(), "escape" | "tab" | "enter" | "space") {
                return;
            }
            keyboard.update(cx, |this, cx| match event.keystroke.key.as_str() {
                "escape" => this.close_trash_settings(window, cx),
                "tab" => {
                    if let Some(dialog) = &this.trash_settings {
                        let input_focus = dialog.input.read(cx).focus_handle.clone();
                        let controls = [&dialog.focus, &input_focus, &dialog.save_focus];
                        let current = controls
                            .iter()
                            .position(|focus| focus.is_focused(window))
                            .unwrap_or(0);
                        let next = if event.keystroke.modifiers.shift {
                            (current + 2) % 3
                        } else {
                            (current + 1) % 3
                        };
                        window.focus(controls[next], cx);
                    }
                }
                "enter" | "space" => {
                    if this
                        .trash_settings
                        .as_ref()
                        .is_some_and(|dialog| dialog.save_focus.is_focused(window))
                    {
                        this.save_trash_settings(cx);
                    } else {
                        this.close_trash_settings(window, cx);
                    }
                }
                _ => {}
            });
            cx.stop_propagation();
        })
        .on_action(|_: &Supprimer, _, cx| cx.stop_propagation())
        .on_action(|_: &Ouvrir, _, cx| cx.stop_propagation())
        .on_action(|_: &Renommer, _, cx| cx.stop_propagation())
        .on_action(|_: &Copier, _, cx| cx.stop_propagation())
        .on_action(|_: &Couper, _, cx| cx.stop_propagation())
        .on_action(|_: &Coller, _, cx| cx.stop_propagation())
        .child(
            Modal::new(
                "trash-settings",
                cx.i18n().text(Message::TrashProperties),
                body,
            )
            .title_icon("icons/trash-2.svg")
            .standalone(false)
            .header_background(theme.surface)
            .body_background(theme.active_background)
            .header(crate::ui::components::tool_window::header(
                "icons/trash-2.svg", cx.i18n().text(Message::TrashProperties),
                window.is_maximized(), false, cx,
                move |window, cx| {
                    title_close.update(cx, |app, cx| app.close_trash_settings(window, cx));
                },
            ))
            .width(rems(28.0))
            .on_backdrop_dismiss(move |window, cx| {
                close.update(cx, |this, cx| this.close_trash_settings(window, cx));
            })
            .action(
                button_style(action_button("trash-settings-cancel"), theme, ButtonVariant::Secondary, true)
                    .track_focus(&dialog.focus)
                    .child(cx.i18n().text(Message::Cancel))
                    .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                        cancel.update(cx, |this, cx| this.close_trash_settings(window, cx));
                    }),
            )
            .action(
                button_style(action_button("trash-settings-save"), theme, ButtonVariant::Primary, !disabled)
                    .track_focus(&dialog.save_focus)

                    .child(cx.i18n().text(Message::TrashSettingsOk))
                    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                        save.update(cx, |this, cx| this.save_trash_settings(cx));
                    }),
            ),
        )
}


fn render_body(
    dialog: &TrashSettings,
    handle: &Entity<OxydeApp>,
    empty: Entity<OxydeApp>,
    size: String,
    window: &Window,
    cx: &mut Context<OxydeApp>,
) -> Div {
    let theme = *cx.theme();
    let disabled = dialog.loading || dialog.busy;
    let section_title = |message: Message, icon: &'static str| {
        div()
            .flex()
            .items_center()
            .gap(rems(0.5))
            .text_size(rems(0.6875))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(theme.text_muted)
            .child(
                svg()
                    .path(icon)
                    .size(rems(0.8125))
                    .flex_none()
                    .text_color(theme.text),
            )
            .child(cx.i18n().text(message).to_uppercase())
    };
    let mut count_args = FluentArgs::new();
    count_args.set("count", dialog.usage.1);
    count_args.set(
        "formatted",
        crate::ui::format::format_number(dialog.usage.1, cx.i18n().language()),
    );
    let mut table = div()
        .rounded_md()
        .border(rems(0.0625))
        .border_color(theme.border)
        .overflow_hidden()
        .flex()
        .flex_col()
        .child(
            div()
                .flex()
                .justify_between()
                .px(rems(0.75))
                .py(rems(0.5))
                .bg(theme.header)
                .rounded_t(rems(0.3125))
                .when(dialog.drives.is_empty(), |header| header.rounded_b(rems(0.3125)))
                .text_size(rems(0.6875))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme.text_muted)
                .child(cx.i18n().text(Message::ColumnLocation).to_uppercase())
                .child(cx.i18n().text(Message::TrashAvailableSpace).to_uppercase()),
        );
    let mut rows = div()
        .id("trash-drive-rows")
        .max_h(rems(8.0))
        .overflow_y_scroll()
        .flex()
        .flex_col();
    for (index, drive) in dialog.drives.iter().enumerate() {
        let h = handle.clone();
        let selected = index == dialog.selected;
        rows = rows.child(
            div()
                .id(("trash-drive", index))
                .when(index + 1 == dialog.drives.len(), |row| row.rounded_b(rems(0.3125)))
                .flex()
                .items_center()
                .gap(rems(0.5))
                .px(rems(0.75))
                .py(rems(0.625))
                .border_t(rems(0.0625))
                .border_color(theme.border)
                .bg(if selected {
                    theme.selected
                } else {
                    theme.surface
                })
                .text_color(if selected { theme.accent } else { theme.text })
                .when(!selected, |row| row.hover(|row| row.bg(theme.hover)))
                .child(
                    svg()
                        .path("icons/hard-drive.svg")
                        .size(rems(0.8125))
                        .flex_none()
                        .text_color(if selected {
                            theme.accent
                        } else {
                            theme.text
                        }),
                )
                .child(div().flex_1().min_w_0().truncate().child(format!(
                    "{} ({})",
                    drive.label,
                    drive.path.trim_end_matches('\\')
                )))
                .child(
                    div()
                        .flex_none()
                        .text_color(if selected {
                            theme.accent
                        } else {
                            theme.text_muted
                        })
                        .child(crate::ui::format::format_size(
                            drive.free_bytes,
                            false,
                            false,
                            cx.i18n().language(),
                        )),
                )
                .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                    h.update(cx, |this, cx| this.select_trash_drive(index, false, cx));
                }),
        );
    }
    table = table.child(rows);
    let mut body = div()
        .flex()
        .flex_col()
        .gap(rems(1.5))
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap(rems(0.75))
                .px(rems(1.25))
                .py(rems(1.0))
                .border(rems(0.0625))
                .border_color(theme.border)
                .rounded_lg()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(rems(0.75))
                        .child(
                            svg()
                                .path("icons/trash-2.svg")
                                .size(rems(1.0))
                                .flex_none()
                                .text_color(theme.accent),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .child(
                                    div()
                                        .text_size(rems(1.125))
                                        .font_weight(FontWeight::BOLD)
                                        .child(size),
                                )
                                .child(
                                    div().text_color(theme.text_muted).child(
                                        cx.i18n()
                                            .text_with_args(Message::TrashItemCount, &count_args),
                                    ),
                                ),
                        ),
                )
                .child(
                    button_style(action_button("trash-settings-empty"), theme, ButtonVariant::Secondary, !disabled && dialog.usage.1 > 0)
                        .flex_none()
                        .opacity(if disabled || dialog.usage.1 == 0 {
                            0.5
                        } else {
                            1.0
                        })
                        .child(cx.i18n().text(Message::TrashDeleteAll))
                        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                            empty.update(cx, |this, cx| this.empty_trash_from_settings(cx));
                        }),
                ),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap(rems(0.5))
                .child(section_title(Message::TrashLocations, "icons/database.svg"))
                .child(table),
        );
    if let Some(config) = dialog.configs.get(dialog.selected) {
        let index = dialog.selected;
        let mb = handle.clone();
        let percent = handle.clone();
        let nuke = handle.clone();
        let card = div()
            .flex()
            .flex_col()
            .gap(rems(1.25))
            .p(rems(1.25))
            .rounded_lg()
            .bg(theme.header)
            .border(rems(0.0625))
            .border_color(theme.border)
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap(rems(0.75))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .gap(rems(0.125))
                            .child(
                                div()
                                    .font_weight(FontWeight::MEDIUM)
                                    .child(cx.i18n().text(Message::TrashCustomSize)),
                            )
                            .child(
                                div()
                                    .text_color(theme.text_muted)
                                    .child(cx.i18n().text(Message::TrashMaxCapacity)),
                            ),
                    )
                    .child(
                        crate::ui::components::input::field_frame(&dialog.input, window, cx)
                            .flex_none()
                            .overflow_hidden()
                            .opacity(if config.nuke_on_delete || disabled {
                                0.5
                            } else {
                                1.0
                            })
                            .child(
                                div()
                                    .w(rems(5.5))
                                    .h_full()
                                    .px(rems(0.625))
                                    .when(!disabled && !config.nuke_on_delete, |d| {
                                        d.child(dialog.input.clone())
                                    })
                                    .when(disabled || config.nuke_on_delete, |d| {
                                        d.flex().items_center().child(dialog.value.clone())
                                    }),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .h_full()
                                    .p(rems(0.125))
                                    .border_l(rems(0.0625))
                                    .border_color(theme.border)
                                    .child(
                                        action_button("trash-unit-mb")
                                            .h_full()
                                            .px(rems(0.5))
                                            .py_0()
                                            .flex()
                                            .items_center()
                                            .text_size(rems(0.6875))
                                            .rounded_sm()
                                            .bg(if !dialog.percent {
                                                theme.accent
                                            } else {
                                                theme.surface
                                            })
                                            .text_color(if !dialog.percent {
                                                theme.text_on_accent
                                            } else {
                                                theme.text_muted
                                            })
                                            .child(cx.i18n().text(Message::TrashMegabytes))
                                            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                                mb.update(cx, |this, cx| {
                                                    this.select_trash_drive(index, false, cx)
                                                });
                                            }),
                                    )
                                    .child(
                                        action_button("trash-unit-percent")
                                            .h_full()
                                            .px(rems(0.5))
                                            .py_0()
                                            .flex()
                                            .items_center()
                                            .text_size(rems(0.6875))
                                            .rounded_sm()
                                            .bg(if dialog.percent {
                                                theme.accent
                                            } else {
                                                theme.surface
                                            })
                                            .text_color(if dialog.percent {
                                                theme.text_on_accent
                                            } else {
                                                theme.text_muted
                                            })
                                            .child("%")
                                            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                                percent.update(cx, |this, cx| {
                                                    this.select_trash_drive(index, true, cx)
                                                });
                                            }),
                                    ),
                            ),
                    ),
            )
            .child(div().h(rems(0.0625)).w_full().bg(theme.border))
            .child(
                div()
                    .id("trash-nuke")
                    .flex()
                    .items_center()
                    .gap(rems(1.0))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .gap(rems(0.125))
                            .child(
                                div()
                                    .font_weight(FontWeight::MEDIUM)
                                    .child(cx.i18n().text(Message::TrashDontMove)),
                            )
                            .child(
                                div()
                                    .text_color(theme.text_muted)
                                    .child(cx.i18n().text(Message::TrashDeleteImmediately)),
                            ),
                    )
                    .child(toggle(config.nuke_on_delete, theme))
                    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                        nuke.update(cx, |this, cx| {
                            if let Some(d) = &mut this.trash_settings {
                                if !d.busy && !d.loading {
                                    d.configs[d.selected].nuke_on_delete =
                                        !d.configs[d.selected].nuke_on_delete;
                                }
                            }
                            cx.notify();
                        });
                    }),
            );
        body = body.child(
            div()
                .flex()
                .flex_col()
                .gap(rems(0.5))
                .child(section_title(
                    Message::TrashSelectedSettings,
                    "icons/settings-2.svg",
                ))
                .child(card),
        );
    }
    let confirm = handle.clone();
    body = body
        .child(
            div()
                .id("trash-confirm")
                .flex()
                .items_center()
                .gap(rems(1.0))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .child(cx.i18n().text(Message::TrashConfirmDelete)),
                )
                .child(toggle(dialog.confirm, theme))
                .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                    confirm.update(cx, |this, cx| {
                        if let Some(d) = &mut this.trash_settings {
                            if !d.busy && !d.loading {
                                d.confirm = !d.confirm;
                            }
                        }
                        cx.notify();
                    });
                }),
        )
        .when_some(dialog.error.as_ref(), |body, error| {
            body.child(
                div()
                    .text_color(theme.error)
                    .child(cx.i18n().text(Message::TrashSettingsError))
                    .child(error.clone()),
            )
        })
        .when(disabled, |body| {
            body.child(cx.i18n().text(Message::PreviewLoading))
        });
    body
}
