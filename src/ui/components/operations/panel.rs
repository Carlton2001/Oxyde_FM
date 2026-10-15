use crate::ui::app::OxydeApp;
use crate::ui::i18n::{I18nContext, Message};
use crate::ui::theme::ThemeContext;
use gpui::prelude::*;
use gpui::*;

pub(super) fn render_operations_panel(
    app: &OxydeApp,
    handle: Entity<OxydeApp>,
    cx: &mut Context<OxydeApp>,
) -> AnyElement {
    let theme = *cx.theme();
    let operations = app.file_ops.list_operations();
    let finished = operations.iter().any(|op| super::is_final(&op.status));
    let manager = app.file_ops.clone();
    let clear_handle = handle.clone();
    let scroll = app.operations_panel_scroll.clone();
    let viewport = scroll.bounds().size.height.as_f32();
    let content = viewport + scroll.max_offset().y.as_f32();
    let scrollbar = super::super::scrollbar::render_scrollbar(
        theme,
        handle.clone(),
        super::super::scrollbar::ScrollbarAxis::Vertical,
        viewport,
        content,
        -scroll.offset().y.as_f32(),
        matches!(
            app.drag_target,
            Some(crate::ui::app::DragTarget::OperationsPanelV { .. })
        ),
        "operations-window".into(),
        None,
        |app, track_id, viewport_h, content_h, _| {
            app.drag_target = Some(crate::ui::app::DragTarget::OperationsPanelV {
                track_id,
                viewport_h,
                content_h,
            });
        },
    );
    let cards = operations
        .into_iter()
        .rev()
        .map(|op| super::card::render_operation_card(app, op, handle.clone(), cx))
        .collect::<Vec<_>>();
    div()
        .flex_1()
        .min_h_0()
        .flex()
        .flex_col()
        .child(
            div()
                .relative()
                .flex_1()
                .min_h_0()
                .child(
                    div()
                        .id("operations-list")
                        .size_full()
                        .flex()
                        .flex_col()
                        .gap(rems(0.75))
                        .p(rems(0.75))
                        .pr(rems(1.5))
                        .overflow_y_scroll()
                        .track_scroll(&app.operations_panel_scroll)
                        .when(cards.is_empty(), |el| {
                            el.child(
                                div()
                                    .p(rems(1.0))
                                    .text_color(theme.text_muted)
                                    .child(cx.i18n().text(Message::OpsEmpty)),
                            )
                        })
                        .children(cards),
                )
                .child(
                    div()
                        .absolute()
                        .right_0()
                        .top_0()
                        .bottom_0()
                        .w(px(super::super::scrollbar::zed_track_width()))
                        .child(scrollbar),
                ),
        )
        .child(
            super::super::tool_window::footer(cx)
                .when(finished, |footer| {
                    footer.child(
                        super::super::tool_window::action_button(
                            "operations-clear",
                            "".into(),
                            false,
                            cx,
                        )
                        .child(
                            svg()
                                .path("icons/broom.svg")
                                .size(rems(0.875))
                                .flex_none()
                                .text_color(theme.text_muted),
                        )
                        .child(cx.i18n().text(Message::OpsDismissAll))
                        .on_click(move |_, _, cx| {
                            manager.dismiss_finished_operations();
                            clear_handle.update(cx, |app, cx| {
                                app.expanded_operations.clear();
                                cx.notify();
                            });
                        }),
                    )
                })
                .child(
                    super::super::tool_window::action_button(
                        "operations-close",
                        cx.i18n().text(Message::OpsHidePanel),
                        false,
                        cx,
                    )
                    .on_click(|_, window, cx| {
                        super::super::tool_window::close_transients(window, cx);
                        window.remove_window();
                    }),
                ),
        )
        .into_any_element()
}
