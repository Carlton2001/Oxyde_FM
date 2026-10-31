use super::rows::{render_sidebar_rows, SidebarRowPresentation};
use super::toolbar::sidebar_toolbar;
use super::{pinned_display_name, sidebar_item_height, sidebar_item_visible, sidebar_row_h};
use crate::ui::app::{DragTarget, OxydeApp};
use crate::ui::components::scrollbar::{render_scrollbar, ScrollbarAxis};
use crate::ui::i18n::I18nContext;
use crate::ui::scale::logical;
use crate::ui::theme::ThemeContext;
use crate::ui::workspace::SidebarItem;
use gpui::prelude::*;
use gpui::*;

pub(super) fn render_sidebar_tree(
    view: &OxydeApp,
    handle: Entity<OxydeApp>,
    window: &mut Window,
    _cx: &mut Context<OxydeApp>,
) -> Div {
    let show_hidden = view.show_hidden;
    let show_system = view.show_system;
    let expandable_drives: std::collections::HashSet<String> = view.state.drives.iter()
        .filter(|drive| view.state.sidebar_nodes.get(&drive.path).is_some_and(|nodes| {
            nodes.iter().any(|node| (show_hidden || !node.is_hidden)
                && (show_system || !node.is_system))
        }))
        .map(|drive| drive.path.clone()).collect();
    // Paths that are genuinely in Quick Access — only these get "Désépingler"
    let pinned_paths: std::collections::HashSet<String> = view
        .state
        .pinned_items
        .iter()
        .map(|i| i.path.clone())
        .collect();
    let sidebar_items: Vec<SidebarItem> = view
        .state
        .sidebar_items
        .iter()
        .filter(|item| sidebar_item_visible(item, show_hidden, show_system, view.show_network()))
        .cloned()
        .collect();
    let theme = *_cx.theme();
    let use_system_icons = cfg!(target_os = "windows") && view.use_system_icons();

    // Dynamic content width (minimum = sidebar width), from the shaped names.
    let font = window.text_style().font();
    let font_size = crate::ui::scale::rem_pixels(rems(0.8125));
    let measure = |text: String| {
        let len = text.len();
        window.text_system().shape_line(text.into(), font_size, &[TextRun {
            len, font: font.clone(), color: theme.text, background_color: None, underline: None, strikethrough: None,
        }], None).width.as_f32()
    };
    let calculated_content_w = sidebar_items
        .iter()
        .map(|item| match item {
            SidebarItem::Header(_) => 0.0,
            SidebarItem::Pinned { item } | SidebarItem::OneDrive { item } => {
                measure(pinned_display_name(item, _cx.i18n()).to_string()) + logical(60.0)
            }
            SidebarItem::Linux { .. } => {
                measure(_cx.i18n().text(crate::ui::i18n::Message::SidebarLinux).to_string()) + logical(60.0)
            }
            SidebarItem::Drive { drive, .. } => {
                measure(crate::ui::format::drive_name(drive, _cx.i18n())) + logical(60.0)
            }
            SidebarItem::Volume { volume } => measure(super::volume_row::name(volume, _cx.i18n())) + logical(60.0),
            SidebarItem::Directory { node, depth, .. } => {
                (*depth as f32 * logical(12.0)) + measure(node.name.clone()) + logical(60.0)
            }
        })
        .fold(view.sidebar_display_width(), |a, b| a.max(b));

    if (calculated_content_w - view.sidebar_content_width).abs() > 1.0 {
        let h = handle.clone();
        _cx.defer(move |cx| {
            h.update(cx, |this, _| {
                this.sidebar_content_width = calculated_content_w;
            });
        });
    }

    let viewport_w = view.sidebar_display_width();
    let content_w = calculated_content_w.max(viewport_w);
    // Reserve scrollbar gutters only when their axis actually overflows.
    let h_overflow = content_w > viewport_w + 1.0;
    let list_top = crate::ui::scale::rem_pixels(crate::ui::style::TAB_BAR_HEIGHT).as_f32();
    let list_bottom = if h_overflow { crate::ui::components::scrollbar::zed_track_width() } else { 0.0 };
    let viewport_h = (view.list_height - list_top - list_bottom).max(1.0);
    let content_h = sidebar_items.iter().map(sidebar_item_height).sum::<f32>().max(1.0);
    let list_right = if content_h > viewport_h { rems(1.125) } else { rems(0.5) };
    // Rows fill the list's visible width until the content itself overflows it.
    let visible_w = viewport_w - crate::ui::scale::rem_pixels(rems(0.5) + list_right).as_f32();
    let row_w = (content_w - logical(26.0)).max(visible_w).max(1.0);

    // Highlight only the folder currently open in the active pane's active tab.
    let selected_path = view
        .state
        .layout
        .find_pane(&view.state.active_pane_id)
        .map(|pane| pane.active_tab().path.clone());
    let show_tooltips = view.context_menus.is_empty();
    let mounting_volume = view.mounting_volume.clone();

    // Clamp before rendering the toolbar so its state matches the visible rows.
    let scroll = view.sidebar_scroll.0.borrow().base_handle.clone();
    let actual_viewport = scroll.bounds().size.height.as_f32();
    let viewport = actual_viewport.max(logical(200.0));
    let max_scroll = (content_h - if actual_viewport > 0.0 { actual_viewport } else { viewport }).max(0.0);
    let offset = (-scroll.offset().y.as_f32()).clamp(0.0, max_scroll);
    if scroll.offset().y.as_f32() != -offset {
        scroll.set_offset(point(scroll.offset().x, px(-offset)));
    }
    let scrolled = offset >= 0.5;

    let mut container = div()
        .w_full()
        .h_full()
        .bg(theme.sidebar)
        .rounded_bl(view.window_corners.bottom_left)
        .on_scroll_wheel(_cx.listener(|_, _, window, cx| {
            crate::ui::components::tooltip::hover::dismiss(window, cx);
            cx.notify();
        }))
        // Inset the virtual list itself so selection backgrounds keep both margins.
        .overflow_hidden()
        .relative()
        .child(
            div()
                .size_full()
                .child(sidebar_toolbar(view, handle.clone(), scrolled, _cx))
                .child(
                    div()
                        .absolute()
                        .top(crate::ui::style::TAB_BAR_HEIGHT)
                        .bottom(px(list_bottom))
                        .left(rems(0.5))
                        .right(list_right) // 12 px scrollbar track + 6 px clear margin when needed.
                        .overflow_hidden()
                        .child(crate::ui::components::tooltip::hover::region_container(
                            handle.clone(), crate::ui::components::tooltip::hover::HoverRegion::DirectoryTree,
                            px(0.0), px(0.0), _cx,
                        ))
                        .child(div()
                        .relative()
                        .h_full()
                        .left(view.sidebar_scroll_h.offset().x)
                        .w(px(row_w))
                        .child({
                            let items_for_list = sidebar_items.clone();
                            let h = handle.clone();
                            let sel = selected_path.clone();
                            let pinned_paths_for_list = pinned_paths.clone();
                            let presentation = SidebarRowPresentation {
                                row_w,
                                pinned_paths: pinned_paths_for_list,
                                expandable_drives,
                                show_tooltips,
                                use_system_icons,
                                mounting_volume: mounting_volume.clone(),
                            };
                            let render_rows = move |range: std::ops::Range<usize>, cx: &mut App| {
                                render_sidebar_rows(range, cx, &items_for_list, &h, &sel, &presentation)
                            };
                            let scroll = scroll.clone();
                            let mut start = 0;
                            let mut top = 0.0;
                            while start < sidebar_items.len() && top + sidebar_item_height(&sidebar_items[start]) < offset {
                                top += sidebar_item_height(&sidebar_items[start]);
                                start += 1;
                            }
                            let mut end = start;
                            let mut bottom = top;
                            while end < sidebar_items.len() && bottom < offset + viewport + sidebar_row_h() {
                                bottom += sidebar_item_height(&sidebar_items[end]);
                                end += 1;
                            }
                            let scroll_handle = handle.clone();
                            div().id("sidebar-list").size_full().flex().flex_col().overflow_y_scroll().track_scroll(&scroll)
                                .on_scroll_wheel(move |_, _, cx| scroll_handle.update(cx, |_, cx| cx.notify()))
                                .child(div().flex_none().h(px(top)))
                                .children(render_rows(start..end, _cx).into_iter().map(|row| div().flex_none().child(row)))
                                .child(div().flex_none().h(px((content_h - bottom).max(0.0))))
                        }))
                )
        );

    // Mark the toolbar edge only once rows have scrolled beneath it.
    if scrolled {
        container = container.child(
            div()
                .absolute()
                .top(crate::ui::style::TAB_BAR_HEIGHT - rems(0.0625))
                .left_0()
                .right_0()
                .h(rems(0.0625))
                .bg(theme.border),
        );
    }

    // Vertical scrollbar
    container = container.child({
        let offset = -view
            .sidebar_scroll
            .0
            .borrow()
            .base_handle
            .offset()
            .y
            .as_f32();
        div()
            .absolute()
            .top(crate::ui::style::TAB_BAR_HEIGHT)
            .bottom(px(list_bottom))
            .right_0()
            .w(rems(0.75))
            .child(render_scrollbar(
                theme,
                handle.clone(),
                ScrollbarAxis::Vertical,
                viewport_h,
                content_h,
                offset,
                matches!(view.drag_target, Some(DragTarget::SidebarV { .. })),
                "sidebar-v".to_string(),
                None,
                |this, track_id, v_h, c_h, _| {
                    this.drag_target = Some(DragTarget::SidebarV {
                        viewport_h: v_h,
                        content_h: c_h,
                        track_id,
                    });
                },
            ))
    });

    // Horizontal scrollbar
    container = container.child(render_scrollbar(
        theme,
        handle.clone(),
        ScrollbarAxis::Horizontal,
        viewport_w,
        content_w,
        -view.sidebar_scroll_h.offset().x.as_f32(),
        matches!(view.drag_target, Some(DragTarget::SidebarH { .. })),
        "sidebar-h".to_string(),
        None,
        |this, track_id, v_w, c_w, _| {
            this.drag_target = Some(DragTarget::SidebarH {
                viewport_w: v_w,
                content_w: c_w,
                track_id,
            });
        },
    ));

    container
}

