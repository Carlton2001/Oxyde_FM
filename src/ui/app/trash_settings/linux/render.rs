use super::*;
use crate::ui::components::modal::{action_button, Modal};
use crate::ui::elements::button::{button_style, ButtonVariant};
use crate::ui::elements::toggle::toggle;
use crate::ui::theme::ThemeContext;
use oxyde_lib::platform::KdeLimitAction;

pub(crate) fn render(
    view: &OxydeApp,
    handle: Entity<OxydeApp>,
    window: &mut Window,
    cx: &mut Context<OxydeApp>,
) -> impl IntoElement {
    let dialog = view.trash_settings.as_ref().unwrap();
    let theme = *cx.theme();
    let disabled = dialog.loading || dialog.busy;
    let body = render_body(dialog, &handle, window, cx);
    let (cancel, save, close, keyboard, title_close) =
        (handle.clone(), handle.clone(), handle.clone(), handle.clone(), handle);
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
                        let controls = focus_order(dialog, cx);
                        let current = controls.iter().position(|focus| focus.is_focused(window)).unwrap_or(0);
                        let next = if event.keystroke.modifiers.shift {
                            (current + controls.len() - 1) % controls.len()
                        } else {
                            (current + 1) % controls.len()
                        };
                        window.focus(&controls[next], cx);
                    }
                }
                "enter" | "space" => {
                    if this.trash_settings.as_ref().is_some_and(|dialog| dialog.save_focus.is_focused(window)) {
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
            Modal::new("trash-settings", cx.i18n().text(Message::TrashProperties), body)
                .title_icon("icons/trash-2.svg")
                .standalone(false)
                .header_background(theme.surface)
                .body_background(theme.active_background)
                .header(crate::ui::components::tool_window::header(
                    "icons/trash-2.svg", cx.i18n().text(Message::TrashProperties),
                    window, false, cx,
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

/// Cancel, the enabled value inputs, then OK.
fn focus_order(dialog: &TrashSettings, cx: &App) -> Vec<FocusHandle> {
    let days = dialog.days.read(cx).focus_handle.clone();
    let percent = dialog.percent.read(cx).focus_handle.clone();
    let mut controls = vec![dialog.focus.clone()];
    match &dialog.retention {
        TrashRetention::Gnome { enabled: true, .. } => controls.push(days),
        TrashRetention::Kde(limits) => if let Some(limits) = limits.get(dialog.selected) {
            if limits.use_time_limit { controls.push(days); }
            if limits.use_size_limit { controls.push(percent); }
        },
        _ => {}
    }
    controls.push(dialog.save_focus.clone());
    controls
}

fn section_title(label: SharedString, icon: &'static str, theme: crate::ui::theme::OxydeTheme) -> Div {
    div()
        .flex()
        .items_center()
        .gap(rems(0.5))
        .text_size(rems(0.6875))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(theme.text_muted)
        .child(svg().path(icon).size(rems(0.8125)).flex_none().text_color(theme.text))
        .child(label.to_uppercase())
}

fn section(label: SharedString, icon: &'static str, content: impl IntoElement, theme: crate::ui::theme::OxydeTheme) -> Div {
    div().flex().flex_col().gap(rems(0.5)).child(section_title(label, icon, theme)).child(content)
}

fn card(theme: crate::ui::theme::OxydeTheme) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(rems(1.0))
        .p(rems(1.25))
        .rounded_lg()
        .bg(theme.header)
        .border(rems(0.0625))
        .border_color(theme.border)
}

fn divider(theme: crate::ui::theme::OxydeTheme) -> Div {
    div().h(rems(0.0625)).w_full().bg(theme.border)
}

/// A labelled row whose whole surface toggles the setting, like the Windows options.
fn toggle_row(
    id: &'static str,
    label: SharedString,
    value: bool,
    theme: crate::ui::theme::OxydeTheme,
    handle: &Entity<OxydeApp>,
    edit: impl Fn(&mut TrashRetention, usize) + Copy + 'static,
) -> Stateful<Div> {
    let handle = handle.clone();
    div()
        .id(id)
        .flex()
        .items_center()
        .gap(rems(1.0))
        .child(div().flex_1().min_w_0().font_weight(FontWeight::MEDIUM).child(label))
        .child(toggle(value, theme))
        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
            handle.update(cx, |this, cx| this.edit_trash_retention(edit, cx));
        })
}

fn number_row(
    label: SharedString,
    input: &Entity<TextInput>,
    value: &str,
    enabled: bool,
    theme: crate::ui::theme::OxydeTheme,
    window: &mut Window,
    cx: &mut Context<OxydeApp>,
) -> Div {
    div()
        .flex()
        .items_center()
        .justify_between()
        .gap(rems(0.75))
        .child(div().flex_1().min_w_0().text_color(if enabled { theme.text } else { theme.text_muted }).child(label))
        .child(
            crate::ui::components::input::field_frame(input, window, cx)
                .flex_none()
                .overflow_hidden()
                .opacity(if enabled { 1.0 } else { 0.5 })
                .child(
                    div()
                        .w(rems(4.5))
                        .h_full()
                        .px(rems(0.625))
                        .when(enabled, |field| field.child(input.clone()))
                        .when(!enabled, |field| field.flex().items_center().child(value.to_string())),
                ),
        )
}

fn note(label: SharedString, theme: crate::ui::theme::OxydeTheme) -> Div {
    div().text_size(rems(0.75)).text_color(theme.text_muted).child(label)
}

fn render_body(
    dialog: &TrashSettings,
    handle: &Entity<OxydeApp>,
    window: &mut Window,
    cx: &mut Context<OxydeApp>,
) -> Div {
    let theme = *cx.theme();
    let disabled = dialog.loading || dialog.busy;
    let size = crate::ui::format::format_size(dialog.usage.0, false, false, cx.i18n().language());
    let mut count_args = FluentArgs::new();
    count_args.set("count", dialog.usage.1);
    count_args.set("formatted", crate::ui::format::format_number(dialog.usage.1, cx.i18n().language()));
    let empty = handle.clone();
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
                        .child(svg().path("icons/trash-2.svg").size(rems(1.0)).flex_none().text_color(theme.accent))
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .child(div().text_size(rems(1.125)).font_weight(FontWeight::BOLD).child(size))
                                .child(div().text_color(theme.text_muted)
                                    .child(cx.i18n().text_with_args(Message::TrashItemCount, &count_args))),
                        ),
                )
                .child(
                    button_style(action_button("trash-settings-empty"), theme, ButtonVariant::Secondary, !disabled && dialog.usage.1 > 0)
                        .flex_none()
                        .opacity(if disabled || dialog.usage.1 == 0 { 0.5 } else { 1.0 })
                        .child(cx.i18n().text(Message::TrashDeleteAll))
                        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                            empty.update(cx, |this, cx| this.empty_trash_from_settings(cx));
                        }),
                ),
        );
    match &dialog.retention {
        TrashRetention::Gnome { enabled, .. } => {
            let content = card(theme)
                .child(toggle_row("trash-gnome-auto-delete", cx.i18n().text(Message::TrashAutoDeleteOld),
                    *enabled, theme, handle, |retention, _| {
                        if let TrashRetention::Gnome { enabled, .. } = retention { *enabled = !*enabled; }
                    }))
                .child(number_row(cx.i18n().text(Message::TrashRetentionDays), &dialog.days,
                    &dialog.days_value, *enabled && !disabled, theme, window, cx))
                .child(divider(theme))
                .child(note(cx.i18n().text(Message::TrashGnomeRetentionNote), theme));
            body = body.child(section(cx.i18n().text(Message::TrashAutoDelete), "icons/settings-2.svg", content, theme));
        }
        TrashRetention::Kde(limits) => {
            body = body.child(section(cx.i18n().text(Message::TrashLocations), "icons/database.svg",
                render_locations(dialog, limits, handle, window, cx), theme));
            if let Some(selected) = limits.get(dialog.selected) {
                let content = card(theme)
                    .child(toggle_row("trash-kde-time-limit", cx.i18n().text(Message::TrashAutoDeleteOld),
                        selected.use_time_limit, theme, handle, |retention, index| {
                            if let TrashRetention::Kde(limits) = retention {
                                if let Some(limits) = limits.get_mut(index) { limits.use_time_limit = !limits.use_time_limit; }
                            }
                        }))
                    .child(number_row(cx.i18n().text(Message::TrashKdeTimeLimit), &dialog.days,
                        &dialog.days_value, selected.use_time_limit && !disabled, theme, window, cx))
                    .child(divider(theme))
                    .child(toggle_row("trash-kde-size-limit", cx.i18n().text(Message::TrashCustomSize),
                        selected.use_size_limit, theme, handle, |retention, index| {
                            if let TrashRetention::Kde(limits) = retention {
                                if let Some(limits) = limits.get_mut(index) { limits.use_size_limit = !limits.use_size_limit; }
                            }
                        }))
                    .child(number_row(cx.i18n().text(Message::TrashKdeSizeLimit), &dialog.percent,
                        &dialog.percent_value, selected.use_size_limit && !disabled, theme, window, cx))
                    .when(selected.use_size_limit, |content| content.child(render_limit_action(selected.action, handle, theme, cx)))
                    .child(divider(theme))
                    .child(note(cx.i18n().text(Message::TrashKdeRetentionNote), theme));
                body = body.child(section(cx.i18n().text(Message::TrashSelectedSettings), "icons/settings-2.svg", content, theme));
            }
        }
        TrashRetention::Unavailable => {}
    }
    let confirm = handle.clone();
    body.child(
            div()
                .id("trash-confirm")
                .flex()
                .items_center()
                .gap(rems(1.0))
                .child(div().flex_1().min_w_0().child(cx.i18n().text(Message::TrashConfirmDelete)))
                .child(toggle(dialog.confirm, theme))
                .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                    confirm.update(cx, |this, cx| {
                        if let Some(dialog) = this.trash_settings.as_mut().filter(|dialog| !dialog.busy && !dialog.loading) {
                            dialog.confirm = !dialog.confirm;
                        }
                        cx.notify();
                    });
                }),
        )
        .when_some(dialog.error.as_ref(), |body, error| {
            body.child(div().text_color(theme.error)
                .child(cx.i18n().text(Message::TrashRetentionError))
                .child(error.clone()))
        })
        .when(disabled, |body| body.child(cx.i18n().text(Message::PreviewLoading)))
}

/// Each KDE trash directory carries its own limits, like Windows drives.
fn render_locations(
    dialog: &TrashSettings,
    limits: &[oxyde_lib::platform::KdeTrashLimits],
    handle: &Entity<OxydeApp>,
    window: &mut Window,
    cx: &mut Context<OxydeApp>,
) -> Div {
    let theme = *cx.theme();
    let mut rows = div().id("trash-location-rows").flex().flex_col();
    for (index, location) in limits.iter().enumerate() {
        let selected = index == dialog.selected;
        let select = handle.clone();
        rows = rows.child(
            div()
                .id(("trash-location", index))
                .when(index + 1 == limits.len(), |row| row.rounded_b(rems(0.3125)))
                .flex()
                .items_center()
                .gap(rems(0.5))
                .px(rems(0.75))
                .py(rems(0.625))
                .border_t(rems(0.0625))
                .border_color(theme.border)
                .bg(if selected { theme.selected } else { theme.surface })
                .text_color(if selected { theme.accent } else { theme.text })
                .when(!selected, |row| row.hover(|row| row.bg(theme.hover)))
                .child(svg().path("icons/hard-drive.svg").size(rems(0.8125)).flex_none()
                    .text_color(if selected { theme.accent } else { theme.text }))
                .child(div().flex_1().min_w_0().truncate().child(location.trash.to_string_lossy().into_owned()))
                .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                    select.update(cx, |this, cx| this.select_trash_location(index, cx));
                }),
        );
    }
    div()
        .rounded_md()
        .border(rems(0.0625))
        .border_color(theme.border)
        .overflow_hidden()
        .flex()
        .flex_col()
        .child(
            div()
                .px(rems(0.75))
                .py(rems(0.5))
                .bg(theme.header)
                .rounded_t(rems(0.3125))
                .text_size(rems(0.6875))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme.text_muted)
                .child(cx.i18n().text(Message::ColumnLocation).to_uppercase()),
        )
        .child(crate::ui::components::scroll_area::scroll_area("trash-location-scroll-state",
            div().max_h(rems(8.0)), rows, false, window, cx))
}

fn render_limit_action(
    current: KdeLimitAction,
    handle: &Entity<OxydeApp>,
    theme: crate::ui::theme::OxydeTheme,
    cx: &mut Context<OxydeApp>,
) -> Div {
    let mut choices = div()
        .flex()
        .flex_wrap()
        .gap(rems(0.25))
        .p(rems(0.125))
        .rounded_md()
        .border(rems(0.0625))
        .border_color(theme.border);
    for (index, (action, label)) in [
        (KdeLimitAction::Warn, Message::TrashKdeWarn),
        (KdeLimitAction::DeleteOldest, Message::TrashKdeDeleteOldest),
        (KdeLimitAction::DeleteLargest, Message::TrashKdeDeleteLargest),
    ].into_iter().enumerate() {
        let selected = action == current;
        let choose = handle.clone();
        choices = choices.child(
            action_button(("trash-kde-limit-action", index))
                .px(rems(0.5))
                .py(rems(0.25))
                .text_size(rems(0.75))
                .rounded_sm()
                .bg(if selected { theme.accent } else { theme.surface })
                .text_color(if selected { theme.text_on_accent } else { theme.text_muted })
                .child(cx.i18n().text(label))
                .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                    choose.update(cx, |this, cx| this.edit_trash_retention(move |retention, selected| {
                        if let TrashRetention::Kde(limits) = retention {
                            if let Some(limits) = limits.get_mut(selected) { limits.action = action; }
                        }
                    }, cx));
                }),
        );
    }
    div()
        .flex()
        .flex_col()
        .gap(rems(0.5))
        .child(div().text_color(theme.text).child(cx.i18n().text(Message::TrashKdeLimitAction)))
        .child(choices)
}
