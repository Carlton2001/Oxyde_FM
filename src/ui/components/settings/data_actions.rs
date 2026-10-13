use crate::ui::app::OxydeApp;
use crate::ui::i18n::{I18nContext, Message};
use crate::ui::theme::{OxydeTheme, ThemeContext};
use gpui::prelude::*;
use gpui::*;
use crate::ui::app::SettingsDataAction;
use crate::ui::components::modal::{action_button, Modal};
use super::controls::icon;

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

pub(super) fn data_action_row(
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
        .child(div().flex_1().flex_shrink_0().whitespace_nowrap().child(label))
        .on_mouse_down(MouseButton::Left, move |_, window, cx| {
            if enabled {
                crate::ui::components::popup::with_owner(window, cx, |window, cx| handle.update(cx, |view, cx| view.open_settings_confirmation(action, window, cx)));
            }
        })
}
