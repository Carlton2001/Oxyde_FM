use crate::ui::app::OxydeApp;
use crate::ui::components::tooltip::Tooltip;
use crate::ui::format::{format_number, format_size};
use crate::ui::i18n::{FluentArgs, I18nContext, Message};
use crate::ui::theme::ThemeContext;
use gpui::prelude::*;
use gpui::*;
use oxyde_lib::systems::file_ops::{FileOpType, FileOperation, OpStatus};

pub fn render_operations_footer(
    app: &OxydeApp,
    handle: Entity<OxydeApp>,
    cx: &mut Context<OxydeApp>,
) -> impl IntoElement {
    let theme = *cx.theme();
    let operations = app.file_ops.list_operations();
    let active = operations.iter().filter(|op| !is_final(&op.status)).count();
    let count = format_number(active as u64, cx.i18n().language());
    let tooltip = cx.i18n().text(if app.operations_panel_open {
        Message::OpsHidePanel
    } else {
        Message::OpsShowPanel
    });
    div()
        .id("operations-footer")
        .absolute()
        .left(px(8.0))
        .bottom(px(5.0))
        .h(px(28.0))
        .flex()
        .items_center()
        .gap(px(6.0))
        .px(px(8.0))
        .rounded_sm()
        .text_color(theme.text_muted)
        .occlude()
        .tooltip(Tooltip::text(tooltip))
        .hover(|style| style.bg(theme.control_hover).text_color(theme.text))
        .child(
            svg()
                .path("icons/download.svg")
                .text_color(theme.text_muted)
                .size_4(),
        )
        .child(count)
        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
            handle.update(cx, |this, cx| {
                this.operations_panel_open = !this.operations_panel_open;
                cx.notify();
            });
        })
}

pub fn render_operations_panel(
    app: &OxydeApp,
    handle: Entity<OxydeApp>,
    cx: &mut Context<OxydeApp>,
) -> impl IntoElement {
    let theme = *cx.theme();
    let operations = app.file_ops.list_operations();
    let title = cx.i18n().text(Message::OpsTitle);
    let mut panel = div()
        .id("operations-panel")
        .absolute()
        .right(px(12.0))
        .bottom(px(36.0))
        .w(px(360.0))
        .max_h(px(420.0))
        .flex()
        .flex_col()
        .text_size(px(12.0))
        .overflow_hidden()
        .rounded_md()
        .border_1()
        .border_color(theme.menu_border)
        .bg(theme.menu_surface)
        .shadow_lg()
        .occlude()
        .on_any_mouse_down(|_, _, cx| cx.stop_propagation())
        .on_scroll_wheel(|_, _, cx| cx.stop_propagation())
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .h(px(34.0))
                .px(px(10.0))
                .border_b_1()
                .border_color(theme.border)
                .text_size(px(12.0))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(6.0))
                        .child(
                            svg()
                                .path("icons/download.svg")
                                .text_color(theme.text)
                                .size_4(),
                        )
                        .child(title),
                )
                .child(
                    div()
                        .id("operations-panel-close")
                        .w(px(24.0))
                        .h(px(24.0))
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
                            handle.update(cx, |this, cx| {
                                this.operations_panel_open = false;
                                cx.notify();
                            });
                        }),
                ),
        );
    if operations.is_empty() {
        panel = panel.child(
            div()
                .p(px(12.0))
                .text_color(theme.text_muted)
                .child(cx.i18n().text(Message::OpsEmpty)),
        );
    } else {
        let cards = operations
            .into_iter()
            .rev()
            .map(|operation| {
                render_operation_card(app, operation, app.handle.clone().expect("handle set"), cx)
            })
            .collect::<Vec<_>>();
        panel = panel.child(
            div()
                .id("operations-list")
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .children(cards),
        );
    }
    panel
}

fn is_final(status: &OpStatus) -> bool {
    matches!(
        status,
        OpStatus::Completed | OpStatus::Cancelled | OpStatus::Error(_)
    )
}

fn format_elapsed_duration(duration_ms: u64, language: crate::ui::i18n::Language) -> String {
    let total_seconds = duration_ms / 1_000;
    if total_seconds < 60 {
        let tenths = duration_ms.saturating_add(50) / 100;
        let decimal = match language {
            crate::ui::i18n::Language::French => ',',
            crate::ui::i18n::Language::English => '.',
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
            if samples.is_empty() || width <= 2.0 || height <= 4.0 {
                return;
            }
            let left = bounds.origin.x.as_f32() + 1.0;
            let right = left + width - 2.0;
            let top = bounds.origin.y.as_f32() + 2.0;
            let bottom = top + height - 4.0;
            let max_speed = samples.iter().copied().max().unwrap_or(0).max(1) as f64;
            let points: Vec<_> = if samples.len() == 1 {
                let y = bottom - (samples[0] as f64 / max_speed) as f32 * (bottom - top);
                vec![point(px(left), px(y)), point(px(right), px(y))]
            } else {
                samples.iter().enumerate().map(|(index, speed)| {
                    let x = left + (right - left) * index as f32 / (samples.len() - 1) as f32;
                    let y = bottom - (*speed as f64 / max_speed) as f32 * (bottom - top);
                    point(px(x), px(y))
                }).collect()
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
            let mut line = PathBuilder::stroke(px(1.5));
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

fn render_operation_card(
    app: &OxydeApp,
    operation: FileOperation,
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
    let unmanaged_extraction = matches!(operation.op_type, FileOpType::Extract)
        && operation.sources.first().is_some_and(|path| !path.extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("zip")))
        && matches!(operation.status, OpStatus::Running | OpStatus::Paused);
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
        let value = format!(
            "{}/s",
            format_size(bytes_per_second, false, true, language)
        );
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
    let turbo = operation.turbo;
    let manager = app.file_ops.clone();
    let pause_manager = manager.clone();
    let turbo_manager = manager.clone();
    let cancel_manager = manager.clone();
    let mut status_row = div()
        .flex()
        .flex_none()
        .items_center()
        .gap(px(4.0))
        .text_color(theme.text_muted);
    if final_state || unmanaged_extraction {
        status_row = status_row.child(i18n.text(state));
    }
    if final_state {
        let dismiss_id = id.clone();
        let dismiss_handle = handle.clone();
        let dismiss_manager = manager.clone();
        status_row = status_row.child(
            div()
                .id(format!("dismiss-{dismiss_id}"))
                .w(px(20.0))
                .h(px(20.0))
                .flex()
                .items_center()
                .justify_center()
                .rounded_sm()
                .hover(|style| style.bg(theme.control_hover))
                .tooltip(Tooltip::text(i18n.text(Message::OpsDismiss)))
                .child(svg().path("icons/x.svg").text_color(theme.icon_muted).size_3())
                .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                    dismiss_manager.dismiss_operation(&dismiss_id);
                    dismiss_handle.update(cx, |_, cx| cx.notify());
                }),
        );
    } else if !unmanaged_extraction {
        let pause_handle = handle.clone();
        let turbo_handle = handle.clone();
        let pause_icon = if paused { "icons/play.svg" } else { "icons/pause.svg" };
        let turbo_icon = if turbo { "icons/zap.svg" } else { "icons/zap-off.svg" };
        status_row = status_row
            .child(
                div()
                    .id(format!("pause-{pause_id}"))
                    .w(px(22.0))
                    .h(px(22.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded_sm()
                    .hover(|style| style.bg(theme.control_hover))
                    .tooltip(Tooltip::text(i18n.text(if paused {
                        Message::OpsResume
                    } else {
                        Message::OpsPause
                    })))
                    .child(svg().path(pause_icon).text_color(theme.text).size_4())
                    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                        pause_manager.set_paused(&pause_id, !paused);
                        pause_handle.update(cx, |_, cx| cx.notify());
                    }),
            )
            .child(
                div()
                    .id(format!("turbo-{turbo_id}"))
                    .w(px(22.0))
                    .h(px(22.0))
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
                    .child(svg().path(turbo_icon).text_color(if turbo { theme.accent } else { theme.text_muted }).size_4())
                    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                        turbo_manager.set_turbo(&turbo_id, !turbo);
                        turbo_handle.update(cx, |_, cx| cx.notify());
                    }),
            )
            .child(
                div()
                    .id(format!("cancel-{cancel_id}"))
                    .w(px(22.0))
                    .h(px(22.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded_sm()
                    .hover(|style| style.bg(theme.control_hover))
                    .tooltip(Tooltip::text(i18n.text(Message::OpsStop)))
                    .child(svg().path("icons/x.svg").text_color(theme.text_muted).size_4())
                    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                        cancel_manager.cancel_operation(&cancel_id);
                        handle.update(cx, |_, cx| cx.notify());
                    }),
            );
    }

    let card = div()
        .id(format!("operation-{id}"))
        .p(px(12.0))
        .border_b_1()
        .border_color(theme.border)
        .flex()
        .flex_col()
        .gap(px(5.0))
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
                        .gap(px(6.0))
                        .child(svg().path(icon).text_color(theme.text).size_4().flex_none())
                        .child(i18n.text(kind))
                        .child(div().text_color(theme.text_muted).overflow_hidden().child(items)),
                )
                .child(status_row),
        );
    let mut body = div()
        .relative()
        .flex()
        .flex_col()
        .gap(px(5.0))
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
    if progress_text.is_some() || percentage.is_some() {
        body = body.child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap(px(8.0))
                .child(div().min_w_0().overflow_hidden().child(progress_text.unwrap_or_default()))
                .child(div().flex_none().child(
                    percentage.map(|value| format!("{value}%")).unwrap_or_default(),
                )),
        );
    }
    let temporal = if final_state {
        operation.duration_ms.map(|duration_ms| {
            let mut args = FluentArgs::new();
            args.set("duration", format_elapsed_duration(duration_ms, language));
            i18n.text_with_args(Message::OpsDuration, &args)
        })
    } else {
        remaining
    };
    if !final_state || speed.is_some() || temporal.is_some() {
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
                .child(div().text_color(theme.text_muted).child(temporal.unwrap_or_default())),
        );
    }
    if let OpStatus::Error(error) = &operation.status {
        body = body.child(div().text_color(theme.error).child(error.clone()));
    }
    if let Some(percentage) = percentage {
        body = body.child(
            div().w_full().h(px(4.0)).overflow_hidden().bg(theme.control_hover).child(
                div()
                    .w(relative(percentage as f32 / 100.0))
                    .h_full()
                    .bg(theme.accent),
            ),
        );
    }
    card.child(body).into_any_element()
}
