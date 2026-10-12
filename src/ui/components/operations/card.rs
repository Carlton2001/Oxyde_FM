use crate::ui::app::OxydeApp;
use crate::ui::components::tooltip::Tooltip;
use crate::ui::format::{format_number, format_size};
use crate::ui::i18n::{FluentArgs, I18nContext, Message};
use crate::ui::scale::{logical, rem_pixels};
use crate::ui::theme::ThemeContext;
use gpui::prelude::*;
use gpui::*;
use oxyde_lib::systems::file_ops::{FileOpType, FileOperation, OpStatus};
use super::is_final;

fn format_elapsed_duration(duration_ms: u64, language: crate::ui::i18n::Language) -> String {
    let total_seconds = duration_ms / 1_000;
    if total_seconds < 60 {
        let tenths = duration_ms.saturating_add(50) / 100;
        let decimal = match language {
            crate::ui::i18n::Language::English => '.',
            _ => ',',
        };
        return format!(
            "{}{decimal}{} s",
            format_number(tenths / 10, language),
            tenths % 10
        );
    }
    if total_seconds < 3_600 {
        return format!(
            "{} min {} s",
            format_number(total_seconds / 60, language),
            format_number(total_seconds % 60, language)
        );
    }
    format!(
        "{} h {} min",
        format_number(total_seconds / 3_600, language),
        format_number((total_seconds % 3_600) / 60, language)
    )
}

fn speed_graph(samples: Vec<u64>, accent: Hsla) -> impl IntoElement {
    canvas(
        |bounds, _, _| bounds,
        move |_, bounds: Bounds<Pixels>, window, _| {
            let width = bounds.size.width.as_f32();
            let height = bounds.size.height.as_f32();
            if samples.is_empty() || width <= logical(2.0) || height <= logical(4.0) {
                return;
            }
            let left = bounds.origin.x.as_f32() + logical(1.0);
            let right = left + width - logical(2.0);
            let top = bounds.origin.y.as_f32() + logical(2.0);
            let bottom = top + height - logical(4.0);
            let max_speed = samples.iter().copied().max().unwrap_or(0).max(1) as f64;
            let points: Vec<_> = if samples.len() == 1 {
                let y = bottom - (samples[0] as f64 / max_speed) as f32 * (bottom - top);
                vec![point(px(left), px(y)), point(px(right), px(y))]
            } else {
                samples
                    .iter()
                    .enumerate()
                    .map(|(index, speed)| {
                        let x = left + (right - left) * index as f32 / (samples.len() - 1) as f32;
                        let y = bottom - (*speed as f64 / max_speed) as f32 * (bottom - top);
                        point(px(x), px(y))
                    })
                    .collect()
            };
            let mut area = PathBuilder::fill();
            area.move_to(point(px(left), px(bottom)));
            for point in &points {
                area.line_to(*point);
            }
            area.line_to(point(px(right), px(bottom)));
            area.close();
            if let Ok(path) = area.build() {
                window.paint_path(path, accent.opacity(0.08));
            }
            let mut line = PathBuilder::stroke(rem_pixels(rems(0.09375)));
            line.move_to(points[0]);
            for point in points.iter().skip(1) {
                line.line_to(*point);
            }
            if let Ok(path) = line.build() {
                window.paint_path(path, accent.opacity(0.28));
            }
        },
    )
    .absolute()
    .inset_0()
}

pub(super) fn render_operation_card(
    app: &OxydeApp,
    operation: FileOperation,
    alternate_background: bool,
    handle: Entity<OxydeApp>,
    cx: &mut Context<OxydeApp>,
) -> AnyElement {
    let theme = *cx.theme();
    let i18n = cx.i18n();
    let language = i18n.language();
    let (kind, icon) = match operation.op_type {
        FileOpType::Copy => (Message::OpCopy, "icons/copy.svg"),
        FileOpType::Move => (Message::OpMove, "icons/arrow-right.svg"),
        FileOpType::Delete => (Message::OpDelete, "icons/delete.svg"),
        FileOpType::Trash => (Message::OpDelete, "icons/trash.svg"),
        FileOpType::Compress => (Message::OpCompress, "icons/archive.svg"),
        FileOpType::Extract => (Message::OpExtract, "icons/archive-restore.svg"),
    };
    let state = match operation.status {
        OpStatus::Queued => Message::OpsQueued,
        OpStatus::Calculating => Message::OpsCalculating,
        OpStatus::Running => Message::OpsRunning,
        OpStatus::Paused => Message::OpsPaused,
        OpStatus::Cancelled => Message::OpsCancelled,
        OpStatus::Completed => Message::OpsCompleted,
        OpStatus::Error(_) => Message::OpsError,
    };
    let final_state = is_final(&operation.status);
    let compact_summary = matches!(operation.status, OpStatus::Completed | OpStatus::Cancelled);
    let completed = matches!(operation.status, OpStatus::Completed);
    let waiting = matches!(operation.status, OpStatus::Queued | OpStatus::Paused);
    let source = operation
        .current_file
        .as_ref()
        .map(std::path::PathBuf::from)
        .or_else(|| operation.sources.first().cloned())
        .unwrap_or_default();
    let mut from_args = FluentArgs::new();
    from_args.set("path", source.to_string_lossy().into_owned());
    let from = i18n.text_with_args(Message::OpsFrom, &from_args);
    let to = operation.destination.as_ref().map(|destination| {
        let mut args = FluentArgs::new();
        args.set("path", destination.to_string_lossy().into_owned());
        i18n.text_with_args(Message::OpsTo, &args)
    });
    let mut items_args = FluentArgs::new();
    items_args.set(
        "done",
        format_number(operation.processed_files as u64, language),
    );
    items_args.set(
        "total",
        format_number(operation.total_files as u64, language),
    );
    let items = i18n.text_with_args(Message::OpsItems, &items_args);
    let ratio = if operation.total_bytes > 0 {
        Some(operation.processed_bytes as f64 / operation.total_bytes as f64)
    } else if operation.total_files > 0 {
        Some(operation.processed_files as f64 / operation.total_files as f64)
    } else {
        None
    };
    let percentage = ratio.map(|value| (value * 100.0).clamp(0.0, 100.0).round() as u32);
    let progress_text = if operation.total_bytes > 0 {
        Some(format!(
            "{} / {}",
            format_size(operation.processed_bytes, false, true, language),
            format_size(operation.total_bytes, false, true, language)
        ))
    } else {
        None
    };
    let displayed_speed = if final_state {
        operation.duration_ms.and_then(|duration_ms| {
            (duration_ms > 0 && operation.processed_bytes > 0)
                .then(|| operation.processed_bytes.saturating_mul(1_000) / duration_ms)
        })
    } else {
        (operation.bytes_per_second > 0).then_some(operation.bytes_per_second)
    };
    let speed = displayed_speed.map(|bytes_per_second| {
        let value = format!("{}/s", format_size(bytes_per_second, false, true, language));
        if final_state {
            let mut args = FluentArgs::new();
            args.set("speed", value);
            i18n.text_with_args(Message::OpsAverage, &args).to_string()
        } else {
            value
        }
    });
    let remaining = if operation.bytes_per_second > 0
        && operation.total_bytes > operation.processed_bytes
        && !final_state
    {
        let seconds =
            (operation.total_bytes - operation.processed_bytes) / operation.bytes_per_second;
        let mut args = FluentArgs::new();
        args.set("seconds", format_number(seconds, language));
        Some(i18n.text_with_args(Message::OpsRemaining, &args))
    } else {
        None
    };
    let id = operation.id.clone();
    let pause_id = id.clone();
    let turbo_id = id.clone();
    let cancel_id = id.clone();
    let paused = matches!(operation.status, OpStatus::Paused);
    let can_start = paused || matches!(operation.status, OpStatus::Queued);
    let turbo = operation.turbo;
    let manager = app.file_ops.clone();
    let pause_manager = manager.clone();
    let turbo_manager = manager.clone();
    let cancel_manager = manager.clone();
    let mut status_row = div()
        .flex()
        .flex_none()
        .items_center()
        .gap(rems(0.25))
        .text_color(theme.text_muted);
    if waiting {
        status_row = status_row.child(
            div()
                .when(waiting, |label| label.text_color(theme.accent))
                .child(i18n.text(state)),
        );
    }
    if final_state {
        let dismiss_id = id.clone();
        let dismiss_handle = handle.clone();
        let dismiss_manager = manager.clone();
        status_row = status_row.child(
            div()
                .id(format!("dismiss-{dismiss_id}"))
                .px(rems(0.3125))
                .h(rems(1.25))
                .flex()
                .items_center()
                .justify_center()
                .gap(rems(0.25))
                .rounded_sm()
                .hover(|style| style.bg(theme.menu_hover).text_color(theme.text))
                .tooltip(Tooltip::text(i18n.text(Message::OpsDismiss)))
                .child(
                    svg()
                        .path("icons/x.svg")
                        .text_color(theme.icon_muted)
                        .size_3(),
                )
                .child(i18n.text(state))
                .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                    dismiss_manager.dismiss_operation(&dismiss_id);
                    dismiss_handle.update(cx, |_, cx| cx.notify());
                }),
        );
    } else {
        let pause_handle = handle.clone();
        let turbo_handle = handle.clone();
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
                div()
                    .id(format!("pause-{pause_id}"))
                    .w(rems(1.375))
                    .h(rems(1.375))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded_sm()
                    .hover(|style| style.bg(theme.control_hover))
                    .tooltip(Tooltip::text(i18n.text(if can_start {
                        Message::OpsResume
                    } else {
                        Message::OpsPause
                    })))
                    .child(svg().path(pause_icon).text_color(theme.text).size_4())
                    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                        pause_manager.set_paused(&pause_id, !can_start);
                        pause_handle.update(cx, |_, cx| cx.notify());
                    }),
            )
            .child(
                div()
                    .id(format!("turbo-{turbo_id}"))
                    .w(rems(1.375))
                    .h(rems(1.375))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded_sm()
                    .hover(|style| style.bg(theme.control_hover))
                    .tooltip(Tooltip::text(i18n.text(if turbo {
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
                                theme.text_muted
                            })
                            .size_4(),
                    )
                    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                        turbo_manager.set_turbo(&turbo_id, !turbo);
                        turbo_handle.update(cx, |_, cx| cx.notify());
                    }),
            )
            .child(
                div()
                    .id(format!("cancel-{cancel_id}"))
                    .w(rems(1.375))
                    .h(rems(1.375))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded_sm()
                    .hover(|style| style.bg(theme.control_hover))
                    .tooltip(Tooltip::text(i18n.text(Message::OpsStop)))
                    .child(
                        svg()
                            .path("icons/x.svg")
                            .text_color(theme.text_muted)
                            .size_4(),
                    )
                    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                        cancel_manager.cancel_operation(&cancel_id);
                        handle.update(cx, |_, cx| cx.notify());
                    }),
            );
    }

    let card = div()
        .id(format!("operation-{id}"))
        .relative()
        .p(rems(0.75))
        .border_b(rems(0.0625))
        .border_color(theme.border)
        .bg(if alternate_background {
            theme.background
        } else {
            theme.menu_surface
        })
        .flex()
        .flex_col()
        .gap(rems(0.3125))
        .when(waiting, |card| {
            card.child(
                div()
                    .absolute()
                    .inset_0()
                    .border_l(rems(0.125))
                    .border_color(theme.accent)
                    .bg(theme.accent.opacity(0.08)),
            )
        })
        .child(
            div()
                .flex()
                .justify_between()
                .items_center()
                .child(
                    div()
                        .flex()
                        .min_w_0()
                        .items_center()
                        .gap(rems(0.375))
                        .child(svg().path(icon).text_color(theme.text).size_4().flex_none())
                        .child(i18n.text(kind))
                        .child(
                            div()
                                .text_color(theme.text_muted)
                                .overflow_hidden()
                                .child(items),
                        ),
                )
                .child(status_row),
        );
    let mut body = div()
        .relative()
        .font_weight(FontWeight::NORMAL)
        .text_color(theme.text_muted)
        .flex()
        .flex_col()
        .gap(rems(0.3125))
        .overflow_hidden();
    if !final_state && !operation.speed_history.is_empty() {
        body = body.child(speed_graph(operation.speed_history.clone(), theme.accent));
    }
    body = body.child(
        div()
            .text_color(theme.text_muted)
            .overflow_hidden()
            .child(from),
    );
    if let Some(to) = to {
        body = body.child(
            div()
                .text_color(theme.text_muted)
                .overflow_hidden()
                .child(to),
        );
    }
    if progress_text.is_some() || (percentage.is_some() && !compact_summary) {
        body = body.child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap(rems(0.5))
                .child(
                    div()
                        .min_w_0()
                        .overflow_hidden()
                        .child(progress_text.unwrap_or_default()),
                )
                .when(completed, |row| {
                    row.when_some(speed.clone(), |row, speed| {
                        row.child(div().flex_none().child(speed))
                    })
                })
                .when(!compact_summary, |row| {
                    row.child(
                        div()
                            .flex_none()
                            .text_color(theme.text)
                            .child(
                                percentage
                                    .map(|value| format!("{value}%"))
                                    .unwrap_or_default(),
                            ),
                    )
                }),
        );
    }
    let speed = speed.filter(|_| !completed);
    let temporal = if final_state {
        operation.duration_ms.map(|duration_ms| {
            let mut args = FluentArgs::new();
            args.set("duration", format_elapsed_duration(duration_ms, language));
            i18n.text_with_args(Message::OpsDuration, &args)
        })
    } else {
        remaining
    };
    if compact_summary {
        body = body.child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap(rems(0.5))
                .child(
                    div()
                        .flex_none()
                        .text_color(theme.text)
                        .child(
                            percentage
                                .unwrap_or(if matches!(operation.status, OpStatus::Completed) {
                                    100
                                } else {
                                    0
                                })
                                .to_string()
                                + "%",
                        ),
                )
                .child(
                    div()
                        .text_color(theme.text_muted)
                        .child(temporal.clone().unwrap_or_default()),
                ),
        );
    }
    if !final_state || speed.is_some() || (temporal.is_some() && !compact_summary) {
        body = body.child(
            div()
                .flex()
                .justify_between()
                .child(speed.unwrap_or_else(|| {
                    if final_state {
                        String::new()
                    } else {
                        i18n.text(Message::OpsSpeedWaiting).to_string()
                    }
                }))
                .when(!compact_summary, |row| {
                    row.child(
                        div()
                            .text_color(theme.text_muted)
                            .child(temporal.unwrap_or_default()),
                    )
                }),
        );
    }
    if let OpStatus::Error(error) = &operation.status {
        body = body.child(div().text_color(theme.error).child(crate::ui::format::error_detail(error, cx.i18n())));
    }
    if let Some(percentage) = percentage.filter(|_| !compact_summary) {
        body = body.child(
            div()
                .w_full()
                .h(rems(0.25))
                .overflow_hidden()
                .bg(theme.control_hover)
                .child(
                    div()
                        .w(relative(percentage as f32 / 100.0))
                        .h_full()
                        .bg(theme.accent.opacity(0.48)),
                ),
        );
    }
    card.child(body).into_any_element()
}
