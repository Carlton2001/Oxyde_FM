//! The panel's two columns: what Oxyde shows on the left, how it works on the right.
use crate::ui::app::{OxydeApp, SettingsDataAction};
use crate::ui::i18n::{I18nContext, Message};
use crate::ui::theme::ThemeContext;
use gpui::prelude::*;
use gpui::*;
use super::SettingsPicker;
use super::controls::{picker, scale_row, search_limit_row, section, separator, toggle_row, window_row, Toggle};
use super::data_actions::data_action_row;
use super::measure::Trailing;

/// The left column's rows, for its width: every row any system may show.
pub(super) fn left_rows(cx: &App) -> Vec<(Message, Trailing)> {
    let scale = super::controls::scale_value(crate::ui::scale::MAX_REM_SIZE, cx);
    let mut rows = vec![
        (Message::SettingsTheme, Trailing::Icon),
        (Message::SettingsScale, Trailing::Stepper(scale)),
    ];
    rows.extend([
        Message::SettingsWindowsIcons, Message::SettingsGridThumbnails,
        Message::SettingsHidden, Message::SettingsSystem, Message::SettingsPreviews,
        Message::SettingsCheckboxes, Message::SettingsPermissionsColumn,
        Message::SettingsPathTracking, Message::SettingsCloudRoots, Message::SettingsNetwork,
    ].map(|label| (label, Trailing::Toggle)));
    rows
}

/// The right column's rows, for its width.
pub(super) fn right_rows() -> Vec<(Message, Trailing)> {
    let limit = oxyde_lib::models::preferences::MAX_SEARCH_LIMIT.to_string();
    vec![
        (Message::SettingsLanguage, Trailing::Icon),
        (Message::SettingsDateFormat, Trailing::Icon),
        (Message::Compression, Trailing::Icon),
        (Message::SettingsTurbo, Trailing::Toggle),
        (Message::SettingsSearchLimit, Trailing::Stepper(limit.into())),
        (Message::SettingsSystemIntegration, Trailing::Icon),
        (Message::SettingsClearCache, Trailing::Nothing),
        (Message::SettingsCacheClearing, Trailing::Nothing),
        (Message::SettingsResetPreferences, Trailing::Nothing),
        (Message::SettingsAbout, Trailing::Icon),
    ]
}

pub(super) fn left_column(view: &OxydeApp, handle: Entity<OxydeApp>, width: f32, cx: &App) -> impl IntoElement {
    let theme = *cx.theme();
    let i18n = cx.i18n();
    let toggle = |toggle: Toggle, on_toggle: fn(&mut OxydeApp, &mut Context<OxydeApp>)| {
        toggle_row(toggle, handle.clone(), cx, on_toggle)
    };
    div()
        .w(rems(width))
        .flex_none()
        .flex()
        .flex_col()
        .pr(rems(0.5))
        .border_r(rems(0.0625))
        .border_color(theme.border)
        .child(section(i18n.text(Message::SettingsAppearance), theme))
        .child({
            let handle = handle.clone();
            window_row("settings-theme-window", "icons/palette.svg", i18n.text(Message::SettingsTheme), theme,
                move |window, cx| crate::ui::components::popup::with_owner(window, cx,
                    |window, cx| handle.update(cx, |view, cx| view.open_theme_window(window, cx))))
        })
        .child(scale_row(view, handle.clone(), cx))
        .when(cfg!(target_os = "windows"), |column| column.child(toggle(Toggle {
            id: "toggle-windows-icons", icon: "icons/app-window.svg",
            label: Message::SettingsWindowsIcons, description: Message::SettingsWindowsIconsDescription,
            hint: None, value: view.use_system_icons(),
        }, |view, cx| view.toggle_system_icons(cx))))
        .child(toggle(Toggle {
            id: "toggle-grid-thumbnails", icon: "icons/layout-grid.svg",
            label: Message::SettingsGridThumbnails, description: Message::SettingsGridThumbnailsDescription,
            hint: None, value: view.show_grid_thumbnails,
        }, |view, cx| {
            view.show_grid_thumbnails = !view.show_grid_thumbnails;
            view.save_preferences(cx);
        }))
        .child(separator(theme))
        .child(section(i18n.text(Message::SettingsFiles), theme))
        .child(toggle(Toggle {
            id: "toggle-hidden", icon: "icons/eye-off.svg",
            label: Message::SettingsHidden, description: Message::SettingsHiddenDescription,
            hint: None, value: view.show_hidden,
        }, |view, cx| {
            view.show_hidden = !view.show_hidden;
            view.refresh_all_tabs(cx);
            view.save_preferences(cx);
        }))
        .when(oxyde_lib::platform::platform().has_system_file_attribute(), |column| column.child(toggle(Toggle {
            id: "toggle-system", icon: "icons/shield.svg",
            label: Message::SettingsSystem, description: Message::SettingsSystemDescription,
            hint: None, value: view.show_system,
        }, |view, cx| {
            view.show_system = !view.show_system;
            view.refresh_all_tabs(cx);
            view.save_preferences(cx);
        })))
        .child(toggle(Toggle {
            id: "toggle-media", icon: "icons/image.svg",
            label: Message::SettingsPreviews, description: Message::SettingsPreviewsDescription,
            hint: None, value: view.show_media_previews,
        }, |view, cx| {
            view.show_media_previews = !view.show_media_previews;
            view.save_preferences(cx);
        }))
        .child(toggle(Toggle {
            id: "toggle-selection-checkboxes", icon: "icons/square-check.svg",
            label: Message::SettingsCheckboxes, description: Message::SettingsCheckboxesDescription,
            hint: None, value: view.show_checkboxes(),
        }, |view, cx| view.toggle_checkboxes(cx)))
        .when(cfg!(unix), |column| column.child(toggle(Toggle {
            id: "toggle-permissions-column", icon: "icons/key.svg",
            label: Message::SettingsPermissionsColumn, description: Message::SettingsPermissionsColumnDescription,
            hint: None, value: view.show_permissions_column(),
        }, |view, cx| view.toggle_permissions_column(cx))))
        .child(separator(theme))
        .child(section(i18n.text(Message::SettingsSidebar), theme))
        .child(toggle(Toggle {
            id: "toggle-path-tracking", icon: "icons/folder-tree.svg",
            label: Message::SettingsPathTracking, description: Message::SettingsPathTrackingDescription,
            hint: None, value: view.follows_active_path(),
        }, |view, cx| view.toggle_path_tracking(cx)))
        .when(view.has_cloud_roots, |column| column.child(toggle(Toggle {
            id: "toggle-onedrive", icon: "icons/cloudy.svg",
            label: Message::SettingsCloudRoots, description: Message::SettingsCloudRootsDescription,
            hint: None, value: view.show_cloud_roots(),
        }, |view, cx| view.toggle_cloud_roots(cx))))
        .child(toggle(Toggle {
            id: "toggle-network", icon: "icons/globe.svg",
            label: Message::SettingsNetwork, description: Message::SettingsNetworkDescription,
            hint: None, value: view.show_network(),
        }, |view, cx| view.toggle_network(cx)))
}

pub(super) fn right_column(view: &OxydeApp, handle: Entity<OxydeApp>, width: f32, cx: &App) -> impl IntoElement {
    let theme = *cx.theme();
    let i18n = cx.i18n();
    let submenu_row = |kind, id, icon, label| picker(view, handle.clone(), kind, id, icon, i18n.text(label), theme);
    div()
        .w(rems(width))
        .flex_none()
        .flex()
        .flex_col()
        .child(section(i18n.text(Message::SettingsRegionLanguage), theme))
        .child(submenu_row(SettingsPicker::Language, "settings-language-picker", "icons/languages.svg", Message::SettingsLanguage))
        .child(submenu_row(SettingsPicker::Date, "settings-date-picker", "icons/calendar-days.svg", Message::SettingsDateFormat))
        .child(separator(theme))
        .child(section(i18n.text(Message::SettingsPerformance), theme))
        .child(submenu_row(SettingsPicker::Compression, "settings-compression-picker", "icons/archive.svg", Message::Compression))
        .child(toggle_row(Toggle {
            id: "toggle-default-turbo", icon: "icons/zap.svg",
            label: Message::SettingsTurbo, description: Message::SettingsTurboDescription,
            hint: Some(Message::SettingsTurboHint), value: view.default_turbo_mode(),
        }, handle.clone(), cx, |view, cx| view.toggle_default_turbo(cx)))
        .child(search_limit_row(view, handle.clone(), cx))
        .when(super::integration::available(view), |column| column
            .child(separator(theme))
            .child(section(i18n.text(Message::SettingsSystemSection), theme))
            .child(submenu_row(SettingsPicker::Integration, "settings-integration-picker", "icons/monitor.svg",
                Message::SettingsSystemIntegration)))
        .child(separator(theme))
        .child(section(i18n.text(Message::SettingsData), theme))
        .child(data_action_row(
            handle.clone(), SettingsDataAction::ClearCache, "settings-clear-cache",
            "icons/trash.svg",
            i18n.text(if view.clearing_cache { Message::SettingsCacheClearing } else { Message::SettingsClearCache }),
            !view.clearing_cache, theme,
        ))
        .child(data_action_row(
            handle.clone(), SettingsDataAction::ResetPreferences, "settings-reset-preferences",
            "icons/rotate-ccw.svg", i18n.text(Message::SettingsResetPreferences),
            true, theme,
        ))
        .child(div().flex_1().min_h(rems(0.75)))
        .child(separator(theme))
        .child(submenu_row(SettingsPicker::About, "settings-about-picker", "icons/info.svg", Message::SettingsAbout))
}
