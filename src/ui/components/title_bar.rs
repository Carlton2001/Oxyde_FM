use crate::ui::app::OxydeApp;
use crate::ui::components::tooltip::Tooltip;
use crate::ui::i18n::{I18nContext, Message};
use crate::ui::scale::rem_pixels;
use crate::ui::theme::ThemeContext;
use gpui::*;

pub const TITLEBAR_H: Rems = rems(2.25);
pub fn titlebar_h() -> f32 {
    rem_pixels(TITLEBAR_H).as_f32()
}

pub fn render_title_bar(
    _view: &OxydeApp,
    handle: Entity<OxydeApp>,
    is_maximized: bool,
    cx: &mut Context<OxydeApp>,
) -> impl IntoElement {
    let theme = *cx.theme();

    let btn_hover_bg: Hsla = theme.control_hover;
    let icon_color: Hsla = theme.text_muted;

    // ── Oxyde app button (left) → opens Settings directly ───────────────────
    let h_menu = handle.clone();
    let measure_handle = handle.clone();
    let app_button = div()
        .id("oxyde-app-btn")
        .relative()
        .tooltip(Tooltip::text(cx.i18n().text(Message::SettingsTitle)))
        .h_full()
        .px_3()
        .flex()
        .items_center()
        .gap_2()
        .rounded_none()
        .flex_none()
        .hover(move |s| s.bg(btn_hover_bg))
        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
            h_menu.update(cx, |this, cx| {
                this.context_menus.clear();
                this.settings_open = !this.settings_open;
                this.settings_picker = None;
                this.settings_submenu_bounds = None;
                cx.notify();
            });
        })
        .child(
            canvas(
                |bounds, _, _| bounds,
                move |_, bounds, _, cx| {
                    measure_handle.update(cx, |this, _| this.settings_button_bounds = Some(bounds));
                },
            )
            .absolute()
            .size_full(),
        )
        .child(img("icons/logo.png").w(rems(1.0)).h(rems(1.0)).flex_none())
        .child(
            div()
                .text_size(rems(0.75))
                .font_weight(FontWeight::BOLD)
                .text_color(theme.text)
                .child("Oxyde GPUI"),
        );

    // ── Draggable center area ────────────────────────────────────────────────
    let drag_area = div()
        .id("titlebar-drag-area")
        .flex_1()
        .h_full()
        .window_control_area(WindowControlArea::Drag)
        .on_mouse_down(MouseButton::Middle, |_, window, _| {
            window.minimize_window();
        });

    // ── Window controls (right) ──────────────────────────────────────────────
    let minimize_btn = {
        div()
            .id("wc-minimize")
            .w(rems(2.875))
            .h_full()
            .flex()
            .items_center()
            .justify_center()
            .flex_none()
            .window_control_area(WindowControlArea::Min)
            .hover(move |s| s.bg(btn_hover_bg))
            .on_mouse_down(MouseButton::Left, |_, window, _| {
                window.minimize_window();
            })
            .child(
                svg()
                    .path("icons/window-minimize.svg")
                    .size_3()
                    .text_color(icon_color),
            )
    };

    let maximize_icon = if is_maximized {
        "icons/window-restore.svg"
    } else {
        "icons/window-maximize.svg"
    };
    let maximize_btn = div()
        .id("wc-maximize")
        .w(rems(2.875))
        .h_full()
        .flex()
        .items_center()
        .justify_center()
        .flex_none()
        .window_control_area(WindowControlArea::Max)
        .hover(move |s| s.bg(btn_hover_bg))
        .on_mouse_down(MouseButton::Left, |_, window, _| {
            window.zoom_window();
        })
        .child(svg().path(maximize_icon).size_3().text_color(icon_color));

    let close_btn = div()
        .id("wc-close")
        .group("window-close")
        .w(rems(2.875))
        .h_full()
        .flex()
        .items_center()
        .justify_center()
        .flex_none()
        .window_control_area(WindowControlArea::Close)
        .hover(|s| s.bg(theme.danger_hover))
        .on_mouse_down(MouseButton::Left, |_, window, _| {
            window.remove_window();
        })
        .child(
            svg()
                .path("icons/x.svg")
                .size_3()
                .text_color(icon_color)
                .group_hover("window-close", |s| s.text_color(theme.text_on_danger)),
        );

    // ── Assemble ─────────────────────────────────────────────────────────────
    div()
        .id("title-bar")
        .w_full()
        .h(px(titlebar_h()))
        .flex()
        .flex_none()
        .items_center()
        .bg(theme.surface)
        .border_b(rems(0.0625))
        .border_color(theme.border)
        .child(app_button)
        .child(drag_area)
        .child(minimize_btn)
        .child(maximize_btn)
        .child(close_btn)
}
