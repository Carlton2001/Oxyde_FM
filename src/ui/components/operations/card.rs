mod controls;
mod graph;
mod metrics;

use super::is_final;
use crate::ui::app::OxydeApp;
use crate::ui::format::format_number;
use crate::ui::i18n::{FluentArgs, I18nContext, Message};
use crate::ui::theme::{OxydeTheme, ThemeContext};
use gpui::prelude::*;
use gpui::*;
use oxyde_lib::systems::file_ops::{FileOpType, FileOperation, OpStatus};

fn status_color(operation: &FileOperation, theme: OxydeTheme) -> Hsla {
    match operation.status {
        OpStatus::Error(_) => theme.error,
        OpStatus::Completed => theme.toast_success,
        OpStatus::Paused => theme.warning_text,
        OpStatus::Cancelled => theme.text_muted,
        _ => theme.accent,
    }
}

fn status_badge(operation: &FileOperation, cx: &App) -> Div {
    let theme = *cx.theme();
    let color = status_color(operation, theme);
    let state = match operation.status {
        OpStatus::Queued => Message::OpsQueued,
        OpStatus::Calculating => Message::OpsCalculating,
        OpStatus::Running => Message::OpsRunning,
        OpStatus::Paused => Message::OpsPaused,
        OpStatus::Cancelled => Message::OpsCancelled,
        OpStatus::Completed => Message::OpsCompleted,
        OpStatus::Error(_) => Message::OpsError,
    };
    div()
        .flex_none()
        .flex()
        .items_center()
        .gap(rems(0.375))
        .px(rems(0.5))
        .py(rems(0.25))
        .rounded(rems(0.375))
        .bg(color.opacity(0.10))
        .text_color(color)
        .text_size(rems(0.75))
        .font_weight(FontWeight::MEDIUM)
        .child(div().size(rems(0.3125)).rounded_full().bg(color))
        .child(cx.i18n().text(state))
}

pub(super) fn render_operation_card(
    app: &OxydeApp,
    operation: FileOperation,
    handle: Entity<OxydeApp>,
    cx: &mut Context<OxydeApp>,
) -> AnyElement {
    let theme = *cx.theme();
    let final_state = is_final(&operation.status);
    let expanded = !final_state || app.expanded_operations.contains(&operation.id);
    let color = status_color(&operation, theme);
    let (kind, icon) = match operation.op_type {
        FileOpType::Copy => (Message::OpCopy, "icons/copy.svg"),
        FileOpType::Move => (Message::OpMove, "icons/arrow-right.svg"),
        FileOpType::Delete => (Message::OpDelete, "icons/delete.svg"),
        FileOpType::Trash => (Message::OpDelete, "icons/trash.svg"),
        FileOpType::Compress => (Message::OpCompress, "icons/archive.svg"),
        FileOpType::Extract => (Message::OpExtract, "icons/archive-restore.svg"),
    };
    let first_name = operation.sources.first().map(|source| {
        let mut name = source
            .file_name()
            .unwrap_or(source.as_os_str())
            .to_string_lossy()
            .into_owned();
        if operation.sources.len() > 1 {
            name.push('…');
        }
        name
    });
    let mut args = FluentArgs::new();
    args.set(
        "done",
        format_number(operation.processed_files as u64, cx.i18n().language()),
    );
    args.set(
        "total",
        format_number(operation.total_files as u64, cx.i18n().language()),
    );
    args.set("count", operation.total_files as i64);
    let items = cx.i18n().text_with_args(Message::OpsItems, &args);
    let id = operation.id.clone();
    let manager = app.file_ops.clone();
    let toggle = handle.clone();
    let header = div()
        .id(format!("operation-toggle-{id}"))
        .px(rems(0.75))
        .py(rems(0.625))
        .flex()
        .items_center()
        .gap(rems(0.625))
        .bg(theme.header)
        .rounded_t(rems(0.5625))
        .when(!expanded, |header| header.rounded_b(rems(0.5625)))
        .when(expanded, |header| {
            header.border_b(rems(0.0625)).border_color(theme.border)
        })
        .when(final_state, |header| {
            header
                .tab_index(0)
                .hover(|style| style.bg(theme.hover))
                .focus_visible(|style| style.border(rems(0.0625)).border_color(theme.accent))
                .on_click(move |_, _, cx| {
                    if !manager
                        .get_operation(&id)
                        .is_some_and(|op| is_final(&op.status))
                    {
                        return;
                    }
                    toggle.update(cx, |app, cx| {
                        if !app.expanded_operations.remove(&id) {
                            app.expanded_operations.insert(id.clone());
                        }
                        cx.notify();
                    });
                })
        })
        .when(final_state, |header| {
            header.child(
                svg()
                    .path(if expanded {
                        "icons/chevron-down.svg"
                    } else {
                        "icons/chevron-right.svg"
                    })
                    .size(rems(0.875))
                    .flex_none()
                    .text_color(theme.icon_muted),
            )
        })
        .child(
            div()
                .size(rems(2.125))
                .flex_none()
                .flex()
                .items_center()
                .justify_center()
                .rounded(rems(0.5))
                .bg(color.opacity(0.10))
                .child(svg().path(icon).size(rems(1.125)).text_color(color)),
        )
        .child(
            div()
                .flex_none()
                .flex()
                .flex_col()
                .gap(rems(0.125))
                .child(
                    div()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(cx.i18n().text(kind)),
                )
                .child(
                    div()
                        .text_size(rems(0.75))
                        .text_color(theme.text_muted)
                        .child(items),
                ),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .px(rems(0.375))
                .when_some(first_name, |name, value| {
                    name.child(
                        div()
                            .truncate()
                            .font_weight(FontWeight::MEDIUM)
                            .child(value),
                    )
                }),
        )
        .child(controls::render(
            &operation,
            app.file_ops.clone(),
            handle.clone(),
            cx,
        ));
    let card = div()
        .id(format!("operation-{}", operation.id))
        .flex_none()
        .flex()
        .flex_col()
        .rounded(rems(0.625))
        .overflow_hidden()
        .border(rems(0.0625))
        .border_color(if final_state {
            theme.border
        } else {
            theme.accent.opacity(0.45)
        })
        .bg(theme.surface)
        .child(header);
    if !expanded {
        return card.into_any_element();
    }
    let mut body = div().p(rems(0.875)).flex().flex_col().gap(rems(0.875));
    body = body.child(metrics::render(&operation, cx));
    if let Some(destination) = &operation.destination {
        let mut args = FluentArgs::new();
        args.set("path", crate::ui::format::display_paths(&destination.to_string_lossy()));
        body = body.child(
            div()
                .flex()
                .items_start()
                .gap(rems(0.5))
                .child(
                    svg()
                        .path("icons/folder.svg")
                        .size(rems(0.9375))
                        .mt(rems(0.0625))
                        .flex_none()
                        .text_color(theme.icon_muted),
                )
                .child(
                    div()
                        .min_w_0()
                        .text_color(theme.text_muted)
                        .child(cx.i18n().text_with_args(Message::OpsTo, &args)),
                ),
        );
    }
    if let OpStatus::Error(error) = &operation.status {
        body = body.child(
            div()
                .p(rems(0.625))
                .rounded(rems(0.375))
                .bg(theme.error.opacity(0.08))
                .text_color(theme.error)
                .child(crate::ui::format::error_detail(error, cx.i18n())),
        );
    }
    body = body.child(super::files::preview(
        &operation,
        app.use_system_icons(),
        handle,
        cx,
    ));
    card.child(body).into_any_element()
}
