use crate::ui::app::OxydeApp;
use crate::ui::components::modal::Modal;
use crate::ui::i18n::{FluentArgs, I18nContext, Language, Message};
use crate::ui::theme::{OxydeTheme, ThemeContext, ThemeId};
use gpui::prelude::*;
use gpui::*;

pub fn render_settings_panel(
    view: &OxydeApp,
    handle: Entity<OxydeApp>,
    cx: &mut Context<OxydeApp>,
) -> impl IntoElement {
    let theme = *cx.theme();
    let i18n = cx.i18n();
    let close = handle.clone();
    let backdrop = handle.clone();
    let mut version = FluentArgs::new();
    version.set("version", env!("CARGO_PKG_VERSION"));
    let body = div()
        .flex()
        .flex_col()
        .gap(rems(0.25))
        .child(section(i18n.text(Message::SettingsAppearance), theme))
        .child(scale_row(view, handle.clone(), cx))
        .when(cfg!(target_os = "windows"), |body| body.child(settings_row(
            i18n.text(Message::SettingsWindowsIcons), i18n.text(Message::SettingsWindowsIconsDescription),
            view.use_system_icons(), "toggle-windows-icons", handle.clone(), theme,
            |view, cx| { view.toggle_system_icons(cx); },
        )))
        .child(
            div()
                .py(rems(0.5))
                .flex()
                .items_center()
                .child(div().flex_1().child(i18n.text(Message::SettingsTheme)))
                .children(
                    [
                        (ThemeId::Dark, Message::ThemeDark),
                        (ThemeId::Light, Message::ThemeLight),
                    ]
                    .map(|(id, label)| {
                        let handle = handle.clone();
                        choice(
                            id.as_str(),
                            i18n.text(label),
                            view.theme_id == id,
                            theme,
                            move |cx| {
                                handle.update(cx, |view, cx| view.set_theme(id, cx));
                            },
                        )
                    }),
                ),
        )
        .child(
            div()
                .py(rems(0.5))
                .flex()
                .items_center()
                .child(div().flex_1().child(i18n.text(Message::SettingsLanguage)))
                .children(
                    [
                        (Language::French, "Français"),
                        (Language::English, "English"),
                    ]
                    .map(|(id, label)| {
                        let handle = handle.clone();
                        choice(
                            id.as_str(),
                            label.into(),
                            view.language == id,
                            theme,
                            move |cx| {
                                handle.update(cx, |view, cx| view.set_language(id, cx));
                            },
                        )
                    }),
                ),
        )
        .when_some(view.settings_error, |body, error| {
            let handle = handle.clone();
            body.child(
                div()
                    .py(rems(0.5))
                    .text_size(rems(0.75))
                    .text_color(theme.error)
                    .child(i18n.text(error))
                    .when(error == Message::SettingsSaveError, |row| {
                        row.child(choice(
                            "retry-save",
                            i18n.text(Message::Retry),
                            false,
                            theme,
                            move |cx| {
                                handle.update(cx, |view, cx| view.save_preferences(cx));
                            },
                        ))
                    }),
            )
        })
        .child(section(i18n.text(Message::SettingsFiles), theme))
        .child(settings_row(
            i18n.text(Message::SettingsHidden),
            i18n.text(Message::SettingsHiddenDescription),
            view.show_hidden,
            "toggle-hidden",
            handle.clone(),
            theme,
            |view, cx| {
                view.show_hidden = !view.show_hidden;
                view.refresh_all_tabs(cx);
                view.save_preferences(cx);
            },
        ))
        .child(settings_row(
            i18n.text(Message::SettingsSystem),
            i18n.text(Message::SettingsSystemDescription),
            view.show_system,
            "toggle-system",
            handle.clone(),
            theme,
            |view, cx| {
                view.show_system = !view.show_system;
                view.refresh_all_tabs(cx);
                view.save_preferences(cx);
            },
        ))
        .child(settings_row(
            i18n.text(Message::SettingsPreviews),
            i18n.text(Message::SettingsPreviewsDescription),
            view.show_media_previews,
            "toggle-media",
            handle.clone(),
            theme,
            |view, cx| {
                view.show_media_previews = !view.show_media_previews;
                view.save_preferences(cx);
            },
        ))
        .child(settings_row(
            i18n.text(Message::SettingsGridThumbnails),
            i18n.text(Message::SettingsGridThumbnailsDescription),
            view.show_grid_thumbnails,
            "toggle-grid-thumbnails",
            handle.clone(),
            theme,
            |view, cx| {
                view.show_grid_thumbnails = !view.show_grid_thumbnails;
                view.save_preferences(cx);
            },
        ))
        .child(section(i18n.text(Message::SettingsAbout), theme))
        .child(
            div()
                .flex()
                .items_center()
                .gap(rems(0.875))
                .py(rems(0.5))
                .child(img("icons/logo.png").size(rems(2.5)).flex_none())
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(rems(0.1875))
                        .child(
                            div()
                                .text_size(rems(0.9375))
                                .font_weight(FontWeight::SEMIBOLD)
                                .child("Oxyde"),
                        )
                        .child(
                            div()
                                .text_size(rems(0.75))
                                .text_color(theme.text_muted)
                                .child(i18n.text_with_args(Message::SettingsVersion, &version)),
                        )
                        .child(
                            div()
                                .text_size(rems(0.6875))
                                .text_color(theme.text_muted)
                                .child(i18n.text(Message::SettingsDescription)),
                        ),
                ),
        );
    Modal::new("settings-panel", i18n.text(Message::SettingsTitle), body)
        .width(rems(30.0))
        .header_action(
            div()
                .id("settings-close-btn")
                .px(rems(0.5))
                .py(rems(0.25))
                .rounded_md()
                .text_size(rems(0.75))
                .text_color(theme.text_muted)
                .hover(|style| style.bg(theme.control_hover))
                .child(i18n.text(Message::Close))
                .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                    close.update(cx, |view, cx| {
                        view.settings_open = false;
                        cx.notify();
                    });
                }),
        )
        .on_backdrop_dismiss(move |_, cx| {
            backdrop.update(cx, |view, cx| {
                view.settings_open = false;
                cx.notify();
            });
        })
}

fn scale_row(view: &OxydeApp, handle: Entity<OxydeApp>, cx: &App) -> impl IntoElement {
    use crate::ui::components::tooltip::Tooltip;
    use crate::ui::scale::{BASE_REM_SIZE, MAX_REM_SIZE, MIN_REM_SIZE};
    let theme = *cx.theme();
    let reference = view.ui_rem_size();
    let mut args = FluentArgs::new();
    args.set("reference", reference);
    let reset = handle.clone();
    let step = |id: &'static str, label: &'static str, increase: bool| {
        let handle = handle.clone();
        let enabled = if increase {
            reference < MAX_REM_SIZE
        } else {
            reference > MIN_REM_SIZE
        };
        div()
            .id(id)
            .size(rems(1.75))
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .rounded_md()
            .when(!enabled, |d| d.opacity(0.4))
            .when(enabled, |d| d.hover(|s| s.bg(theme.control_hover)))
            .child(label)
            .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                cx.stop_propagation();
                if enabled {
                    handle.update(cx, |view, cx| {
                        let reference = view.ui_rem_size();
                        let next = if increase {
                            reference + 1
                        } else {
                            reference.saturating_sub(1)
                        };
                        view.set_ui_rem_size(next, window, cx);
                    });
                }
            })
    };
    div()
        .py(rems(0.5))
        .flex()
        .items_center()
        .child(div().flex_1().child(cx.i18n().text(Message::SettingsScale)))
        .child(
            div()
                .flex()
                .items_center()
                .rounded_md()
                .bg(theme.surface)
                .child(step("scale-decrease", "−", false))
                .child(
                    div()
                        .id("scale-reset")
                        .min_w(rems(3.75))
                        .h(rems(1.75))
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_color(theme.accent)
                        .tooltip(Tooltip::text(cx.i18n().text(Message::SettingsScaleReset)))
                        .child(cx.i18n().text_with_args(Message::SettingsScaleValue, &args))
                        .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                            reset.update(cx, |view, cx| {
                                view.set_ui_rem_size(BASE_REM_SIZE, window, cx)
                            });
                        }),
                )
                .child(step("scale-increase", "+", true)),
        )
}

fn section(label: SharedString, theme: OxydeTheme) -> impl IntoElement {
    div()
        .mt(rems(0.75))
        .py(rems(0.5))
        .text_size(rems(0.6875))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(theme.text_muted)
        .child(label)
}

fn choice(
    id: &'static str,
    label: SharedString,
    selected: bool,
    theme: OxydeTheme,
    on_select: impl Fn(&mut App) + 'static,
) -> impl IntoElement {
    div()
        .id(id)
        .px(rems(0.625))
        .py(rems(0.375))
        .ml(rems(0.25))
        .rounded_md()
        .text_size(rems(0.75))
        .bg(if selected {
            theme.selected
        } else {
            theme.surface
        })
        .border(rems(0.0625))
        .border_color(if selected { theme.accent } else { theme.border })
        .text_color(theme.text)
        .hover(|style| style.bg(theme.control_hover))
        .child(label)
        .on_mouse_down(MouseButton::Left, move |_, _, cx| on_select(cx))
}

fn settings_row(
    label: SharedString,
    description: SharedString,
    value: bool,
    id: &'static str,
    handle: Entity<OxydeApp>,
    theme: OxydeTheme,
    on_toggle: impl Fn(&mut OxydeApp, &mut Context<OxydeApp>) + 'static,
) -> impl IntoElement {
    div()
        .py(rems(0.625))
        .flex()
        .items_center()
        .border_b(rems(0.0625))
        .border_color(theme.border)
        .child(
            div()
                .flex()
                .flex_col()
                .gap(rems(0.125))
                .flex_1()
                .min_w_0()
                .mr(rems(1.0))
                .child(
                    div()
                        .text_size(rems(0.8125))
                        .text_color(theme.text)
                        .child(label),
                )
                .child(
                    div()
                        .text_size(rems(0.6875))
                        .text_color(theme.text_muted)
                        .child(description),
                ),
        )
        .child(
            div()
                .id(id)
                .w(rems(2.625))
                .h(rems(1.375))
                .rounded_full()
                .bg(if value {
                    theme.accent
                } else {
                    theme.text_muted
                })
                .flex()
                .items_center()
                .flex_none()
                .pl(px(if value { 22.0 } else { 2.0 }))
                .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                    handle.update(cx, |view, cx| on_toggle(view, cx));
                })
                .child(
                    div()
                        .size(rems(1.125))
                        .rounded_full()
                        .bg(theme.surface)
                        .flex_none(),
                ),
        )
}
