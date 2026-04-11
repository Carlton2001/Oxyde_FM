use gpui::*;
use crate::ui::app::{OxydeApp, ContextMenuContent, ContextMenuInstance};
use crate::ui::theme::ThemeContext;

pub const TITLEBAR_H: f32 = 36.0;

pub fn render_title_bar(
    _view: &OxydeApp,
    handle: Entity<OxydeApp>,
    is_maximized: bool,
    cx: &mut Context<OxydeApp>,
) -> impl IntoElement {
    let theme = *cx.theme();

    let btn_hover_bg: Hsla = rgba(0xffffff12).into();
    let icon_color: Hsla   = rgba(0xffffffaa).into();

    // ── Oxyde app button (left) ──────────────────────────────────────────────
    let h_menu = handle.clone();
    let app_button = div()
        .id("oxyde-app-btn")
        .h_full()
        .px_3()
        .flex()
        .items_center()
        .gap_2()
        .rounded_none()
        .flex_none()
        .hover(move |s| s.bg(btn_hover_bg))
        .cursor_pointer()
        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
            let _ = h_menu.update(cx, |this, cx| {
                // Toggle: if app menu already open, close it
                if this.context_menus.first()
                    .map(|m| matches!(&m.content, ContextMenuContent::Items(_, _)))
                    .unwrap_or(false)
                {
                    this.context_menus.clear();
                    cx.notify();
                    return;
                }
                this.context_menus.clear();
                let items = crate::ui::components::app_menu_items(this.handle.clone().unwrap());
                this.context_menus.push(ContextMenuInstance {
                    position: Point::new(px(0.0), px(TITLEBAR_H)),
                    content: ContextMenuContent::Items(items, None),
                    scroll_handle: UniformListScrollHandle::new(),
                    width: px(0.0),
                    open_left: false,
                });
                cx.notify();
            });
        })
        .child(
            img("icons/logo.png")
                .w(px(16.0))
                .h(px(16.0))
                .flex_none()
        )
        .child(
            div()
                .text_size(px(12.0))
                .font_weight(FontWeight::MEDIUM)
                .text_color(theme.text)
                .child("Oxyde")
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
            .w(px(46.0))
            .h_full()
            .flex()
            .items_center()
            .justify_center()
            .flex_none()
            .window_control_area(WindowControlArea::Min)
            .hover(move |s| s.bg(btn_hover_bg))
            .cursor_pointer()
            .on_mouse_down(MouseButton::Left, |_, window, _| {
                window.minimize_window();
            })
            .child(
                svg()
                    .path("icons/window-minimize.svg")
                    .size_3()
                    .text_color(icon_color)
            )
    };

    let maximize_icon = if is_maximized {
        "icons/window-restore.svg"
    } else {
        "icons/window-maximize.svg"
    };
    let maximize_btn = div()
        .id("wc-maximize")
        .w(px(46.0))
        .h_full()
        .flex()
        .items_center()
        .justify_center()
        .flex_none()
        .window_control_area(WindowControlArea::Max)
        .hover(move |s| s.bg(btn_hover_bg))
        .cursor_pointer()
        .on_mouse_down(MouseButton::Left, |_, window, _| {
            window.zoom_window();
        })
        .child(
            svg()
                .path(maximize_icon)
                .size_3()
                .text_color(icon_color)
        );

    let close_btn = div()
        .id("wc-close")
        .w(px(46.0))
        .h_full()
        .flex()
        .items_center()
        .justify_center()
        .flex_none()
        .window_control_area(WindowControlArea::Close)
        .hover(|s| s.bg(rgb(0xc42b1c)))
        .cursor_pointer()
        .on_mouse_down(MouseButton::Left, |_, window, _| {
            window.remove_window();
        })
        .child(
            svg()
                .path("icons/x.svg")
                .size_3()
                .text_color(icon_color)
        );

    // ── Assemble ─────────────────────────────────────────────────────────────
    div()
        .id("title-bar")
        .w_full()
        .h(px(TITLEBAR_H))
        .flex()
        .flex_none()
        .items_center()
        .bg(theme.surface)
        .border_b_1()
        .border_color(theme.border)
        .child(app_button)
        .child(drag_area)
        .child(minimize_btn)
        .child(maximize_btn)
        .child(close_btn)
}
