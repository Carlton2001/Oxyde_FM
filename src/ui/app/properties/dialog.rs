use super::super::{Coller, Copier, Couper, Ouvrir, OxydeApp, Renommer, Supprimer};
use super::{general, property_presentation, shortcut};
use crate::ui::components::modal::{action_button, Modal};
use crate::ui::i18n::{I18nContext, Message};
use crate::ui::theme::ThemeContext;
use gpui::prelude::FluentBuilder;
use gpui::*;

pub(crate) fn render(
    view: &OxydeApp,
    handle: Entity<OxydeApp>,
    window: &mut Window,
    cx: &mut Context<OxydeApp>,
) -> impl IntoElement {
    let dialog = view.properties_dialog.as_ref().unwrap();
    let theme = *cx.theme();
    let presentation = dialog.properties.as_ref().map(property_presentation);
    let label_width = general::label_width(presentation.as_ref(), theme, window, cx);
    let mut body = div().flex().flex_col().gap(rems(0.375));
    if let Some(p) = dialog.properties.as_ref().filter(|_| !dialog.shortcut_tab) {
        body = general::render(body, view, p, presentation.as_ref().unwrap(), label_width, theme, cx);
    } else if dialog.shortcut_tab {
        body = shortcut::fields(body, dialog, &handle, theme, cx);
    } else if dialog.error.is_none() {
        body = body.child(cx.i18n().text(Message::PreviewLoading));
    }
    let has_shortcut = dialog
        .properties
        .as_ref()
        .is_some_and(|properties| properties.shortcut.is_some());
    let property_tabs = shortcut::tabs(dialog, &handle, has_shortcut, theme, cx);
    if dialog.saving {
        body = body.child(cx.i18n().text(Message::ShortcutSaving));
    }
    if let Some(error) = dialog.validation_error {
        body = body.child(div().text_color(theme.error).child(cx.i18n().text(error)));
    }
    body = shortcut::run_menu(body, dialog, &handle, theme, cx);
    body = body.when_some(dialog.error.as_ref(), |body, error| {
        body.child(
            div()
                .text_color(theme.error)
                .child(cx.i18n().text(Message::OperationFailed))
                .child(error.clone()),
        )
    });
    if let Some(tabs) = property_tabs {
        body = div()
            .flex()
            .flex_col()
            .child(tabs)
            .child(div().px(rems(1.0)).py(rems(1.25)).child(body));
    }
    let keyboard = handle.clone();
    let close = handle.clone();
    let cancel = handle.clone();
    let ok = handle.clone();
    let system = handle.clone();
    let title_close = handle;
    div()
        .absolute()
        .inset_0()
        .track_focus(&dialog.focus)
        .on_key_down(move |event, window, cx| {
            keyboard.update(cx, |this, cx| match event.keystroke.key.as_str() {
                "escape" => {
                    if let Some(dialog) = &mut this.properties_dialog {
                        if dialog.run_menu.take().is_some() {
                            cx.notify();
                            return;
                        }
                    }
                    this.close_file_properties(window, cx);
                }
                "enter" | "space"
                    if this
                        .properties_dialog
                        .as_ref()
                        .is_some_and(|dialog| dialog.run_focus.is_focused(window)) =>
                {
                    if let Some(dialog) = &mut this.properties_dialog {
                        if dialog.run_menu.take().is_none() {
                            dialog.run_menu = dialog
                                .run_bounds
                                .map(|bounds| point(bounds.left(), bounds.bottom()));
                        }
                        cx.notify();
                    }
                }
                "up" | "down"
                    if this
                        .properties_dialog
                        .as_ref()
                        .is_some_and(|dialog| dialog.run_menu.is_some()) =>
                {
                    if let Some(dialog) = &mut this.properties_dialog {
                        let modes = [1, 3, 7];
                        let index = modes
                            .iter()
                            .position(|mode| *mode == dialog.run_window)
                            .unwrap_or(0);
                        dialog.run_window =
                            modes[(index + if event.keystroke.key == "up" { 2 } else { 1 }) % 3];
                        cx.notify();
                    }
                }
                "enter" | "space"
                    if this
                        .properties_dialog
                        .as_ref()
                        .is_some_and(|dialog| dialog.focus.is_focused(window)) =>
                {
                    this.close_file_properties(window, cx)
                }
                "enter" => this.accept_properties(window, cx),
                "space"
                    if this.properties_dialog.as_ref().is_some_and(|dialog| {
                        dialog.focus.is_focused(window) || dialog.ok_focus.is_focused(window)
                    }) =>
                {
                    this.accept_properties(window, cx)
                }
                "tab" => {
                    if let Some(dialog) = &this.properties_dialog {
                        let mut focuses = vec![dialog.focus.clone()];
                        if dialog.shortcut_tab {
                            focuses.extend(
                                dialog
                                    .shortcut_inputs
                                    .iter()
                                    .map(|input| input.read(cx).focus_handle.clone()),
                            );
                            focuses.push(dialog.run_focus.clone());
                        }
                        focuses.push(dialog.ok_focus.clone());
                        let current = focuses
                            .iter()
                            .position(|focus| focus.is_focused(window))
                            .unwrap_or(0);
                        let next = if event.keystroke.modifiers.shift {
                            (current + focuses.len() - 1) % focuses.len()
                        } else {
                            (current + 1) % focuses.len()
                        };
                        window.focus(&focuses[next], cx);
                    }
                }
                _ => {}
            });
            cx.stop_propagation();
        })
        .on_action(|_: &Supprimer, _, cx| cx.stop_propagation())
        .on_action(|_: &Ouvrir, _, cx| cx.stop_propagation())
        .on_action(|_: &Renommer, _, cx| cx.stop_propagation())
        .on_action(|_: &Couper, _, cx| cx.stop_propagation())
        .on_action(|_: &Copier, _, cx| cx.stop_propagation())
        .on_action(|_: &Coller, _, cx| cx.stop_propagation())
        .child(
            Modal::new("file-properties", cx.i18n().text(Message::Properties), body)
                .when(has_shortcut, |modal| {
                    modal.body_padding(rems(0.0), rems(0.0))
                })
                .width(rems(28.0))
                .title_icon("icons/settings-2.svg")
                .header(
                    div()
                        .flex()
                        .items_center()
                        .gap(rems(0.5))
                        .child(
                            svg()
                                .path("icons/settings-2.svg")
                                .size(rems(1.0))
                                .flex_none()
                                .text_color(theme.text_muted),
                        )
                        .child(div().flex_1().child(cx.i18n().text(Message::Properties)))
                        .child(
                            div()
                                .id("properties-close")
                                .child(svg().path("icons/x.svg").size(rems(1.0)))
                                .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                                    cx.stop_propagation();
                                    title_close.update(cx, |this, cx| {
                                        this.close_file_properties(window, cx)
                                    });
                                }),
                        ),
                )
                .on_backdrop_dismiss(move |window, cx| {
                    close.update(cx, |this, cx| this.close_file_properties(window, cx));
                })
                .action(
                    action_button("properties-system")
                        .bg(theme.control_hover)
                        .child(cx.i18n().text(Message::PropertiesSystem))
                        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                            system.update(cx, |this, cx| this.system_file_properties(cx));
                        }),
                )
                .action(
                    action_button("properties-cancel")
                        .track_focus(&dialog.focus)
                        .bg(theme.control_hover)
                        .child(cx.i18n().text(Message::Cancel))
                        .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                            cancel.update(cx, |this, cx| this.close_file_properties(window, cx));
                        }),
                )
                .action(
                    action_button("properties-ok")
                        .track_focus(&dialog.ok_focus)
                        .bg(theme.accent)
                        .text_color(theme.text_on_accent)
                        .child(cx.i18n().text(Message::TrashSettingsOk))
                        .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                            ok.update(cx, |this, cx| this.accept_properties(window, cx));
                        }),
                ),
        )
        .when(dialog.saving, |root| {
            root.child(
                div()
                    .absolute()
                    .inset_0()
                    .on_any_mouse_down(|_, _, cx| cx.stop_propagation()),
            )
        })
}
