//! The submenu a picker row opens beside the panel.
use super::{controls::choice, measure::Measure, SettingsPicker};
use crate::ui::app::OxydeApp;
use crate::ui::format::{date_format, DateFormat};
use crate::ui::i18n::{I18nContext, Language, Message};
use crate::ui::theme::ThemeContext;
use gpui::prelude::*;
use gpui::*;

/// Each language named in itself, so it can be found from any other.
pub(super) const LANGUAGES: [(Language, &str); 5] = [
    (Language::French, "Français"),
    (Language::English, "English"),
    (Language::German, "Deutsch"),
    (Language::Spanish, "Español"),
    (Language::Italian, "Italiano"),
];

pub(super) const DATE_FORMATS: [(DateFormat, Message); 3] = [
    (DateFormat::Us, Message::DateFormatUs),
    (DateFormat::European, Message::DateFormatEuropean),
    (DateFormat::Iso, Message::DateFormatIso),
];

fn contents(view: &OxydeApp, handle: Entity<OxydeApp>, kind: SettingsPicker, window: &mut Window, cx: &mut Context<OxydeApp>) -> Vec<AnyElement> {
    let theme = *cx.theme();
    match kind {
        SettingsPicker::Language => LANGUAGES.map(|(id, label)| {
            let handle = handle.clone();
            choice(id.as_str(), label.into(), view.language == id, theme, move |cx| {
                handle.update(cx, |view, cx| {
                    view.set_language(id, cx);
                    cx.notify();
                });
            })
            .into_any_element()
        }).into(),
        SettingsPicker::Date => {
            let current = date_format(cx);
            DATE_FORMATS.map(|(format, label)| {
                let handle = handle.clone();
                choice(format.as_str(), cx.i18n().text(label), current == format, theme, move |cx| {
                    handle.update(cx, |view, cx| view.set_date_format(format, cx));
                })
                .into_any_element()
            }).into()
        }
        // Its format list opens from here, as a surface of this submenu.
        SettingsPicker::Compression => super::compression::compression_table(view, handle, window, cx),
        SettingsPicker::Integration => super::integration::integration_choices(view, handle, cx),
        SettingsPicker::About => vec![super::about::about(view, handle, cx)],
    }
}

/// The submenu's surface, as wide as its content needs and scrolling only past the
/// height available.
pub(super) fn render(
    view: &OxydeApp,
    handle: Entity<OxydeApp>,
    kind: SettingsPicker,
    window: &mut Window,
    cx: &mut Context<OxydeApp>,
) -> Stateful<Div> {
    let theme = *cx.theme();
    let width = Measure::new(window, cx.i18n(), theme).submenu(kind);
    let choices = contents(view, handle, kind, window, cx);
    let available = crate::ui::components::popup::available_size(window, cx);
    let margin = crate::ui::scale::rem_pixels(rems(0.375)) * 2.0;
    let height = (available.height - margin).max(px(1.0));
    crate::ui::components::select_menu::surface(theme)
        .id("settings-submenu")
        .w(crate::ui::scale::rem_pixels(rems(width)).min(available.width - margin))
        .max_h(height)
        .on_any_mouse_down(|_, _, cx| cx.stop_propagation())
        .on_scroll_wheel(|_, _, cx| cx.stop_propagation())
        // Reaching the submenu keeps it, whatever rows the pointer crossed on the way.
        .on_mouse_move(|_, _, cx| crate::ui::components::popup::hover::cancel(cx))
        .child(crate::ui::components::scroll_area::scroll_area("settings-submenu-scroll-state",
            div().max_h((height - window.rem_size()).max(px(1.0))),
            div().id("settings-submenu-scroll").children(choices),
            false, window, cx))
}
