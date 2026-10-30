use super::super::{Coller, Copier, Couper, Ouvrir, Renommer, Supprimer};
use super::{general, permissions, property_presentation, shortcut, tabs, PropertiesTab, PropertiesWindow};
use crate::ui::components::tool_window::action_button;
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
    let has_file_system = dialog.drive.as_ref().is_some_and(|drive| drive.file_system.is_some());
    let label_width = general::label_width(presentation.as_ref(), has_file_system, theme, window, cx);
    let compact = window.viewport_size().width < rems(28.0).to_pixels(window.rem_size());
    let mut body = div().flex().flex_col().gap(rems(0.375));
    if let Some(edit) = dialog.permissions.as_ref().filter(|_| matches!(dialog.tab, PropertiesTab::Permissions | PropertiesTab::Contents)) {
        body = permissions::render(body, dialog, edit, dialog.tab == PropertiesTab::Contents, &handle, compact, theme, cx);
    } else if dialog.paths.len() > 1 {
        body = super::selection::render(dialog, label_width, compact, theme, cx);
    } else if let Some(p) = dialog.properties.as_ref().filter(|_| dialog.tab == PropertiesTab::General) {
        body = general::render(
            body,
            view,
            p,
            presentation.as_ref().unwrap(),
            label_width,
            compact,
            theme,
            cx,
        );
    } else if dialog.tab == PropertiesTab::Shortcut {
        body = shortcut::fields(body, dialog, &handle, compact, theme, cx);
    } else if dialog.error.is_none() {
        body = body.child(cx.i18n().text(Message::PropertiesLoading));
    }
    let has_shortcut = dialog
        .properties
        .as_ref()
        .is_some_and(|properties| properties.shortcut.is_some());
    let has_permissions = dialog.permissions.is_some();
    let has_contents = dialog.permissions.as_ref().is_some_and(|edit| edit.offers_enclosed());
    let property_tabs = tabs::render(dialog, &handle, has_shortcut, has_permissions, has_contents, theme, cx);
    if dialog.saving {
        body = body.child(cx.i18n().text(Message::ShortcutSaving));
    }
    if let Some(error) = dialog.validation_error {
        body = body.child(div().text_color(theme.error).child(cx.i18n().text(error)));
    }
    body = shortcut::run_menu(body, dialog, &handle, theme, window, cx);
    if let Some(error) = &dialog.error {
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
    let body = crate::ui::components::scroll_area::scroll_area("properties-body-scroll-state", div(),
        div().id("properties-body")
            .when(!has_shortcut && !has_permissions, |body| body.px(rems(1.0)).py(rems(1.25)))
            .child(body), false, window, cx);
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
                        if dialog.run_menu.take().is_some()
                            || dialog.permissions.as_mut().is_some_and(|edit| edit.menu.take().is_some())
                        {
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
                        if dialog.tab == PropertiesTab::Shortcut {
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
                    window,
                    false,
                    cx,
                    move |window, cx| {
                        title_close.update(cx, |this, cx| this.close_file_properties(window, cx))
                    },
                ))
                .child(
                    body,
                )
                .child(
                    crate::ui::components::tool_window::footer(cx)
                        // Linux has no system Properties dialog; xdg-open would just open the item.
                        .when(cfg!(target_os = "windows"), |footer| footer.child(
                            action_button("properties-system", "".into(), false, cx)
                                .child(
                                    svg()
                                        .path("icons/wrench.svg")
                                        .size(rems(0.875))
                                        .flex_none()
                                        .text_color(theme.text),
                                )
                                .child(cx.i18n().text(Message::PropertiesSystem))
                                .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                                    system.update(cx, |this, cx| {
                                        this.system_file_properties(window, cx)
                                    });
                                }),
                        ))
                        .child(
                            action_button("properties-cancel", cx.i18n().text(Message::Cancel), false, cx)
                                .track_focus(&dialog.focus)
                                .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                                    cancel.update(cx, |this, cx| {
                                        this.close_file_properties(window, cx)
                                    });
                                }),
                        )
                        .child(
                            action_button("properties-ok", cx.i18n().text(Message::TrashSettingsOk), true, cx)
                                .track_focus(&dialog.ok_focus)
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
