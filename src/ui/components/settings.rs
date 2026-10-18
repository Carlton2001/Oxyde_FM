mod about;
mod columns;
mod controls;
mod compression;
mod default_format;
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

pub fn render_settings_panel(view: &OxydeApp, handle: Entity<OxydeApp>, window: &mut Window, cx: &mut Context<OxydeApp>) -> AnyElement {
    #[cfg(target_os = "windows")]
    if crate::ui::components::popup::supported(window, cx) {
        let bounds = view.settings_button_bounds.unwrap_or(Bounds::new(point(px(0.0), px(0.0)), size(px(0.0), px(44.0))));
        crate::ui::components::popup::show("settings-panel", crate::ui::components::popup::PopupAnchor::element(bounds), handle.clone(), window, cx,
            |view| view.settings_open,
            |view, window, cx| render_settings_surface(view, cx.entity(), window, cx, None),
            |view, _, cx| { view.archive_format_bounds = None; view.settings_open = false; view.settings_about_open = false; view.settings_picker = None; view.settings_submenu_bounds = None; cx.notify(); });
        return div().into_any_element();
    }
    render_settings_surface(view, handle, window, cx, None)
}
fn render_settings_surface(
    view: &OxydeApp,
    handle: Entity<OxydeApp>,
    window: &mut Window,
    cx: &mut Context<OxydeApp>,
    _only_picker: Option<SettingsPicker>,
) -> AnyElement {
    // Only render this control in its actual owner, never in the parent menu.
    #[cfg(target_os = "windows")]
    let render_compression = _only_picker == Some(SettingsPicker::Compression)
        || !crate::ui::components::popup::supported(window, cx);
    #[cfg(not(target_os = "windows"))]
    let render_compression = true;
    let compression = if render_compression {
        compression_table(view, handle.clone(), window, cx)
    } else { Vec::new() };
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
        Message::SettingsTheme,
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
    let available_height = (crate::ui::components::popup::available_size(window, cx).height - origin.y - margin).max(px(1.0));
    let panel = div()
        .id("settings-panel")
        .w(rems(panel_width))
        .max_w(crate::ui::components::popup::available_size(window, cx).width - margin * 2.0)
        .max_h(available_height)
        .p(rems(0.4375))
        .rounded_lg()
        .bg(theme.menu_surface)
        .border(rems(0.0625))
        .border_color(theme.menu_border)
        .shadow(crate::ui::style::rem_shadow(false))
        .text_size(rems(0.8125))
        .text_color(theme.text)
        .occlude()
        .on_any_mouse_down(|_, _, cx| cx.stop_propagation())
        .on_scroll_wheel(|_, _, cx| cx.stop_propagation())
        .child(super::scroll_area::scroll_area("settings-panel-scroll-state",
            div().max_h((available_height - window.rem_size()).max(px(1.0))),
            div().id("settings-panel-scroll").child(body), true, window, cx));
    #[cfg(target_os = "windows")]
    if _only_picker.is_none() && crate::ui::components::popup::is_menu(window, cx) {
        if let (Some(kind), Some(bounds)) = (view.settings_picker.filter(|kind| *kind != SettingsPicker::About), view.settings_submenu_bounds) {
            let open_left = false;
            crate::ui::components::popup::show("settings-picker", crate::ui::components::popup::PopupAnchor::beside(bounds, open_left), handle.clone(), window, cx,
                move |view| view.settings_open && view.settings_picker == Some(kind),
                move |view, window, cx| render_settings_surface(view, cx.entity(), window, cx, Some(kind)),
                |view, _, cx| { view.archive_format_bounds = None; view.settings_picker = None; view.settings_submenu_bounds = None; cx.notify(); });
        }
        return panel.into_any_element();
    }    let mut overlay = div()
        .absolute()
        .top_0()
        .bottom_0()
        .left_0()
        .right_0()
        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
            cx.stop_propagation();
            backdrop.update(cx, |view, cx| {
                view.archive_format_bounds = None;
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
            SettingsPicker::Theme => (cx.i18n().text(Message::SettingsTheme), themes),
            SettingsPicker::Language => (cx.i18n().text(Message::SettingsLanguage), languages),
            SettingsPicker::Date => (cx.i18n().text(Message::SettingsDateFormat), dates),
            SettingsPicker::About => unreachable!("About is rendered as a settings column"),
            SettingsPicker::Compression => (cx.i18n().text(Message::Compression), compression),

        };
        let submenu_width =
            crate::ui::scale::rem_pixels(rems(if kind == SettingsPicker::Compression {
                22.0
            } else {
                18.0
            }))
            .min(crate::ui::components::popup::available_size(window, cx).width - margin * 2.0);
        // Let the content determine the height. Anchoring moves the menu upwards
        // when needed; scrolling is only necessary if it exceeds the viewport.
        let submenu_available_height =
            (crate::ui::components::popup::available_size(window, cx).height - margin * 2.0).max(px(1.0));
        let submenu = div()
            .id("settings-submenu")
            .w(submenu_width)
            .max_h(submenu_available_height)
            .p(rems(0.4375))
            .rounded_lg()
            .bg(theme.menu_surface)
            .border(rems(0.0625))
            .border_color(theme.menu_border)
            .shadow(crate::ui::style::rem_shadow(false))
            .occlude()
            .text_size(rems(0.8125))
            .text_color(theme.text)
            .on_any_mouse_down(|_, _, cx| cx.stop_propagation())
            .on_scroll_wheel(|_, _, cx| cx.stop_propagation())
            .child(super::scroll_area::scroll_area("settings-submenu-scroll-state",
                div().max_h((submenu_available_height - window.rem_size()).max(px(1.0))),
                div().id("settings-submenu-scroll").child(section(title, theme)).children(choices),
                false, window, cx));

        #[cfg(target_os = "windows")]
        if _only_picker.is_some() { return submenu.into_any_element(); }
        let open_left = bounds.right() + submenu_width + margin > crate::ui::components::popup::available_size(window, cx).width
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
    overlay.into_any_element()
}
