use crate::ui::app::OxydeApp;
use crate::ui::i18n::{I18nContext, Message};
use crate::ui::theme::ThemeContext;
use gpui::prelude::*;
use gpui::*;
use super::SettingsPicker;
use super::controls::{picker, scale_row, search_limit_row, section, separator, settings_row};
use super::data_actions::data_action_row;
use crate::ui::app::SettingsDataAction;

pub(super) fn left_column(view: &OxydeApp, handle: Entity<OxydeApp>, left_width: f32, cx: &App) -> impl IntoElement {
    let theme = *cx.theme();
    let i18n = cx.i18n();
    div()
        .w(rems(left_width))
        .flex_none()
        .flex()
        .flex_col()
        .pr(rems(0.5))
        .border_r(rems(0.0625))
        .border_color(theme.border)
        .child(section(i18n.text(Message::SettingsAppearance), theme))
        .child(picker(
            view,
            handle.clone(),
            SettingsPicker::Theme,
            "settings-theme-picker",
            "icons/sun.svg",
            i18n.text(Message::SettingsTheme),
            theme,
        ))
        .child(scale_row(view, handle.clone(), cx))
        .child(settings_row(
            i18n.text(Message::SettingsGroupByDate),
            i18n.text(Message::SettingsGroupByDateDescription),
            "icons/calendar-days.svg",
            view.state.find_pane(&view.state.active_pane_id).is_some_and(|pane| pane.active_tab().group_by_date),
            "toggle-date-grouping",
            handle.clone(),
            theme,
            |view, cx| {
                let pane_id = view.state.active_pane_id.clone();
                view.toggle_date_grouping(&pane_id, cx);
            },
        ))
        .child(settings_row(
            i18n.text(Message::SettingsGridThumbnails),
            i18n.text(Message::SettingsGridThumbnailsDescription),
            "icons/layout-grid.svg",
            view.show_grid_thumbnails,
            "toggle-grid-thumbnails",
            handle.clone(),
            theme,
            |view, cx| {
                view.show_grid_thumbnails = !view.show_grid_thumbnails;
                view.save_preferences(cx);
            },
        ))
        .when(cfg!(target_os = "windows"), |body| {
            body.child(settings_row(
                i18n.text(Message::SettingsWindowsIcons),
                i18n.text(Message::SettingsWindowsIconsDescription),
                "icons/window-maximize.svg",
                view.use_system_icons(),
                "toggle-windows-icons",
                handle.clone(),
                theme,
                |view, cx| view.toggle_system_icons(cx),
            ))
        })
        .child(settings_row(
            i18n.text(Message::SettingsCheckboxes),
            i18n.text(Message::SettingsCheckboxesDescription),
            "icons/square-check.svg",
            view.show_checkboxes(),
            "toggle-selection-checkboxes",
            handle.clone(),
            theme,
            |view, cx| view.toggle_checkboxes(cx),
        ))
        .child(settings_row(
            i18n.text(Message::SettingsPathTracking),
            i18n.text(Message::SettingsPathTrackingDescription),
            "icons/folder-tree.svg",
            view.follows_active_path(),
            "toggle-path-tracking",
            handle.clone(),
            theme,
            |view, cx| view.toggle_path_tracking(cx),
        ))
        .child(separator(theme))
        .child(section(i18n.text(Message::SettingsFiles), theme))
        .child(settings_row(
            i18n.text(Message::SettingsHidden),
            i18n.text(Message::SettingsHiddenDescription),
            "icons/eye.svg",
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
            "icons/eye.svg",
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
            "icons/eye.svg",
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
            i18n.text(Message::SettingsNetwork),
            i18n.text(Message::SettingsNetworkDescription),
            "icons/globe.svg",
            view.show_network(),
            "toggle-network",
            handle.clone(),
            theme,
            |view, cx| view.toggle_network(cx),
        ))
}

pub(super) fn right_column(view: &OxydeApp, handle: Entity<OxydeApp>, right_width: f32, cx: &App) -> impl IntoElement {
    let theme = *cx.theme();
    let i18n = cx.i18n();
    div()
        .w(rems(right_width))
        .flex_none()
        .flex()
        .flex_col()
        .child(section(i18n.text(Message::SettingsRegionLanguage), theme))
        .child(picker(
            view,
            handle.clone(),
            SettingsPicker::Language,
            "settings-language-picker",
            "icons/languages.svg",
            i18n.text(Message::SettingsLanguage),
            theme,
        ))
        .child(picker(
            view,
            handle.clone(),
            SettingsPicker::Date,
            "settings-date-picker",
            "icons/calendar-days.svg",
            i18n.text(Message::SettingsDateFormat),
            theme,
        ))
        .child(separator(theme))
        .child(section(i18n.text(Message::SettingsPerformance), theme))
        .child(picker(
            view,
            handle.clone(),
            SettingsPicker::Compression,
            "settings-compression-picker",
            "icons/archive.svg",
            i18n.text(Message::Compression),
            theme,
        ))
        .child(settings_row(
            i18n.text(Message::SettingsTurbo),
            i18n.text(Message::SettingsTurboDescription),
            "icons/zap.svg", view.default_turbo_mode(), "toggle-default-turbo",
            handle.clone(), theme,
            |view, cx| view.toggle_default_turbo(cx),
        ))
        .child(search_limit_row(view, handle.clone(), cx))
        .child(separator(theme))
        .child(section(i18n.text(Message::SettingsData), theme))
        .child(data_action_row(
            handle.clone(), SettingsDataAction::ClearCache, "settings-clear-cache",
            "icons/trash.svg", i18n.text(Message::SettingsClearCache),
            !view.clearing_cache, theme,
        ))
        .child(data_action_row(
            handle.clone(), SettingsDataAction::ResetPreferences, "settings-reset-preferences",
            "icons/rotate-ccw.svg", i18n.text(Message::SettingsResetPreferences),
            true, theme,
        ))
        .child(div().flex_1().min_h(rems(0.75)))
        .child(separator(theme))
        .child(picker(
            view,
            handle.clone(),
            SettingsPicker::About,
            "settings-about-picker",
            "icons/info.svg",
            i18n.text(Message::SettingsAbout),
            theme,
        ))
}
