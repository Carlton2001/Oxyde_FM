use super::super::{Coller, Copier, Couper, Ouvrir, Renommer, Supprimer};
use super::{general, property_presentation, shortcut, PropertiesWindow};
use crate::ui::components::modal::action_button;
use crate::ui::i18n::{I18nContext, Message};
use crate::ui::theme::ThemeContext;
use gpui::prelude::FluentBuilder;
use gpui::*;

pub(crate) fn render(
    view: &PropertiesWindow,
    handle: Entity<PropertiesWindow>,
    window: &mut Window,
    cx: &mut Context<PropertiesWindow>,
) -> impl IntoElement {
    let dialog = view.properties_dialog.as_ref().unwrap();
    let theme = *cx.theme();
    let height_limit = crate::ui::components::tool_window::content_height_limit(window, cx);
    let presentation = dialog.properties.as_ref().map(property_presentation);
    let label_width = general::label_width(presentation.as_ref(), theme, window, cx);
    let mut body = div().flex().flex_col().gap(rems(0.375));
    if dialog.paths.len() > 1 {
        body = super::selection::render(dialog, label_width, theme, cx);
    } else if let Some(p) = dialog.properties.as_ref().filter(|_| !dialog.shortcut_tab) {
        body = general::render(
            body,
            view,
            p,
            presentation.as_ref().unwrap(),
            label_width,
            theme,
            cx,
        );
    } else if dialog.shortcut_tab {
        body = shortcut::fields(body, dialog, &handle, theme, cx);
    } else if dialog.error.is_none() {
        body = body.child(cx.i18n().text(Message::PropertiesLoading));
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
    body = shortcut::run_menu(body, dialog, &handle, theme, window, cx);
    for error in dialog.error.iter().chain(dialog.system_properties_error.iter()) {
        body = body.child(
            div()
                .text_color(theme.error)
                .child(cx.i18n().text(Message::OperationFailed))
                .child(error.clone()),
        );
    }
    if let Some(tabs) = property_tabs {
        body = div()
            .flex()
            .flex_col()
            .child(tabs)
            .child(div().px(rems(1.0)).py(rems(1.25)).child(body));
    }
    let keyboard = handle.clone();
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
                    if !this.context_menus.is_empty() {
                        this.context_menus.clear();
                        cx.notify();
                        return;
                    }
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
            div()
                .relative()
                .w_full()
                .max_h(height_limit)
                .flex()
                .flex_col()
                .child(crate::ui::components::tool_window::header(
                    "icons/settings-2.svg",
                    cx.i18n().text(Message::Properties),
                    window.is_maximized(),
                    false,
                    cx,
                    move |window, cx| {
                        title_close.update(cx, |this, cx| this.close_file_properties(window, cx))
                    },
                ))
                .child(
                    div()
                        .id("properties-body")
                        .flex_shrink(1.0)
                        .min_h_0()
                        .overflow_y_scroll()
                        .when(!has_shortcut, |body| body.px(rems(1.0)).py(rems(1.25)))
                        .child(body),
                )
                .child(
                    div()
                        .flex()
                        .justify_end()
                        .gap(rems(0.5))
                        .px(rems(1.0))
                        .py(rems(0.625))
                        .border_t(rems(0.0625))
                        .border_color(theme.border)
                        .child(
                            action_button("properties-system")
                                .when(dialog.system_properties_pending, |button| button.opacity(0.5))
                                .bg(theme.control_hover)
                                .child(cx.i18n().text(Message::PropertiesSystem))
                                .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                                    system.update(cx, |this, cx| {
                                        this.system_file_properties(window, cx)
                                    });
                                }),
                        )
                        .child(
                            action_button("properties-cancel")
                                .track_focus(&dialog.focus)
                                .bg(theme.control_hover)
                                .child(cx.i18n().text(Message::Cancel))
                                .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                                    cancel.update(cx, |this, cx| {
                                        this.close_file_properties(window, cx)
                                    });
                                }),
                        )
                        .child(
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
                .child(canvas(|bounds, _, _| bounds, |_, bounds, window, cx| {
                    crate::ui::components::tool_window::fit_content_height(bounds.size.height, window, cx);
                }).absolute().size_full()),
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
