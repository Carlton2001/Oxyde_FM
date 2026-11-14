//! Without native surfaces, the panel and its submenu are drawn in the main window,
//! over a backdrop that closes them on a click.
use super::{panel, submenu, submenu_anchor};
use crate::ui::app::OxydeApp;
use crate::ui::i18n::I18nContext;
use crate::ui::theme::ThemeContext;
use gpui::prelude::*;
use gpui::*;

pub(super) fn render(view: &OxydeApp, handle: Entity<OxydeApp>, window: &mut Window, cx: &mut Context<OxydeApp>) -> AnyElement {
    let origin = view
        .settings_button_bounds
        .map_or(point(px(0.0), px(crate::ui::components::title_bar::titlebar_h())), |button| point(px(0.0), button.bottom()));
    let backdrop = handle.clone();
    let mut overlay = div()
        .absolute()
        .inset_0()
        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
            cx.stop_propagation();
            backdrop.update(cx, |view, cx| {
                view.close_settings();
                cx.notify();
            });
        })
        .child(anchored().position(origin).snap_to_window().child(panel::render(view, handle.clone(), window, cx)));
    if let (Some(kind), Some(bounds)) = (view.settings_picker, submenu_anchor(view)) {
        let submenu = submenu::render(view, handle, kind, window, cx);
        let margin = crate::ui::scale::rem_pixels(rems(0.375));
        let width = crate::ui::components::popup::available_size(window, cx).width;
        // Opened on the side that has room for it.
        let submenu_width = crate::ui::scale::rem_pixels(rems(super::measure::Measure::new(window, cx.i18n(), *cx.theme()).submenu(kind)));
        let open_left = bounds.right() + submenu_width + margin > width && bounds.left() - submenu_width >= margin;
        overlay = overlay.child(
            anchored()
                .position(point(if open_left { bounds.left() } else { bounds.right() }, bounds.top()))
                .anchor(if open_left { Anchor::TopRight } else { Anchor::TopLeft })
                .snap_to_window()
                .child(submenu),
        );
    }
    overlay.into_any_element()
}
