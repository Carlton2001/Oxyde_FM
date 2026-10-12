mod about;
mod columns;
mod controls;
mod compression;
mod data_actions;

use controls::{choice, choice_with_icon, section};
use compression::compression_table;

pub(crate) use data_actions::render_settings_confirmation;

use crate::ui::app::OxydeApp;

use crate::ui::format::{date_format, DateFormat};
use crate::ui::i18n::{FluentArgs, I18nContext, Language, Message};
use crate::ui::theme::{ThemeContext, ThemeId};
use gpui::prelude::*;
use gpui::*;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum SettingsPicker {
    Theme,
    Language,
    Date,
    About,
    Compression,
}

pub fn render_settings_panel(
    view: &OxydeApp,
    handle: Entity<OxydeApp>,
    window: &mut Window,
    cx: &mut Context<OxydeApp>,
) -> impl IntoElement {
    let theme = *cx.theme();
    let i18n = cx.i18n();
    let backdrop = handle.clone();
    let themes = ThemeId::ALL.into_iter().map(|id| {
        let handle = handle.clone();
        let mut args = FluentArgs::new();
        args.set("name", id.name());
        let label = if id == ThemeId::Monokai {
            id.name().into()
        } else {
            i18n.text_with_args(if id.is_light() { Message::ThemeLight } else { Message::ThemeDark }, &args)
        };
        choice_with_icon(
            id.as_str(), label, view.theme_id == id, theme,
            Some(if id.is_light() { "icons/sun.svg" } else { "icons/moon.svg" }),
            move |cx| {
                handle.update(cx, |view, cx| {
                    view.set_theme(id, cx);
                    cx.notify();
                });
            },
        ).into_any_element()
    }).collect();
    let languages = [
        (Language::French, "Français"),
        (Language::English, "English"),
        (Language::German, "Deutsch"),
        (Language::Spanish, "Español"),
        (Language::Italian, "Italiano"),
    ]
    .map(|(id, label)| {
        let handle = handle.clone();
        choice(
            id.as_str(),
            label.into(),
            view.language == id,
            theme,
            move |cx| {
                handle.update(cx, |view, cx| {
                    view.set_language(id, cx);
                    cx.notify();
                });
            },
        )
        .into_any_element()
    })
    .into_iter()
    .collect();
    let dates = [
        (DateFormat::Us, Message::DateFormatUs),
        (DateFormat::European, Message::DateFormatEuropean),
        (DateFormat::Iso, Message::DateFormatIso),
    ]
    .map(|(format, label)| {
        let handle = handle.clone();
        choice(
            format.as_str(),
            i18n.text(label),
            date_format(cx) == format,
            theme,
            move |cx| {
                handle.update(cx, |view, cx| {
                    view.set_date_format(format, cx);
                });
            },
        )
        .into_any_element()
    })
    .into_iter()
    .collect();
    let compression = compression_table(view, handle.clone(), cx);
    // Keep the settings columns compact and stable when About is toggled.
    // Measure translated labels before allocating space to icons and controls.
    // These widths depend on the content, never on whether About is open.
    let mut settings_font = window.text_style().font();
    settings_font.family = "Inter".into();
    settings_font.weight = crate::ui::style::DEFAULT_FONT_WEIGHT;
    let label_width = |message| {
        let label = i18n.text(message);
        let line = window.text_system().shape_line(
            label.clone(), crate::ui::scale::rem_pixels(rems(0.8125)),
            &[TextRun {
                len: label.len(), font: settings_font.clone(), color: theme.text,
                background_color: None, underline: None, strikethrough: None,
            }], None,
        );
        line.width.as_f32() / crate::ui::scale::rem_pixels(rems(1.0)).as_f32()
    };
    let left_width = [
        Message::SettingsTheme, Message::SettingsGroupByDate,
        Message::SettingsGridThumbnails, Message::SettingsWindowsIcons,
        Message::SettingsCheckboxes, Message::SettingsPathTracking,
        Message::SettingsHidden, Message::SettingsSystem,
        Message::SettingsPreviews, Message::SettingsNetwork,
    ].into_iter().map(|message| label_width(message) + 5.0625)
        .fold(16.5_f32, f32::max)
        .max(label_width(Message::SettingsScale) + 9.6875);
    let right_width = [
        Message::SettingsLanguage, Message::SettingsDateFormat, Message::Compression,
        Message::SettingsTurbo, Message::SettingsClearCache,
        Message::SettingsResetPreferences, Message::SettingsAbout,
    ].into_iter().map(|message| label_width(message) + 4.5)
        .fold(17.5_f32, f32::max)
        .max(label_width(Message::SettingsSearchLimit) + 9.125);
    let about_width = 16.5;
    let column_gap = 0.5;
    // Panel padding (2 × 0.375 rem) and borders (2 × 0.0625 rem).
    let panel_frame = 0.875;
    let panel_width = left_width + right_width + column_gap + panel_frame
        + if view.settings_about_open { about_width + column_gap } else { 0.0 };
    let left = columns::left_column(view, handle.clone(), left_width, cx);
    let about = about::about_column(view, handle.clone(), about_width, cx);
    let right = columns::right_column(view, handle.clone(), right_width, cx);
    let body = div()
        .flex()
        .flex_col()
        .child(div().flex().gap(rems(column_gap)).child(left).child(right)
            .when(view.settings_about_open, |columns| columns.child(about)))
        .when_some(view.settings_data_status, |body, status| {
            body.child(div().px(rems(0.375)).py(rems(0.375))
                .text_color(if status == Message::SettingsCacheError { theme.error } else { theme.text_muted })
                .child(i18n.text(status)))
        })
        .when_some(view.settings_error, |body, error| {
            let handle = handle.clone();
            body.child(
                div()
                    .py(rems(0.375))
                    .text_size(rems(0.8125))
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
        });
    let margin = crate::ui::scale::rem_pixels(rems(0.375));
    let origin = view
        .settings_button_bounds
        .map(|b| point(px(0.0), b.bottom()))
        .unwrap_or(point(
            px(0.0),
            px(crate::ui::components::title_bar::titlebar_h()),
        ));
    let available_height = (window.viewport_size().height - origin.y - margin).max(px(1.0));
    let panel = div()
        .id("settings-panel")
        .w(rems(panel_width))
        .max_w(window.viewport_size().width - margin * 2.0)
        .max_h(available_height)
        .overflow_x_scroll()
        .overflow_y_scroll()
        .p(rems(0.375))
        .rounded_lg()
        .bg(theme.menu_surface)
        .border(rems(0.0625))
        .border_color(theme.menu_border)
        .shadow_md()
        .text_size(rems(0.8125))
        .occlude()
        .on_any_mouse_down(|_, _, cx| cx.stop_propagation())
        .on_scroll_wheel(|_, _, cx| cx.stop_propagation())
        .child(body);
    let mut overlay = div()
        .absolute()
        .top_0()
        .bottom_0()
        .left_0()
        .right_0()
        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
            cx.stop_propagation();
            backdrop.update(cx, |view, cx| {
                view.settings_open = false;
                view.settings_about_open = false;
                view.settings_picker = None;
                view.settings_submenu_bounds = None;
                cx.notify();
            });
        })
        .child(anchored().position(origin).snap_to_window().child(panel));
    if let (Some(kind), Some(bounds)) = (view.settings_picker.filter(|kind| *kind != SettingsPicker::About), view.settings_submenu_bounds) {
        let (title, choices): (SharedString, Vec<AnyElement>) = match kind {
            SettingsPicker::Theme => (i18n.text(Message::SettingsTheme), themes),
            SettingsPicker::Language => (i18n.text(Message::SettingsLanguage), languages),
            SettingsPicker::Date => (i18n.text(Message::SettingsDateFormat), dates),
            SettingsPicker::About => unreachable!("About is rendered as a settings column"),
            SettingsPicker::Compression => (i18n.text(Message::Compression), compression),

        };
        let submenu_width =
            crate::ui::scale::rem_pixels(rems(if kind == SettingsPicker::Compression {
                22.0
            } else {
                18.0
            }))
            .min(window.viewport_size().width - margin * 2.0);
        // Let the content determine the height. Anchoring moves the menu upwards
        // when needed; scrolling is only necessary if it exceeds the viewport.
        let submenu_available_height =
            (window.viewport_size().height - margin * 2.0).max(px(1.0));
        let submenu = div()
            .id("settings-submenu")
            .w(submenu_width)
            .max_h(submenu_available_height)
            .overflow_y_scroll()
            .p(rems(0.25))
            .rounded_lg()
            .bg(theme.menu_surface)
            .border(rems(0.0625))
            .border_color(theme.menu_border)
            .shadow_md()
            .occlude()
            .text_size(rems(0.8125))
            .on_any_mouse_down(|_, _, cx| cx.stop_propagation())
            .on_scroll_wheel(|_, _, cx| cx.stop_propagation())
            .child(section(title, theme))
            .children(choices);
        let open_left = bounds.right() + submenu_width + margin > window.viewport_size().width
            && bounds.left() - submenu_width >= margin;
        let position = point(
            if open_left {
                bounds.left()
            } else {
                bounds.right()
            },
            bounds.top(),
        );
        overlay = overlay.child(
            anchored()
                .position(position)
                .anchor(if open_left {
                    Anchor::TopRight
                } else {
                    Anchor::TopLeft
                })
                .snap_to_window()
                .child(submenu),
        );
    }
    overlay
}
