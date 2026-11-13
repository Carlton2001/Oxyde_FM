use super::*;
use crate::ui::components::modal::Modal;
use crate::ui::theme::ThemeContext;
use crate::ui::elements::button::ButtonVariant;

use crate::ui::elements::toggle::toggle;

pub(crate) fn render(
    view: &OxydeApp,
    handle: Entity<OxydeApp>,
    window: &mut Window,
    cx: &mut Context<OxydeApp>,
) -> impl IntoElement {
    let dialog = view.trash_settings.as_ref().unwrap();
    let disabled = dialog.loading || dialog.busy;
    let size = crate::ui::format::format_size(dialog.usage.0, false, false, cx.i18n().language());
    let empty = handle.clone();
    let cancel = handle.clone();
    let save = handle.clone();
    let keyboard = handle.clone();
    let title_close = handle.clone();
    let body = render_body(dialog, &handle, empty, size, window, cx);
    // Cancel holds the dialog's first focus.
    div()
        .absolute()
        .inset_0()
        .on_key_down(move |event, window, cx| {
            // Character input is dispatched after key handlers by GPUI.
            // Only consume the modal shortcuts so TextInput can receive typing.
            if !matches!(event.keystroke.key.as_str(), "escape" | "tab" | "enter") {
                return;
            }
            keyboard.update(cx, |this, cx| match event.keystroke.key.as_str() {
                "escape" => this.close_trash_settings(window, cx),
                "tab" => {
                    if let Some(dialog) = &this.trash_settings {
                        let order = [dialog.focus.clone(), dialog.input.read(cx).focus_handle.clone(), dialog.save_focus.clone()];
                        crate::ui::components::dialog::cycle(&order, event.keystroke.modifiers.shift, window, cx);
                    }
                }
                // Enter in a field saves, as the default button.
                "enter" => {
                    if this.trash_settings.as_ref().is_some_and(|dialog| !dialog.loading && !dialog.busy) {
                        this.save_trash_settings(cx);
                    }
                }
                _ => {}
            });
            cx.stop_propagation();
        })
        .child(
            Modal::new("trash-settings", cx.i18n().text(Message::TrashProperties), body)
                .title_icon("icons/trash-2.svg")
                .header(crate::ui::components::tool_window::header(
                    "icons/trash-2.svg", cx.i18n().text(Message::TrashProperties),
                    window, false, cx,
                    move |window, cx| {
                        title_close.update(cx, |app, cx| app.close_trash_settings(window, cx));
                    },
                ))
                .action(
                    crate::ui::components::dialog::button("trash-settings-cancel", cx.i18n().text(Message::Cancel), ButtonVariant::Secondary)
                        .focus(&dialog.focus)
                        .on_press(move |window, cx| {
                            cancel.update(cx, |this, cx| this.close_trash_settings(window, cx));
                        }),
                )
                .action(
                    crate::ui::components::dialog::button("trash-settings-save", cx.i18n().text(Message::TrashSettingsOk), ButtonVariant::Primary)
                        .focus(&dialog.save_focus)
                        .enabled(!disabled)
                        .on_press(move |_, cx| {
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
    window: &mut Window,
    cx: &mut Context<OxydeApp>,
) -> Div {
    let theme = *cx.theme();
    let disabled = dialog.loading || dialog.busy;
    let section_title = |label: SharedString, icon: &'static str| {
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
            .child(label.to_uppercase())
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
                .child(div().flex_1().min_w_0().truncate().child(if cfg!(target_os = "windows") {
                    format!("{} ({})", drive.label, drive.path.trim_end_matches('\\'))
                } else {
                    // Linux file managers name volumes alone.
                    crate::ui::format::drive_name(drive, cx.i18n())
                }))
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
    table = table.child(crate::ui::components::scroll_area::scroll_area("trash-drive-scroll-state",
        div().max_h(rems(8.0)), rows, false, window, cx));
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
                    crate::ui::components::dialog::button("trash-settings-empty", cx.i18n().text(Message::TrashDeleteAll), ButtonVariant::Secondary)
                        .enabled(!disabled && dialog.usage.1 > 0)
                        .on_press(move |_, cx| {
                            empty.update(cx, |this, cx| this.empty_trash_from_settings(cx));
                        }),
                ),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap(rems(0.5))
                .child(section_title(cx.i18n().text(Message::TrashLocations), "icons/database.svg"))
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
                                    .px(crate::ui::components::input::FIELD_PADDING)
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
                                    .cursor(CursorStyle::Arrow)
                                    .child(
                                        div().id("trash-unit-mb")
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
                                        div().id("trash-unit-percent")
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
            .child(crate::ui::elements::separator::horizontal(theme.border))
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
                    .child(toggle("switch", config.nuke_on_delete, theme))
                    .on_click(move |_, _, cx| {
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
                    cx.i18n().text(Message::TrashSelectedSettings),
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
                .child(toggle("switch", dialog.confirm, theme))
                .on_click(move |_, _, cx| {
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
