use crate::ui::app::{OxydeApp, SettingsDataAction};
use crate::ui::components::modal::{action_button, Modal};
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
    let compression = compression_table(view, handle.clone(), cx);
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
        ));
    let body = div()
        .flex()
        .flex_col()
        .child(div().flex().gap(rems(0.5)).child(left).child(right))
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
        .top_0()
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
            SettingsPicker::Compression => (i18n.text(Message::Compression), compression),

        };
        let submenu_width =
            crate::ui::scale::rem_pixels(rems(if kind == SettingsPicker::Compression {
                22.0
            } else {
                18.0
            }))
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

pub(crate) fn render_settings_confirmation(
    view: &OxydeApp,
    handle: Entity<OxydeApp>,
    window: &mut Window,
    cx: &mut Context<OxydeApp>,
) -> impl IntoElement {
    let confirmation = view.settings_confirmation.as_ref().expect("settings confirmation");
    let reset = confirmation.action == SettingsDataAction::ResetPreferences;
    let title = if reset { Message::SettingsResetPreferences } else { Message::SettingsClearCache };
    let prompt = if reset { Message::SettingsResetConfirm } else { Message::SettingsClearCacheConfirm };
    let icon_path = if reset { "icons/rotate-ccw.svg" } else { "icons/trash.svg" };
    let theme = *cx.theme();
    let keyboard = handle.clone();
    let cancel = handle.clone();
    let confirm = handle.clone();
    let close = handle.clone();
    let backdrop = handle;
    div()
        .absolute()
        .inset_0()
        .track_focus(&confirmation.focus)
        .on_key_down(move |event, window, cx| {
            keyboard.update(cx, |view, cx| match event.keystroke.key.as_str() {
                "escape" => view.resolve_settings_confirmation(false, window, cx),
                "tab" => {
                    if let Some(confirmation) = &view.settings_confirmation {
                        let focus = if confirmation.confirm_focus.is_focused(window) {
                            &confirmation.focus
                        } else {
                            &confirmation.confirm_focus
                        };
                        window.focus(focus, cx);
                    }
                }
                "enter" | "space" => {
                    let confirm = view.settings_confirmation.as_ref()
                        .is_some_and(|confirmation| confirmation.confirm_focus.is_focused(window));
                    view.resolve_settings_confirmation(confirm, window, cx);
                }
                _ => {}
            });
            cx.stop_propagation();
        })
        .on_action(|_: &crate::ui::app::Supprimer, _, cx| cx.stop_propagation())
        .on_action(|_: &crate::ui::app::Ouvrir, _, cx| cx.stop_propagation())
        .on_action(|_: &crate::ui::app::Renommer, _, cx| cx.stop_propagation())
        .on_action(|_: &crate::ui::app::Copier, _, cx| cx.stop_propagation())
        .on_action(|_: &crate::ui::app::Couper, _, cx| cx.stop_propagation())
        .on_action(|_: &crate::ui::app::Coller, _, cx| cx.stop_propagation())
        .on_action(|_: &crate::ui::app::ToutSelectionner, _, cx| cx.stop_propagation())
        .on_action(|_: &crate::ui::app::Annuler, _, cx| cx.stop_propagation())
        .on_action(|_: &crate::ui::app::Refaire, _, cx| cx.stop_propagation())
        .on_action(|_: &crate::ui::app::Actualiser, _, cx| cx.stop_propagation())
        .child(
            Modal::new("settings-data-confirmation", cx.i18n().text(title),
                div().child(cx.i18n().text(prompt)))
                .title_icon(icon_path)
                .header(
                    div().w_full().flex().items_center().gap(rems(0.5))
                        .child(svg().path(icon_path).size(rems(1.0)).text_color(theme.text_muted))
                        .child(div().flex_1().text_size(rems(0.875))
                            .font_weight(FontWeight::MEDIUM).child(cx.i18n().text(title)))
                        .child(div().id("settings-confirm-close").p(rems(0.25)).rounded_md()
                            .hover(|d| d.bg(theme.control_hover))
                            .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                                cx.stop_propagation();
                                close.update(cx, |view, cx| view.resolve_settings_confirmation(false, window, cx));
                            })
                            .child(svg().path("icons/x.svg").size(rems(1.0)).text_color(theme.text_muted))),
                )
                .on_backdrop_dismiss(move |window, cx| {
                    backdrop.update(cx, |view, cx| view.resolve_settings_confirmation(false, window, cx));
                })
                .action(
                    action_button("settings-confirm-cancel")
                        .bg(theme.control_hover)
                        .border(rems(0.0625))
                        .border_color(if confirmation.focus.is_focused(window) { theme.accent } else { theme.control_hover })
                        .child(cx.i18n().text(Message::Cancel))
                        .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                            cancel.update(cx, |view, cx| view.resolve_settings_confirmation(false, window, cx));
                        }),
                )
                .action(
                    action_button("settings-confirm-accept")
                        .track_focus(&confirmation.confirm_focus)
                        .bg(theme.accent).text_color(theme.text_on_accent)
                        .hover(|d| d.opacity(0.85))
                        .border(rems(0.0625))
                        .border_color(if confirmation.confirm_focus.is_focused(window) { theme.text } else { theme.accent })
                        .child(cx.i18n().text(title))
                        .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                            confirm.update(cx, |view, cx| view.resolve_settings_confirmation(true, window, cx));
                        }),
                ),
        )
}

fn data_action_row(
    handle: Entity<OxydeApp>,
    action: SettingsDataAction,
    id: &'static str,
    icon_path: &'static str,
    label: SharedString,
    enabled: bool,
    theme: OxydeTheme,
) -> impl IntoElement {
    div().id(id).min_h(rems(2.0)).px(rems(0.375))
        .flex().items_center().gap(rems(0.5)).rounded_md()
        .when(enabled, |row| row.hover(|s| s.bg(theme.menu_hover)))
        .when(!enabled, |row| row.opacity(0.5))
        .child(icon(icon_path, theme))
        .child(div().flex_1().min_w_0().child(label))
        .on_mouse_down(MouseButton::Left, move |_, window, cx| {
            if enabled {
                handle.update(cx, |view, cx| view.open_settings_confirmation(action, window, cx));
            }
        })
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

fn scale_row(view: &OxydeApp, handle: Entity<OxydeApp>, cx: &App) -> impl IntoElement {
    use crate::ui::scale::{BASE_REM_SIZE, MAX_REM_SIZE, MIN_REM_SIZE};
    let theme = *cx.theme();
    let reference = view.ui_rem_size();
    let mut args = FluentArgs::new();
    args.set("reference", reference);
    let reset = handle.clone();
    let value = div().id("scale-reset").min_w(rems(3.25)).h(rems(1.5)).flex().items_center().justify_center()
        .tooltip(Tooltip::text(cx.i18n().text(Message::SettingsScaleReset)))
        .child(cx.i18n().text_with_args(Message::SettingsScaleValue, &args))
        .on_mouse_down(MouseButton::Left, move |_, window, cx| {
            cx.stop_propagation();
            reset.update(cx, |view, cx| view.set_ui_rem_size(BASE_REM_SIZE, window, cx));
        });
    div().min_h(rems(2.0)).px(rems(0.375)).gap(rems(0.5)).flex().items_center()
        .child(icon("icons/type.svg", theme))
        .child(div().flex_1().min_w_0().child(cx.i18n().text(Message::SettingsScale)))
        .child(crate::ui::elements::stepper::stepper(
            "scale", value, reference > MIN_REM_SIZE, reference < MAX_REM_SIZE, theme,
            move |increase, window, cx| {
                handle.update(cx, |view, cx| {
                    let reference = view.ui_rem_size();
                    let next = if increase { reference + 1 } else { reference.saturating_sub(1) };
                    view.set_ui_rem_size(next, window, cx);
                });
            },
        ))
}

fn search_limit_row(view: &OxydeApp, handle: Entity<OxydeApp>, cx: &App) -> impl IntoElement {
    use oxyde_lib::models::preferences::{SEARCH_LIMIT_STEP, MAX_SEARCH_LIMIT};
    let theme = *cx.theme();
    let limit = view.search_limit();
    div().id("settings-search-limit-row").min_h(rems(2.0)).px(rems(0.375)).gap(rems(0.5)).flex().items_center()
        .tooltip(Tooltip::text(cx.i18n().text(Message::SettingsSearchLimitDescription)))
        .child(icon("icons/zap.svg", theme))
        .child(div().flex_1().min_w_0().child(cx.i18n().text(Message::SettingsSearchLimit)))
        .child(crate::ui::elements::stepper::stepper(
            "search-limit", limit.to_string(), limit > SEARCH_LIMIT_STEP, limit < MAX_SEARCH_LIMIT, theme,
            move |increase, _, cx| {
                handle.update(cx, |view, cx| view.step_search_limit(increase, cx));
            },
        ))
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

fn compression_table(view: &OxydeApp, handle: Entity<OxydeApp>, cx: &App) -> Vec<AnyElement> {
    let theme = *cx.theme();
    let i18n = cx.i18n();
    let header = div()
        .flex()
        .gap(rems(0.25))
        .border_b(rems(0.0625))
        .border_color(theme.border)
        .children(
            [
                Message::CompressionFormat,
                Message::CompressionFast,
                Message::CompressionNormal,
                Message::CompressionBest,
            ]
            .into_iter()
            .map(|message| {
                div()
                    .flex_1()
                    .min_w_0()
                    .px(rems(0.375))
                    .py(rems(0.25))
                    .text_color(theme.text_muted)
                    .text_center()
                    .child(i18n.text(message))
            }),
        );
    let mut rows = vec![header.into_any_element()];
    for (format, label) in [("zip", "ZIP"), ("7z", "7z"), ("zst", "Zst")] {
        let configured = view.compression_quality(format);
        let selected = if configured == "fast" || configured == "best" {
            configured
        } else {
            "normal"
        };
        let mut row = div().flex().gap(rems(0.25)).mt(rems(0.25)).items_center().child(
            div()
                .flex_1()
                .min_w_0()
                .px(rems(0.375))
                .py(rems(0.25))
                .text_center()
                .child(label),
        );
        for quality in ["fast", "normal", "best"] {
            let handle = handle.clone();
            let active = selected == quality;
            row = row.child(
                div()
                    .id(format!("compression-{format}-{quality}"))
                    .flex_1()
                    .min_w_0()
                    .h(rems(1.75))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded_md()
                    .bg(if active {
                        theme.selected
                    } else {
                        theme.menu_surface
                    })
                    .hover(|style| style.bg(theme.menu_hover))
                    .child(
                        div()
                            .size(rems(1.0))
                            .when(active, |cell| cell.child(icon("icons/check.svg", theme))),
                    )
                    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                        handle.update(cx, |view, cx| {
                            view.set_compression_quality(format, quality, cx)
                        });
                    }),
            );
        }
        rows.push(row.into_any_element());
    }
    rows
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
