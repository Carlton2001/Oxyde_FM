use crate::ui::app::OxydeApp;
use crate::ui::components::tooltip::Tooltip;
use crate::ui::format::{date_format, DateFormat};
use crate::ui::i18n::{FluentArgs, I18nContext, Language, Message};
use crate::ui::theme::{OxydeTheme, ThemeContext, ThemeId};
use gpui::prelude::*;
use gpui::*;
use std::{cell::Cell, rc::Rc};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum SettingsPicker {
    Theme,
    Language,
    Date,
    About,
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
    let themes = [
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
                handle.update(cx, |view, cx| {
                    view.set_theme(id, cx);
                    cx.notify();
                });
            },
        )
        .into_any_element()
    })
    .into_iter()
    .collect();
    let languages = [
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
    let left = div()
        .flex_1()
        .min_w_0()
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
        ));
    let mut version = FluentArgs::new();
    version.set("version", env!("CARGO_PKG_VERSION"));
    let about = vec![div()
        .flex()
        .items_center()
        .gap(rems(0.75))
        .py(rems(0.375))
        .child(img("icons/logo.png").size(rems(2.5)).flex_none())
        .child(
            div()
                .flex()
                .flex_col()
                .gap(rems(0.25))
                .child("Oxyde")
                .child(
                    div()
                        .text_size(rems(0.8125))
                        .text_color(theme.text_muted)
                        .child(i18n.text_with_args(Message::SettingsVersion, &version)),
                )
                .child(
                    div()
                        .text_size(rems(0.6875))
                        .text_color(theme.text_muted)
                        .child(i18n.text(Message::SettingsDescription)),
                ),
        )
        .into_any_element()];
    let right = div()
        .flex_1()
        .min_w_0()
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
        .child(placeholder(
            i18n.text(Message::SettingsTurbo),
            "icons/zap.svg",
            i18n.text(Message::SettingsComingSoon),
            theme,
        ))
        .child(placeholder(
            i18n.text(Message::SettingsSearchLimit),
            "icons/zap.svg",
            i18n.text(Message::SettingsComingSoon),
            theme,
        ))
        .child(separator(theme))
        .child(section(i18n.text(Message::SettingsData), theme))
        .child(placeholder(
            i18n.text(Message::SettingsClearCache),
            "icons/trash.svg",
            i18n.text(Message::SettingsComingSoon),
            theme,
        ))
        .child(placeholder(
            i18n.text(Message::SettingsResetPreferences),
            "icons/rotate-ccw.svg",
            i18n.text(Message::SettingsComingSoon),
            theme,
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
        ));
    let body = div()
        .flex()
        .flex_col()
        .child(div().flex().gap(rems(0.5)).child(left).child(right))
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
        .w(rems(38.0))
        .max_w(window.viewport_size().width - margin * 2.0)
        .max_h(available_height)
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
        .top(origin.y)
        .bottom_0()
        .left_0()
        .right_0()
        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
            cx.stop_propagation();
            backdrop.update(cx, |view, cx| {
                view.settings_open = false;
                view.settings_picker = None;
                view.settings_submenu_bounds = None;
                cx.notify();
            });
        })
        .child(anchored().position(origin).snap_to_window().child(panel));
    if let (Some(kind), Some(bounds)) = (view.settings_picker, view.settings_submenu_bounds) {
        let (title, choices): (SharedString, Vec<AnyElement>) = match kind {
            SettingsPicker::Theme => (i18n.text(Message::SettingsTheme), themes),
            SettingsPicker::Language => (i18n.text(Message::SettingsLanguage), languages),
            SettingsPicker::Date => (i18n.text(Message::SettingsDateFormat), dates),
            SettingsPicker::About => (i18n.text(Message::SettingsAbout), about),
        };
        let submenu_width = crate::ui::scale::rem_pixels(rems(18.0))
            .min(window.viewport_size().width - margin * 2.0);
        let submenu = div()
            .id("settings-submenu")
            .w(submenu_width)
            .max_h(available_height.min(crate::ui::scale::rem_pixels(rems(20.0))))
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

fn icon(path: &'static str, theme: OxydeTheme) -> impl IntoElement {
    svg()
        .path(path)
        .size(rems(0.875))
        .flex_none()
        .text_color(theme.text)
}

fn separator(theme: OxydeTheme) -> impl IntoElement {
    div()
        .h(rems(0.0625))
        .my(rems(0.25))
        .bg(theme.border)
        .flex_none()
}

fn picker(
    view: &OxydeApp,
    handle: Entity<OxydeApp>,
    kind: SettingsPicker,
    id: &'static str,
    icon_path: &'static str,
    label: SharedString,
    theme: OxydeTheme,
) -> impl IntoElement {
    let active = view.settings_picker == Some(kind);
    let row_bounds = Rc::new(Cell::new(None::<Bounds<Pixels>>));
    let paint_bounds = row_bounds.clone();
    let paint_handle = handle.clone();
    div()
        .id(id)
        .relative()
        .h(rems(2.0))
        .px(rems(0.375))
        .flex()
        .items_center()
        .gap(rems(0.5))
        .rounded_md()
        .when(active, |row| row.bg(theme.selected))
        .hover(|s| s.bg(theme.menu_hover))
        .child(
            canvas(
                |bounds, _, _| bounds,
                move |_, bounds, _, cx| {
                    paint_bounds.set(Some(bounds));
                    if active {
                        paint_handle.update(cx, |view, cx| {
                            if view.settings_submenu_bounds != Some(bounds) {
                                view.settings_submenu_bounds = Some(bounds);
                                cx.notify();
                            }
                        });
                    }
                },
            )
            .absolute()
            .size_full(),
        )
        .child(icon(icon_path, theme))
        .child(div().flex_1().min_w_0().child(label))
        .child(icon("icons/chevron-right.svg", theme))
        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
            handle.update(cx, |view, cx| {
                if view.settings_picker == Some(kind) {
                    view.settings_picker = None;
                    view.settings_submenu_bounds = None;
                } else {
                    view.settings_picker = Some(kind);
                    view.settings_submenu_bounds = row_bounds.get();
                }
                cx.notify();
            });
        })
}

fn placeholder(
    label: SharedString,
    icon_path: &'static str,
    status: SharedString,
    theme: OxydeTheme,
) -> impl IntoElement {
    div()
        .min_h(rems(2.0))
        .px(rems(0.375))
        .flex()
        .items_center()
        .gap(rems(0.5))
        .text_color(theme.text_muted)
        .child(icon(icon_path, theme))
        .child(div().flex_1().child(label))
        .child(div().flex_none().text_size(rems(0.6875)).child(status))
}

fn scale_row(view: &OxydeApp, handle: Entity<OxydeApp>, cx: &App) -> impl IntoElement {
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
            .size(rems(1.25))
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
        .min_h(rems(2.0))
        .px(rems(0.375))
        .gap(rems(0.5))
        .flex()
        .items_center()
        .child(icon("icons/type.svg", theme))
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
                        .min_w(rems(2.5))
                        .h(rems(1.25))
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
        .py(rems(0.375))
        .px(rems(0.375))
        .text_size(rems(0.6875))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(theme.text_muted)
        .child(label.to_uppercase())
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
        .px(rems(0.75))
        .py(rems(0.375))
        .flex()
        .items_center()
        .gap(rems(0.5))
        .rounded_md()
        .text_size(rems(0.8125))
        .bg(if selected {
            theme.selected
        } else {
            theme.menu_surface
        })
        .text_color(theme.text)
        .hover(|style| style.bg(theme.menu_hover))
        .child(div().flex_1().child(label))
        .when(selected, |row| row.child(icon("icons/check.svg", theme)))
        .on_mouse_down(MouseButton::Left, move |_, _, cx| on_select(cx))
}

fn settings_row(
    label: SharedString,
    description: SharedString,
    icon_path: &'static str,
    value: bool,
    id: &'static str,
    handle: Entity<OxydeApp>,
    theme: OxydeTheme,
    on_toggle: impl Fn(&mut OxydeApp, &mut Context<OxydeApp>) + 'static,
) -> impl IntoElement {
    div()
        .id(id)
        .min_h(rems(2.0))
        .px(rems(0.375))
        .flex()
        .items_center()
        .gap(rems(0.5))
        .rounded_md()
        .hover(|s| s.bg(theme.control_hover))
        .tooltip(Tooltip::text(description))
        .child(icon(icon_path, theme))
        .child(div().flex_1().min_w_0().text_size(rems(0.8125)).child(label))
        .child(
            div()
                .w(rems(1.875))
                .h(rems(1.125))
                .rounded_full()
                .bg(if value { theme.accent } else { theme.border })
                .flex()
                .items_center()
                .flex_none()
                .px(rems(0.125))
                .when(value, |s| s.justify_end())
                .child(
                    div()
                        .size(rems(0.875))
                        .rounded_full()
                        .bg(theme.surface)
                        .flex_none(),
                ),
        )
        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
            handle.update(cx, |view, cx| on_toggle(view, cx));
        })
}
