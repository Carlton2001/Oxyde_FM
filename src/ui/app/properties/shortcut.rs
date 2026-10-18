use super::PropertiesWindow;
use super::{general, PropertiesDialog};
use crate::ui::i18n::{I18nContext, Message};
use crate::ui::theme::OxydeTheme;
use gpui::prelude::FluentBuilder;
use gpui::*;
use crate::ui::theme::ThemeContext;

pub(super) fn fields(
    mut body: Div,
    dialog: &PropertiesDialog,
    handle: &Entity<PropertiesWindow>,
    theme: OxydeTheme,
    cx: &mut Context<PropertiesWindow>,
) -> Div {
    for (index, message) in [
        Message::ShortcutTarget,
        Message::ShortcutArguments,
        Message::ShortcutWorkingDirectory,
        Message::ShortcutComment,
    ]
    .into_iter()
    .enumerate()
    {
        body = body.child(crate::ui::components::modal::form_row_with_label_width(
            cx.i18n().text(message),
            div()
                .flex_1()
                .min_w_0()
                .child(dialog.shortcut_inputs[index].clone()),
            rems(6.0),
            cx,
        ));
    }
    let picker = handle.clone();
    let measure = handle.clone();
    body =
        body.child(general::divider(theme))
            .child(crate::ui::components::modal::form_row_with_label_width(
                cx.i18n().text(Message::ShortcutRun),
                div()
                    .id("shortcut-run")
                    .relative()
                    .track_focus(&dialog.run_focus)
                    .flex_1()
                    .min_w_0()
                    .h(rems(2.0))
                    .px(rems(0.625))
                    .flex()
                    .items_center()
                    .justify_between()
                    .border(rems(0.0625))
                    .border_color(theme.border)
                    .bg(theme.surface)
                    .rounded_sm()
                    .child(
                        canvas(
                            |bounds, _, _| bounds,
                            move |bounds, _, _, cx| {
                                measure.update(cx, |this, _| {
                                    if let Some(dialog) = &mut this.properties_dialog {
                                        dialog.run_bounds = Some(bounds);
                                    }
                                })
                            },
                        )
                        .absolute()
                        .inset_0()
                        .size_full(),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .child(cx.i18n().text(run_window_message(dialog.run_window))),
                    )
                    .child(
                        svg()
                            .path("icons/chevron-down.svg")
                            .size(rems(0.75))
                            .flex_none()
                            .text_color(theme.text),
                    )
                    .on_mouse_down(MouseButton::Left, move |event, window, cx| {
                        picker.update(cx, |this, cx| {
                            if let Some(dialog) = &mut this.properties_dialog {
                                if !dialog.saving {
                                    window.focus(&dialog.run_focus, cx);
                                    dialog.run_menu = dialog.run_menu.is_none().then_some(
                                        dialog
                                            .run_bounds
                                            .map(|bounds| point(bounds.left(), bounds.bottom()))
                                            .unwrap_or(event.position),
                                    );
                                    cx.notify();
                                }
                            }
                        });
                    }),
                rems(6.0),
                cx,
            ));
    body
}

pub(super) fn tabs(
    dialog: &PropertiesDialog,
    handle: &Entity<PropertiesWindow>,
    has_shortcut: bool,
    theme: OxydeTheme,
    cx: &mut Context<PropertiesWindow>,
) -> Option<Div> {
    let mut property_tabs = None;
    if has_shortcut {
        let mut tabs = div()
            .flex()
            .flex_none()
            .items_center()
            .px(rems(1.25))
            .bg(theme.dialog_tabs)
            .gap(rems(0.25))
            .border_b(rems(0.0625))
            .border_color(theme.border);
        for (shortcut, message, icon) in [
            (false, Message::PropertiesGeneral, "icons/file-text.svg"),
            (true, Message::PropertiesShortcut, "icons/link.svg"),
        ] {
            let tab = handle.clone();
            let active = shortcut == dialog.shortcut_tab;
            tabs =
                tabs.child(
                    div()
                        .id(if shortcut {
                            "properties-shortcut-tab"
                        } else {
                            "properties-general-tab"
                        })
                        .px(rems(1.0))
                        .py(rems(0.75))
                        .text_size(rems(0.8125))
                        .font_weight(FontWeight::MEDIUM)
                        .flex()
                        .items_center()
                        .justify_center()
                        .gap(rems(0.5))
                        .when(!active, |tab| {
                            tab.hover(move |style| style.bg(theme.hover).text_color(theme.text))
                        })
                        .child(svg().path(icon).size(rems(0.875)).flex_none().text_color(
                            if active {
                                theme.accent
                            } else {
                                theme.text
                            },
                        ))
                        .rounded_none()
                        .border_b(rems(0.0625))
                        .border_color(if active {
                            theme.accent
                        } else {
                            transparent_black()
                        })
                        .text_color(if active {
                            theme.accent
                        } else {
                            theme.text_muted
                        })
                        .child(cx.i18n().text(message))
                        .on_click(move |_, window, cx| {
                            cx.stop_propagation();
                            tab.update(cx, |this, cx| {
                                this.select_properties_tab(shortcut, window, cx)
                            });
                        }),
                );
        }
        property_tabs = Some(tabs);
    }
    property_tabs
}

pub(super) fn run_menu(
    mut body: Div,
    dialog: &PropertiesDialog,
    handle: &Entity<PropertiesWindow>,
    theme: OxydeTheme,
    window: &mut Window,
    cx: &mut Context<PropertiesWindow>,
) -> Div {
    #[cfg(target_os = "windows")]
    if let Some(position) = dialog.run_menu.filter(|_| crate::ui::components::popup::supported(window, cx)) {
        let bounds = dialog.run_bounds.unwrap_or(Bounds::new(position, size(px(0.0), px(0.0))));
        crate::ui::components::popup::show("shortcut-run", crate::ui::components::popup::PopupAnchor::element(bounds), handle.clone(), window, cx,
            |view| view.properties_dialog.as_ref().is_some_and(|dialog| dialog.run_menu.is_some() && dialog.shortcut_tab),
            |view, _, cx| run_choices(&cx.entity(), *cx.theme(), view.properties_dialog.as_ref().map_or(1, |dialog| dialog.run_window), cx).into_any_element(),
            |view, _, cx| { if let Some(dialog) = &mut view.properties_dialog { dialog.run_menu = None; cx.notify(); } });
        return body;
    }    if let Some(position) = dialog.run_menu {
        let menu = run_choices(handle, theme, dialog.run_window, cx);
        let close = handle.clone();
        body = body.child(deferred(
            anchored()
                .position(position)
                .snap_to_window()
                .child(menu.on_mouse_down_out(move |event, _, cx| {
                    close.update(cx, |this, cx| {
                        if let Some(dialog) = &mut this.properties_dialog {
                            if dialog.run_bounds.is_some_and(|bounds| bounds.contains(&event.position)) {
                                return;
                            }
                            dialog.run_menu = None;
                            cx.notify();
                        }
                    })
                })),
        ));
    }
    body
}

fn run_choices(handle: &Entity<PropertiesWindow>, theme: OxydeTheme, selected_mode: i32, cx: &mut Context<PropertiesWindow>) -> Div {
        let mut menu = crate::ui::components::select_menu::surface(theme)
            .on_any_mouse_down(|_, _, cx| cx.stop_propagation())
            .on_mouse_up(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_mouse_move(|_, _, cx| cx.stop_propagation())
            .on_scroll_wheel(|_, _, cx| cx.stop_propagation())
            .w(rems(17.0));
        for mode in [1, 3, 7] {
            let choice = handle.clone();
            let key_choice = handle.clone();
            let key_action = std::rc::Rc::new(move |_: &mut Window, cx: &mut App| { key_choice.update(cx, |this, cx| { if let Some(dialog) = &mut this.properties_dialog { dialog.run_window = mode; dialog.run_menu = None; cx.notify(); } }); });
            menu = menu.child(
                crate::ui::components::popup::keyboard::item(crate::ui::components::select_menu::row(
                    div().id(("shortcut-run-mode", mode as usize)),
                    theme,
                    selected_mode == mode,
                )
                    .child(cx.i18n().text(run_window_message(mode)))
                    .on_click(move |_, _, cx| {
                        cx.stop_propagation();
                        choice.update(cx, |this, cx| {
                            if let Some(dialog) = &mut this.properties_dialog {
                                dialog.run_window = mode;
                                dialog.run_menu = None;
                                cx.notify();
                            }
                        })
                    }), key_action, None, None),
            );
        }
        menu
}

fn run_window_message(mode: i32) -> Message {
    match mode {
        3 => Message::ShortcutMaximized,
        7 => Message::ShortcutMinimized,
        _ => Message::ShortcutNormal,
    }
}
