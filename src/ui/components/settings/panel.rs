//! The panel's surface: both columns, then a save or integration error.
use super::{aims_right, columns, measure::{Measure, COLUMN_GAP}};
use crate::ui::app::OxydeApp;
use crate::ui::components::popup::{self, hover};
use crate::ui::elements::button::{action, button_style, ButtonVariant};
use crate::ui::i18n::{I18nContext, Message};
use crate::ui::theme::{OxydeTheme, ThemeContext};
use gpui::prelude::*;
use gpui::*;

/// A failure, in the rows' margins, with Retry when saving failed.
fn error_row(error: Message, handle: Entity<OxydeApp>, theme: OxydeTheme, cx: &App) -> impl IntoElement {
    div()
        .px(rems(0.375))
        .py(rems(0.5))
        .flex()
        .items_center()
        .gap(rems(0.75))
        .text_color(theme.error)
        .child(div().flex_1().min_w_0().child(cx.i18n().text(error)))
        .when(error == Message::SettingsSaveError, |row| {
            row.child(action(button_style(div().id("settings-retry-save"), theme, ButtonVariant::Secondary, true))
                .child(cx.i18n().text(Message::Retry))
                .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                    cx.stop_propagation();
                    handle.update(cx, |view, cx| view.save_preferences(cx));
                }))
        })
}

pub(super) fn render(view: &OxydeApp, handle: Entity<OxydeApp>, window: &mut Window, cx: &mut Context<OxydeApp>) -> Stateful<Div> {
    let theme = *cx.theme();
    let measure = Measure::new(window, cx.i18n(), theme);
    let left_width = measure.left(&columns::left_rows(cx));
    let right_width = measure.right(&columns::right_rows());
    let width = left_width + right_width + COLUMN_GAP + crate::ui::components::select_menu::chrome().0;
    let body = div()
        .flex()
        .flex_col()
        .child(div().flex().gap(rems(COLUMN_GAP))
            .child(columns::left_column(view, handle.clone(), left_width, cx))
            .child(columns::right_column(view, handle.clone(), right_width, cx)))
        .when_some(view.settings_error, |body, error| body.child(error_row(error, handle.clone(), theme, cx)));
    let available = popup::available_size(window, cx);
    let margin = crate::ui::scale::rem_pixels(rems(0.375));
    let top = view
        .settings_button_bounds
        .map_or(px(crate::ui::components::title_bar::titlebar_h()), |button| button.bottom());
    let height = (available.height - top - margin).max(px(1.0));
    // The menus' surface: corner, padding, outline and shadow.
    crate::ui::components::select_menu::surface(theme)
        .id("settings-panel")
        // Resting on a row without a submenu closes the one open; its own rows stop the move.
        .on_mouse_move(move |event, window, cx| {
            let previous = hover::moved(event.position, cx);
            if handle.read(cx).settings_picker.is_none() {
                return;
            }
            // Heading for the submenu, beside its row: each such move restarts the
            // delay, so crossing rows on the way keeps it and only resting closes it.
            if previous.is_some_and(|from| aims_right(from, event.position)) {
                hover::cancel(cx);
            }
            let close = handle.clone();
            hover::intend("settings-close", window, cx, move |_, cx| {
                close.update(cx, |view, cx| {
                    view.close_settings_submenu();
                    cx.notify();
                });
            });
        })
        .w(rems(width))
        .max_w(available.width - margin * 2.0)
        .max_h(height)
        .on_any_mouse_down(|_, _, cx| cx.stop_propagation())
        .on_scroll_wheel(|_, _, cx| cx.stop_propagation())
        .child(crate::ui::components::scroll_area::scroll_area("settings-panel-scroll-state",
            div().max_h((height - window.rem_size()).max(px(1.0))),
            div().id("settings-panel-scroll").child(body), true, window, cx))
}
