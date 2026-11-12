use crate::ui::app::OxydeApp;
use crate::ui::i18n::{I18nContext, Message};
use crate::ui::theme::ThemeContext;
use gpui::prelude::*;
use gpui::*;
use crate::ui::elements::segmented::{segment, segmented};

/// Width of the formats' column, so every level switch starts at the same place.
const FORMAT_COLUMN: Rems = rems(3.0);

/// The default format, then one row per format: its name in a fixed column and
/// its level as a single choice among Fast, Normal and Best.
pub(super) fn compression_table(view: &OxydeApp, handle: Entity<OxydeApp>, window: &mut Window, cx: &mut Context<OxydeApp>) -> Vec<AnyElement> {
    let theme = *cx.theme();
    let mut rows = vec![super::default_format::render(view, handle.clone(), window, cx)];
    for (format, label) in [("zip", "ZIP"), ("7z", "7z"), ("zst", "Zst"), ("gz", "Gz"), ("xz", "Xz"), ("bz2", "Bz2")] {
        let configured = view.compression_quality(format);
        let selected = if configured == "fast" || configured == "best" { configured } else { "normal" };
        let mut levels = segmented(SharedString::from(format!("compression-{format}")), None, theme).flex_1().min_w_0();
        for (quality, message) in [("fast", Message::CompressionFast), ("normal", Message::CompressionNormal), ("best", Message::CompressionBest)] {
            let handle = handle.clone();
            let key_handle = handle.clone();
            let key_action = std::rc::Rc::new(move |_: &mut Window, cx: &mut App| {
                key_handle.update(cx, |view, cx| view.set_compression_quality(format, quality, cx));
            });
            let option = segment(SharedString::from(format!("compression-{format}-{quality}")), selected == quality, true, theme)
                .flex_1()
                .px(rems(0.375))
                .child(div().min_w_0().truncate().child(cx.i18n().text(message)))
                .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                    handle.update(cx, |view, cx| view.set_compression_quality(format, quality, cx));
                });
            levels = levels.child(crate::ui::components::popup::keyboard::item(option, key_action, None, None));
        }
        rows.push(
            div()
                .flex()
                .items_center()
                .gap(rems(0.5))
                .px(rems(0.375))
                .child(div().w(FORMAT_COLUMN).flex_none().child(label))
                .child(levels)
                .into_any_element(),
        );
    }
    rows
}
