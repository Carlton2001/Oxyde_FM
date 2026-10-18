use crate::ui::app::OxydeApp;
use crate::ui::components::tooltip::{NativeTooltipExt, Tooltip};
use crate::ui::elements::button::{button_style, ButtonVariant};
use crate::ui::i18n::{I18nContext, Message};
use crate::ui::theme::ThemeContext;
use gpui::prelude::*;
use gpui::*;
use oxyde_lib::systems::file_ops::{FileOperation, FileOperationManager, OpStatus};
use std::sync::Arc;

pub(super) fn render(
    operation: &FileOperation,
    manager: Arc<FileOperationManager>,
    handle: Entity<OxydeApp>,
    cx: &App,
) -> Div {
    let theme = *cx.theme();
    let i18n = cx.i18n();
    let final_state = super::super::is_final(&operation.status);
    let id = operation.id.clone();
    let pause_id = id.clone();
    let turbo_id = id.clone();
    let cancel_id = id.clone();
    let paused = matches!(operation.status, OpStatus::Paused);
    let can_start = paused || matches!(operation.status, OpStatus::Queued);
    let turbo = operation.turbo;
    let pause_manager = manager.clone();
    let turbo_manager = manager.clone();
    let cancel_manager = manager.clone();
    let mut status_row = div()
        .flex()
        .flex_none()
        .items_center()
        .gap(rems(0.25))
        .text_color(theme.text_muted);
    status_row = status_row.child(super::status_badge(operation, cx));
    if final_state {
        let dismiss_id = id.clone();
        let dismiss_handle = handle.clone();
        let dismiss_manager = manager.clone();
        status_row = status_row.child(
            crate::ui::components::tool_window::action_button(
                format!("dismiss-{dismiss_id}"),
                "".into(),
                false,
                cx,
            )
            .child(
                svg()
                    .path("icons/x.svg")
                    .text_color(theme.text)
                    .size(rems(0.875)),
            )
            .child(cx.i18n().text(Message::OpsDismiss))
            .on_click(move |_, _, cx| {
                cx.stop_propagation();
                dismiss_manager.dismiss_operation(&dismiss_id);
                dismiss_handle.update(cx, |app, cx| {
                    app.expanded_operations.remove(&dismiss_id);
                    cx.notify();
                });
            }),
        );
    } else {
        let pause_handle = handle.clone();
        let turbo_handle = handle.clone();
        let cancel_handle = handle.clone();
        let pause_icon = if can_start {
            "icons/play.svg"
        } else {
            "icons/pause.svg"
        };
        let turbo_icon = if turbo {
            "icons/zap.svg"
        } else {
            "icons/zap-off.svg"
        };
        status_row = status_row
            .child(
                button_style(
                    div().id(format!("pause-{pause_id}")),
                    theme,
                    ButtonVariant::Secondary,
                    true,
                )
                .tab_index(0)
                .w(rems(1.75))
                .h(rems(1.75))
                .flex()
                .items_center()
                .justify_center()
                .rounded(rems(0.375))
                .native_tooltip(Tooltip::text(i18n.text(if can_start {
                    Message::OpsResume
                } else {
                    Message::OpsPause
                })))
                .child(svg().path(pause_icon).text_color(theme.text).size_4())
                .on_click(move |_, _, cx| {
                    cx.stop_propagation();
                    pause_manager.set_paused(&pause_id, !can_start);
                    pause_handle.update(cx, |_, cx| cx.notify());
                }),
            )
            .child(
                button_style(
                    div().id(format!("turbo-{turbo_id}")),
                    theme,
                    ButtonVariant::Secondary,
                    true,
                )
                .tab_index(0)
                .w(rems(1.75))
                .h(rems(1.75))
                .flex()
                .items_center()
                .justify_center()
                .rounded(rems(0.375))
                .native_tooltip(Tooltip::text(i18n.text(if turbo {
                    Message::OpsFast
                } else {
                    Message::OpsNormal
                })))
                .child(
                    svg()
                        .path(turbo_icon)
                        .text_color(if turbo {
                            theme.accent
                        } else {
                            theme.text
                        })
                        .size_4(),
                )
                .on_click(move |_, _, cx| {
                    cx.stop_propagation();
                    turbo_manager.set_turbo(&turbo_id, !turbo);
                    turbo_handle.update(cx, |_, cx| cx.notify());
                }),
            )
            .child(
                button_style(
                    div().id(format!("cancel-{cancel_id}")),
                    theme,
                    ButtonVariant::Secondary,
                    true,
                )
                .tab_index(0)
                .w(rems(1.75))
                .h(rems(1.75))
                .flex()
                .items_center()
                .justify_center()
                .rounded(rems(0.375))
                .native_tooltip(Tooltip::text(i18n.text(Message::OpsStop)))
                .child(
                    svg()
                        .path("icons/x.svg")
                        .text_color(theme.text)
                        .size_4(),
                )
                .on_click(move |_, _, cx| {
                    cx.stop_propagation();
                    cancel_manager.cancel_operation(&cancel_id);
                    cancel_handle.update(cx, |_, cx| cx.notify());
                }),
            );
    }

    status_row
}
