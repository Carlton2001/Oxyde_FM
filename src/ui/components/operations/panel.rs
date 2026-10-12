use crate::ui::app::OxydeApp;
use crate::ui::components::tooltip::Tooltip;
use crate::ui::i18n::{I18nContext, Message};
use crate::ui::theme::ThemeContext;
use gpui::prelude::*;
use gpui::*;
use super::is_final;

use crate::ui::components::scrollbar::{render_scrollbar, ScrollbarAxis};
use crate::ui::format::format_number;
use crate::ui::i18n::FluentArgs;
use crate::ui::scale::logical;
use super::{panel_width, panel_max_height, OperationsPanelView};
use super::card::render_operation_card;

pub fn render_operations_panel(
    app: &OxydeApp,
    handle: Entity<OxydeApp>,
    panel_handle: Entity<OperationsPanelView>,
    panel_position: Option<Point<Pixels>>,
    panel_bounds: Option<Bounds<Pixels>>,
    is_dragging: bool,
    viewport_size: Size<Pixels>,
    cx: &mut Context<OxydeApp>,
) -> impl IntoElement {
    let theme = *cx.theme();
    let operations = app.file_ops.list_operations();
    let mut title_args = FluentArgs::new();
    title_args.set("count", operations.len() as i64);
    let title = cx.i18n().text_with_args(Message::OpsTitle, &title_args);
    let operation_count = format_number(operations.len() as u64, cx.i18n().language());
    let has_finished_operations = operations
        .iter()
        .any(|operation| is_final(&operation.status));
    let operations_manager = app.file_ops.clone();
    let close_handle = handle.clone();
    let panel_size = panel_bounds
        .map(|bounds| bounds.size)
        .unwrap_or(size(px(panel_width()), px(panel_max_height())));
    let saved_position = panel_position.map(|position| {
        let margin = logical(12.0);
        let max_left =
            (viewport_size.width.as_f32() - panel_size.width.as_f32() - margin).max(margin);
        let max_top =
            (viewport_size.height.as_f32() - panel_size.height.as_f32() - logical(36.0)).max(margin);
        point(
            px(position.x.as_f32().clamp(margin, max_left)),
            px(position.y.as_f32().clamp(margin, max_top)),
        )
    });
    let mut panel = div()
        .id("operations-panel")
        .absolute()
        .w(px(panel_width()))
        .max_h(px(panel_max_height()))
        .flex()
        .flex_col()
        .text_size(rems(0.75))
        .overflow_hidden()
        .rounded_md()
        .border(rems(0.0625))
        .border_color(theme.menu_border)
        .bg(theme.menu_surface)
        .shadow(crate::ui::style::rem_shadow(true))
        .occlude()
        .on_any_mouse_down(|_, _, cx| cx.stop_propagation())
        .on_mouse_move({
            let panel_handle = panel_handle.clone();
            move |event, window, cx| {
                if !event.dragging() {
                    panel_handle.update(cx, |panel, cx| panel.finish_drag(cx));
                    if let Some(app) = panel_handle.read(cx).app.upgrade() {
                        app.update(cx, |app, cx| app.finish_pointer_drag(cx));
                    }
                    return;
                }
                let pointer = event.position;
                let viewport = window.viewport_size();
                panel_handle.update(cx, |this, cx| {
                    this.update_drag(pointer, viewport, cx);
                    this.update_scrollbar_drag(pointer, cx);
                });
            }
        })
        .capture_any_mouse_up({
            let app_handle = handle.clone();
            let panel_handle = panel_handle.clone();
            move |event, _, cx| {
                if event.button == MouseButton::Left {
                    panel_handle.update(cx, |this, cx| {
                        this.finish_drag(cx);
                    });
                    app_handle.update(cx, |app, cx| {
                        if matches!(
                            app.drag_target,
                            Some(crate::ui::app::DragTarget::OperationsPanelV { .. })
                        ) {
                            app.drag_target = None;
                            cx.notify();
                        }
                    });
                }
            }
        })
        .on_mouse_up_out(MouseButton::Left, {
            let app_handle = handle.clone();
            let panel_handle = panel_handle.clone();
            move |_, _, cx| {
                panel_handle.update(cx, |this, cx| {
                    this.finish_drag(cx);
                });
                app_handle.update(cx, |app, cx| {
                    if matches!(
                        app.drag_target,
                        Some(crate::ui::app::DragTarget::OperationsPanelV { .. })
                    ) {
                        app.drag_target = None;
                        cx.notify();
                    }
                });
            }
        })
        .on_scroll_wheel(|_, _, cx| cx.stop_propagation())
        .child({
            let panel_handle = panel_handle.clone();
            canvas(
                |bounds, _, _| bounds,
                move |bounds, _, _, cx| {
                    panel_handle.update(cx, |this, cx| {
                        if this.bounds != Some(bounds) {
                            this.bounds = Some(bounds);
                            cx.notify();
                        }
                    });
                },
            )
            .absolute()
            .size_full()
        })
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .h(rems(2.125))
                .px(rems(0.625))
                .border_b(rems(0.0625))
                .border_color(theme.border)
                .text_size(rems(0.75))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(rems(0.375))
                        .cursor(if is_dragging {
                            CursorStyle::ClosedHand
                        } else {
                            CursorStyle::OpenHand
                        })
                        .child(
                            svg()
                                .path("icons/download.svg")
                                .text_color(theme.text)
                                .size_4(),
                        )
                        .child(operation_count)
                        .child(title),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(rems(0.25))
                        .when(has_finished_operations, |actions| {
                            let manager = operations_manager.clone();
                            let handle = panel_handle.clone();
                            actions.child(
                                div()
                                    .id("operations-panel-clear")
                                    .w(rems(1.5))
                                    .h(rems(1.5))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .rounded_sm()
                                    .hover(|style| style.bg(theme.control_hover))
                                    .tooltip(Tooltip::text(cx.i18n().text(Message::OpsDismissAll)))
                                    .child(
                                        svg()
                                            .path("icons/broom.svg")
                                            .text_color(theme.icon_muted)
                                            .size_4(),
                                    )
                                    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                        cx.stop_propagation();
                                        manager.dismiss_finished_operations();
                                        handle.update(cx, |_, cx| cx.notify());
                                    }),
                            )
                        })
                        .child(
                            div()
                                .id("operations-panel-close")
                                .w(rems(1.5))
                                .h(rems(1.5))
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded_sm()
                                .hover(|style| style.bg(theme.control_hover))
                                .tooltip(Tooltip::text(cx.i18n().text(Message::OpsHidePanel)))
                                .child(
                                    svg()
                                        .path("icons/chevron-down.svg")
                                        .text_color(theme.icon_muted)
                                        .size_4(),
                                )
                                .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                    cx.stop_propagation();
                                    close_handle.update(cx, |this, cx| {
                                        this.operations_panel_open = false;
                                        this.operations_panel_opened_manually = false;
                                        cx.notify();
                                    });
                                }),
                        ),
                )
                .cursor(if is_dragging {
                    CursorStyle::ClosedHand
                } else {
                    CursorStyle::OpenHand
                })
                .on_mouse_down(MouseButton::Left, {
                    let drag_handle = panel_handle.clone();
                    move |event, _, cx| {
                        cx.stop_propagation();
                        let pointer = event.position;
                        drag_handle.update(cx, |this, cx| {
                            if let Some(bounds) = this.bounds {
                                this.drag_offset = Some(point(
                                    pointer.x - bounds.left(),
                                    pointer.y - bounds.top(),
                                ));
                                this.last_drag_update = None;
                                cx.notify();
                            }
                        });
                    }
                }),
        );
    if let Some(position) = saved_position {
        panel = panel.left(position.x).top(position.y);
    } else {
        panel = panel.right(rems(0.75)).bottom(rems(2.25));
    }
    if operations.is_empty() {
        panel = panel.child(
            div()
                .p(rems(0.75))
                .text_color(theme.text_muted)
                .child(cx.i18n().text(Message::OpsEmpty)),
        );
    } else {
        let cards = operations
            .into_iter()
            .rev()
            .enumerate()
            .map(|(index, operation)| {
                render_operation_card(
                    app,
                    operation,
                    index % 2 == 1,
                    app.handle.clone().expect("handle set"),
                    cx,
                )
            })
            .collect::<Vec<_>>();
        let scroll_handle = app.operations_panel_scroll.clone();
        let viewport_h = scroll_handle.bounds().size.height.as_f32();
        let content_h = viewport_h + scroll_handle.max_offset().y.as_f32();
        let scroll_offset = -scroll_handle.offset().y.as_f32();
        let scroll_viewport = div()
            .id("operations-list")
            .flex_1()
            .min_h_0()
            .pr(px(crate::ui::components::scrollbar::zed_track_width()))
            .overflow_y_scroll()
            .track_scroll(&scroll_handle)
            .children(cards);
        panel = panel.child(scroll_viewport).child(
            div()
                .absolute()
                .top(rems(2.125))
                .right_0()
                .bottom_0()
                .w(px(crate::ui::components::scrollbar::zed_track_width()))
                .child(render_scrollbar(
                    theme,
                    handle,
                    ScrollbarAxis::Vertical,
                    viewport_h,
                    content_h,
                    scroll_offset,
                    matches!(
                        app.drag_target,
                        Some(crate::ui::app::DragTarget::OperationsPanelV { .. })
                    ),
                    "operations-panel".to_string(),
                    None,
                    |this, track_id, viewport_h, content_h, _| {
                        this.drag_target = Some(crate::ui::app::DragTarget::OperationsPanelV {
                            viewport_h,
                            content_h,
                            track_id,
                        });
                    },
                )),
        );
    }
    panel
}
