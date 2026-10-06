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
    view: &OxydeApp,
    handle: Entity<OxydeApp>,
    is_maximized: bool,
    cx: &mut Context<OxydeApp>,
) -> impl IntoElement {
    let theme = *cx.theme();

    let btn_hover_bg: Hsla = theme.control_hover;
    let icon_color: Hsla = theme.text_muted;

    let action_button = |id: &'static str, icon: &'static str, label: Message| {
        div().id(id).w(rems(2.25)).h_full().flex().items_center()
            .justify_center().flex_none()
            .hover(move |s| s.bg(btn_hover_bg))
            .tooltip(Tooltip::with_shortcut(
                cx.i18n().text(label),
                if label == Message::Refresh { Some("F5".into()) } else { None },
            ))
            .child(svg().path(icon).size_4().text_color(icon_color))
    };
    let refresh_handle = handle.clone();
    let refresh = action_button("title-refresh", "icons/refresh-cw.svg", Message::Refresh)
        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
            refresh_handle.update(cx, |view, cx| view.refresh_all(cx));
        });
    let panel_handle = handle.clone();
    let new_panel = action_button("title-new-panel", "icons/plus.svg", Message::NewPanel)
        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
            panel_handle.update(cx, |view, cx| view.add_panel_right(cx));
        });
    let tools_handle = handle.clone();
    let tools_measure_handle = handle.clone();
    let tools = action_button("title-tools", "icons/wrench.svg", Message::Tools)
        .relative()
        .child(canvas(|bounds, _, _| bounds, move |_, bounds, _, cx| {
            tools_measure_handle.update(cx, |view, _| view.tools_button_bounds = Some(bounds));
        }).absolute().size_full())
        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
            tools_handle.update(cx, |view, cx| {
                let Some(bounds) = view.tools_button_bounds else { return; };
                use crate::ui::app::{ContextMenuContent, ContextMenuInstance};
                use crate::ui::components::dropdown::DropdownItem;
                let items = [Message::ToolSizes, Message::ToolSearch, Message::ToolDuplicates]
                    .into_iter().map(|message| {
                        let search_handle = tools_handle.clone();
                        let mut item = DropdownItem::new(cx.i18n().text(message).to_string(), move |window, cx| {
                            if message == Message::ToolSearch {
                                search_handle.update(cx, |view, cx| view.open_advanced_search(window, cx));
                            }
                        });
                        item.is_dimmed = message != Message::ToolSearch;
                        item
                    }).collect();
                view.settings_open = false;
                view.context_menus.clear();
                view.context_menus.push(ContextMenuInstance {
                    position: point(bounds.right(), bounds.bottom()),
                    content: ContextMenuContent::Items(items, None),
                    scroll_handle: UniformListScrollHandle::new(),
                    width: px(0.0), bottom_padding: 0.0, open_left: true,
                    hovered_item: None, native_request_id: None,
                });
                cx.notify();
            });
        });

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
        .child(img("icons/logo.png").w(rems(1.25)).h(rems(1.25)).flex_none())
        .child(
            div()
                .text_size(rems(0.8125))
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
            crate::trace_shutdown_request("title bar close button");
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
        .child(refresh)
        .child(new_panel)
        .child(tools)
        .child(crate::ui::components::operations::render_operations_footer(view, handle.clone(), cx))
        .child(div().w(rems(0.5)).h_full().flex_none())
        .child(minimize_btn)
        .child(maximize_btn)
        .child(close_btn)
}
