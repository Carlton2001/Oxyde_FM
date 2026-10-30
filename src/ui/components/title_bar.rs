use crate::ui::components::client_decorations::ClientDecorationsExt;
use crate::ui::components::tooltip::NativeTooltipExt;
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
    window: &Window,
    cx: &mut Context<OxydeApp>,
) -> impl IntoElement {
    let theme = *cx.theme();

    let btn_hover_bg: Hsla = theme.control_hover;
    let icon_color: Hsla = theme.text;

    let action_button = |id: &'static str, icon: &'static str, label: Message| {
        let bounds = std::rc::Rc::new(std::cell::Cell::new(Bounds::<Pixels>::default()));
        let measured = bounds.clone();
        div().id(id).w(rems(2.25)).h_full().flex().items_center()
            .justify_center().flex_none().relative()
            .hover(move |s| s.bg(btn_hover_bg))
            .native_tooltip(Tooltip::anchored(Tooltip::with_shortcut(
                cx.i18n().text(label),
                match label { Message::Refresh => Some("F5 / Ctrl+R".into()), Message::NewPanel => Some("Ctrl+N".into()), _ => None },
            ), bounds))
            .child(canvas(move |rect, _, _| measured.set(rect), |_, _, _, _| {}).absolute().left_0().top_0().size_full())
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
    let viewer_handle = handle.clone();
    let viewer_label = if view.viewer_open() { Message::ViewerClose } else { Message::ViewerOpen };
    let viewer = action_button("title-viewer", "icons/eye.svg", viewer_label)
        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
            viewer_handle.update(cx, |view, cx| view.toggle_viewer(cx));
        });
    let tools_handle = handle.clone();
    let tools_measure_handle = handle.clone();
    let tools = action_button("title-tools", "icons/search.svg", Message::Tools)
        .w_auto().px(rems(0.5)).gap(rems(0.125))
        .child(crate::ui::elements::button::menu_chevron(false, theme.text_muted))
        .relative()
        .child(canvas(|bounds, _, _| bounds, move |_, bounds, _, cx| {
            tools_measure_handle.update(cx, |view, _| view.tools_button_bounds = Some(bounds));
        }).absolute().size_full())
        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
            tools_handle.update(cx, |view, cx| {
                if view.close_menu_owned_by("title-tools", cx) { return; }
                let Some(bounds) = view.tools_button_bounds else { return; };
                use crate::ui::app::{ContextMenuContent, ContextMenuInstance};
                use crate::ui::components::dropdown::DropdownItem;
                let items = [Message::ToolSearch, Message::ToolDuplicates]
                    .into_iter().map(|message| {
                        let search_handle = tools_handle.clone();
                        let mut item = DropdownItem::new(cx.i18n().text(message).to_string(), move |window, cx| {
                            if message == Message::ToolDuplicates {
                                search_handle.update(cx, |view, cx| view.open_duplicates(window, cx));
                            }
                            if message == Message::ToolSearch {
                                search_handle.update(cx, |view, cx| view.open_advanced_search(window, cx));
                            }
                        });
                        if message == Message::ToolSearch { item.icon = Some(crate::ui::components::dropdown::IconName::Search); }
                        if message == Message::ToolDuplicates { item.icon = Some(crate::ui::components::dropdown::IconName::Copy); }
                        item
                    }).collect();
                view.settings_open = false;
                view.context_menus.clear();
                view.context_menus.push(ContextMenuInstance {
            native_bounds: None,
                    position: point(bounds.right(), bounds.bottom()),
                    content: ContextMenuContent::Items(items, None),
                    scroll_handle: UniformListScrollHandle::new(),
                    width: px(0.0), bottom_padding: 0.0, open_left: true,
                    hovered_item: None, native_request_id: None, open_up: false,
                });
                view.mark_menu_owner("title-tools");
                cx.notify();
            });
        });

    // ── Oxyde app button (left) → opens Settings directly ───────────────────
    let h_menu = handle.clone();
    let measure_handle = handle.clone();
    let app_bounds = std::rc::Rc::new(std::cell::Cell::new(Bounds::<Pixels>::default()));
    let app_measured = app_bounds.clone();
    let app_button = div()
        .id("oxyde-app-btn")
        .relative()
        .native_tooltip(Tooltip::anchored(Tooltip::text(cx.i18n().text(Message::SettingsTitle)), app_bounds.clone()))
        .h_full()
        .px_3()
        .flex()
        .items_center()
        .gap_2()
        .rounded_none()
        .rounded_tl(crate::ui::components::client_decorations::inner_corners(window).top_left)
        .flex_none()
        .hover(move |s| s.bg(btn_hover_bg))
        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
            h_menu.update(cx, |this, cx| {
                this.context_menus.clear();
                this.settings_open = !this.settings_open;
                this.settings_about_open = false;
                this.settings_picker = None;
                this.settings_submenu_bounds = None;
                cx.notify();
            });
        })
        .child(
            canvas(
                |bounds, _, _| bounds,
                move |_, bounds, _, cx| {
                    app_measured.set(bounds);
                    measure_handle.update(cx, |this, _| this.settings_button_bounds = Some(bounds));
                },
            )
            .absolute()
            .size_full(),
        )
        .child(img("icons/logo_title.png").w(rems(1.25)).h(rems(1.25)).flex_none())
        .child(
            div()
                .text_size(rems(0.8125))
                .font_weight(FontWeight::BOLD)
                .text_color(theme.text)
                .child("Oxyde"),
        );

    // ── Draggable center area ────────────────────────────────────────────────
    let drag_area = div()
        .id("titlebar-drag-area")
        .flex_1()
        .h_full()
        .window_control_area(WindowControlArea::Drag)
        .client_drag_area()
        .on_mouse_down(MouseButton::Middle, |_, window, _| {
            window.minimize_window();
        });

    // ── Assemble ─────────────────────────────────────────────────────────────
    let corners = crate::ui::components::client_decorations::inner_corners(window);
    let is_maximized = window.is_maximized();
    crate::ui::components::client_decorations::round(div(), corners, crate::ui::components::client_decorations::TOP)
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
        .child(viewer)
        .child(tools)
        .child(crate::ui::components::operations::render_operations_footer(view, handle.clone(), cx))
        .child(div().w(rems(0.5)).h_full().flex_none())
        .child(render_window_controls(is_maximized, corners.top_right, cx, move |window, cx| {
            close_main_window(&handle, "title bar close button", window, cx);
        }))
}

/// Close the main window the way its close button does, ending the application.
pub(crate) fn close_main_window(handle: &Entity<OxydeApp>, reason: &'static str, window: &mut Window, cx: &mut App) {
    handle.update(cx, |view, cx| view.stop_viewers(cx));
    crate::trace_shutdown_request(reason);
    crate::ui::app::duplicates::close_duplicate_windows(cx);
    crate::ui::components::operations::close_operations_windows(cx);
    crate::ui::app::advanced_search::close_advanced_search_windows(cx);
    crate::ui::app::viewer::close_viewer_windows(cx);
    window.remove_window();
}

/// Shared title-bar controls. The caller owns its close lifecycle.
/// Dedicated caption symbols use a 12-unit grid and a one-unit stroke,
/// independently of the Lucide assets and their global stroke setting.
pub fn render_window_controls(
    is_maximized: bool,
    corner: Pixels,
    cx: &App,
    on_close: impl Fn(&mut Window, &mut App) + 'static,
) -> impl IntoElement {
    render_window_controls_with_options(is_maximized, true, corner, cx, on_close)
}

pub(crate) fn render_window_controls_with_options(
    is_maximized: bool,
    allow_resize: bool,
    corner: Pixels,
    cx: &App,
    on_close: impl Fn(&mut Window, &mut App) + 'static,
) -> impl IntoElement {
    let theme = *cx.theme();
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
            .hover(move |s| s.bg(theme.control_hover))
            .on_mouse_down(MouseButton::Left, |_, window, _| {
                window.minimize_window();
            })
            .child(
                svg()
                    .path("visuals/window-controls/minimize.svg")
                    .size_3()
                    .text_color(theme.text),
            )
    };

    let maximize_icon = if is_maximized {
        "visuals/window-controls/restore.svg"
    } else {
        "visuals/window-controls/maximize.svg"
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
        .hover(move |s| s.bg(theme.control_hover))
        .on_mouse_down(MouseButton::Left, |_, window, _| {
            window.zoom_window();
        })
        .child(svg().path(maximize_icon).size_3().text_color(theme.text));

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
        .rounded_tr(corner)
        .hover(|s| s.bg(theme.danger_hover))
        .on_mouse_down(MouseButton::Left, move |_, window, cx| {
            on_close(window, cx);
        })
        .child(
            svg()
                .path("visuals/window-controls/close.svg")
                .size_3()
                .text_color(theme.text)
                .group_hover("window-close", |s| s.text_color(theme.text_on_danger)),
        );

    div().h_full().flex().items_center().flex_none()
        .when(allow_resize, |controls| controls.child(minimize_btn).child(maximize_btn)).child(close_btn)
}
use gpui::prelude::FluentBuilder;
