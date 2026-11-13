use crate::ui::components::confirmation;
use crate::ui::app::OxydeApp;
use crate::ui::i18n::{I18nContext, Message};
use crate::ui::theme::{OxydeTheme, ThemeContext};
use gpui::prelude::*;
use gpui::*;
use crate::ui::app::SettingsDataAction;
use crate::ui::components::modal::Modal;
use crate::ui::elements::button::ButtonVariant;
use super::controls::icon;

pub(crate) fn render_settings_confirmation(
    view: &OxydeApp,
    handle: Entity<OxydeApp>,
    _window: &mut Window,
    cx: &mut Context<OxydeApp>,
) -> impl IntoElement {
    let confirmation = view.settings_confirmation.as_ref().expect("settings confirmation");
    let reset = confirmation.action == SettingsDataAction::ResetPreferences;
    let title = if reset { Message::SettingsResetPreferences } else { Message::SettingsClearCache };
    let prompt = if reset { Message::SettingsResetConfirm } else { Message::SettingsClearCacheConfirm };
    let icon_path = if reset { "icons/rotate-ccw.svg" } else { "icons/trash.svg" };
    // Resetting loses choices; the cache is rebuilt on its own.
    let tone = if reset { confirmation::Tone::Danger } else { confirmation::Tone::Neutral };
    let theme = *cx.theme();
    let (keyboard, cancel, confirm, close) = (handle.clone(), handle.clone(), handle.clone(), handle);
    // Cancel holds the keyboard first.
    div()
        .absolute()
        .inset_0()
        .on_key_down(move |event, window, cx| {
            keyboard.update(cx, |view, cx| match event.keystroke.key.as_str() {
                "escape" => view.resolve_settings_confirmation(false, window, cx),
                "tab" => {
                    if let Some(confirmation) = &view.settings_confirmation {
                        let order = [confirmation.focus.clone(), confirmation.confirm_focus.clone()];
                        crate::ui::components::dialog::cycle(&order, event.keystroke.modifiers.shift, window, cx);
                    }
                }
                _ => {}
            });
            cx.stop_propagation();
        })
        .child(
            Modal::new("settings-data-confirmation", cx.i18n().text(title),
                confirmation::body(icon_path, tone, cx.i18n().text(prompt), Vec::new(), None, theme))
                .title_icon(icon_path)
                .on_close(move |window, cx| {
                    close.update(cx, |view, cx| view.resolve_settings_confirmation(false, window, cx));
                })
                .action(
                    crate::ui::components::dialog::button("settings-confirm-cancel", cx.i18n().text(Message::Cancel), ButtonVariant::Secondary)
                        .focus(&confirmation.focus)
                        .on_press(move |window, cx| {
                            cancel.update(cx, |view, cx| view.resolve_settings_confirmation(false, window, cx));
                        }),
                )
                .action(
                    crate::ui::components::dialog::button("settings-confirm-accept", cx.i18n().text(title), tone.variant())
                        .focus(&confirmation.confirm_focus)
                        .on_press(move |window, cx| {
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
    let action = std::rc::Rc::new(move |window: &mut Window, cx: &mut App| { if enabled { crate::ui::components::popup::with_owner(window, cx, |window, cx| handle.update(cx, |view, cx| view.open_settings_confirmation(action, window, cx))); } });
    let pointer = action.clone();
    let row = div().id(id).min_h(crate::ui::style::MENU_ITEM_HEIGHT).px(rems(0.375))
        .flex().items_center().gap(rems(0.5)).rounded(rems(0.3125))
        .when(enabled, |row| row.hover(|s| s.bg(theme.menu_hover)))
        .when(!enabled, |row| row.opacity(0.5))
        .child(icon(icon_path, theme))
        .child(div().flex_1().flex_shrink_0().whitespace_nowrap().child(label))
        .on_mouse_down(MouseButton::Left, move |_, window, cx| pointer(window, cx));
    if enabled { crate::ui::components::popup::keyboard::item(row, action, None, None).into_any_element() } else { row.into_any_element() }
}
