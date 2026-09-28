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
        .gap(px(4.0))
        .child(section(i18n.text(Message::SettingsAppearance), theme))
        .child(
            div()
                .py(px(8.0))
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
                .py(px(8.0))
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
                    .py(px(8.0))
                    .text_size(px(12.0))
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
            handle,
            theme,
            |view, cx| {
                view.show_media_previews = !view.show_media_previews;
                view.save_preferences(cx);
            },
        ))
        .child(section(i18n.text(Message::SettingsAbout), theme))
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(14.0))
                .py(px(8.0))
                .child(img("icons/logo.png").size(px(40.0)).flex_none())
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(3.0))
                        .child(
                            div()
                                .text_size(px(15.0))
                                .font_weight(FontWeight::SEMIBOLD)
                                .child("Oxyde"),
                        )
                        .child(
                            div()
                                .text_size(px(12.0))
                                .text_color(theme.text_muted)
                                .child(i18n.text_with_args(Message::SettingsVersion, &version)),
                        )
                        .child(
                            div()
                                .text_size(px(11.0))
                                .text_color(theme.text_muted)
                                .child(i18n.text(Message::SettingsDescription)),
                        ),
                ),
        );
    Modal::new("settings-panel", i18n.text(Message::SettingsTitle), body)
        .width(px(480.0))
        .header_action(
            div()
                .id("settings-close-btn")
                .px(px(8.0))
                .py(px(4.0))
                .rounded_md()
                .cursor_pointer()
                .text_size(px(12.0))
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

fn section(label: SharedString, theme: OxydeTheme) -> impl IntoElement {
    div()
        .mt(px(12.0))
        .py(px(8.0))
        .text_size(px(11.0))
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
        .px(px(10.0))
        .py(px(6.0))
        .ml(px(4.0))
        .rounded_md()
        .cursor_pointer()
        .text_size(px(12.0))
        .bg(if selected {
            theme.selected
        } else {
            theme.surface
        })
        .border_1()
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
        .py(px(10.0))
        .flex()
        .items_center()
        .border_b_1()
        .border_color(theme.border)
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(2.0))
                .flex_1()
                .min_w_0()
                .mr(px(16.0))
                .child(
                    div()
                        .text_size(px(13.0))
                        .text_color(theme.text)
                        .child(label),
                )
                .child(
                    div()
                        .text_size(px(11.0))
                        .text_color(theme.text_muted)
                        .child(description),
                ),
        )
        .child(
            div()
                .id(id)
                .w(px(42.0))
                .h(px(22.0))
                .rounded_full()
                .bg(if value {
                    theme.accent
                } else {
                    theme.text_muted
                })
                .flex()
                .items_center()
                .flex_none()
                .cursor_pointer()
                .pl(px(if value { 22.0 } else { 2.0 }))
                .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                    handle.update(cx, |view, cx| on_toggle(view, cx));
                })
                .child(
                    div()
                        .size(px(18.0))
                        .rounded_full()
                        .bg(theme.surface)
                        .flex_none(),
                ),
        )
}
