use crate::ui::app::OxydeApp;
use crate::ui::i18n::{I18nContext, Message};
use crate::ui::theme::ThemeContext;
use gpui::prelude::*;
use gpui::*;
use super::controls::icon;

pub(super) fn compression_table(view: &OxydeApp, handle: Entity<OxydeApp>, window: &mut Window, cx: &mut Context<OxydeApp>) -> Vec<AnyElement> {
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
    let mut rows = vec![super::default_format::render(view, handle.clone(), window, cx), header.into_any_element()];
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
            let key_handle = handle.clone();
            let key_action = std::rc::Rc::new(move |_: &mut Window, cx: &mut App| { key_handle.update(cx, |view, cx| view.set_compression_quality(format, quality, cx)); });
            row = row.child(
                crate::ui::components::popup::keyboard::item(div()
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
                    }), key_action, None, None),
            );
        }
        rows.push(row.into_any_element());
    }
    rows
}
