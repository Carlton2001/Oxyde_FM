use super::{graph, status_color};
use crate::ui::format::{format_number, format_size};
use crate::ui::i18n::{FluentArgs, I18nContext, Message};
use crate::ui::theme::ThemeContext;
use gpui::prelude::*;
use gpui::*;
use oxyde_lib::systems::file_ops::{FileOperation, OpStatus};

pub(super) fn render(operation: &FileOperation, cx: &App) -> Div {
    let theme = *cx.theme();
    let i18n = cx.i18n();
    let language = i18n.language();
    let final_state = super::super::is_final(&operation.status);
    let color = status_color(operation, theme);
    let ratio = if operation.total_bytes > 0 {
        Some(operation.processed_bytes as f64 / operation.total_bytes as f64)
    } else if operation.total_files > 0 {
        Some(operation.processed_files as f64 / operation.total_files as f64)
    } else {
        match operation.status {
            OpStatus::Completed => Some(1.0),
            OpStatus::Cancelled => Some(0.0),
            _ => None,
        }
    };
    let percentage = ratio.map(|ratio| (ratio * 100.0).clamp(0.0, 100.0).round() as u32);
    let volume = (operation.total_bytes > 0).then(|| {
        format!(
            "{} / {}",
            format_size(operation.processed_bytes, false, true, language),
            format_size(operation.total_bytes, false, true, language)
        )
    });
    let displayed_speed = if final_state {
        operation.duration_ms.and_then(|duration| {
            (duration > 0 && operation.processed_bytes > 0)
                .then(|| operation.processed_bytes.saturating_mul(1_000) / duration)
        })
    } else {
        (operation.bytes_per_second > 0).then_some(operation.bytes_per_second)
    };
    let speed = displayed_speed
        .map(|speed| {
            let value = format!("{}/s", format_size(speed, false, true, language));
            if final_state {
                let mut args = FluentArgs::new();
                args.set("speed", value);
                i18n.text_with_args(Message::OpsAverage, &args).to_string()
            } else {
                value
            }
        })
        .unwrap_or_else(|| {
            if final_state {
                String::new()
            } else if matches!(operation.status, OpStatus::Running)
                && ratio.is_some_and(|ratio| ratio >= 1.0)
            {
                i18n.text(Message::OpsFinalizing).to_string()
            } else {
                i18n.text(Message::OpsSpeedWaiting).to_string()
            }
        });
    let temporal = if final_state {
        operation.duration_ms.map(|duration| {
            let mut args = FluentArgs::new();
            args.set("duration", format_elapsed_duration(duration, language));
            i18n.text_with_args(Message::OpsDuration, &args)
        })
    } else if operation.bytes_per_second > 0 && operation.total_bytes > operation.processed_bytes {
        let seconds =
            (operation.total_bytes - operation.processed_bytes) / operation.bytes_per_second;
        let mut args = FluentArgs::new();
        args.set("seconds", format_number(seconds, language));
        Some(i18n.text_with_args(Message::OpsRemaining, &args))
    } else {
        None
    };

    div()
        .flex()
        .flex_col()
        .gap(rems(0.625))
        .child(
            div()
                .flex()
                .items_end()
                .justify_between()
                .gap(rems(0.75))
                .child(
                    div()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap(rems(0.125))
                        .child(
                            div()
                                .text_size(rems(1.625))
                                .line_height(relative(1.1))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(color)
                                .child(
                                    percentage
                                        .map(|value| format!("{value}%"))
                                        .unwrap_or_else(|| "—".into()),
                                ),
                        )
                        .when_some(volume, |column, volume| {
                            column.child(div().text_color(theme.text_muted).child(volume))
                        }),
                )
                .child(
                    div()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .items_end()
                        .gap(rems(0.25))
                        .child(
                            div()
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(theme.text)
                                .child(speed),
                        )
                        .when_some(temporal, |column, temporal| {
                            column.child(
                                div()
                                    .text_size(rems(0.75))
                                    .text_color(theme.text_muted)
                                    .child(temporal),
                            )
                        }),
                ),
        )
        .when(!final_state && !operation.speed_history.is_empty(), |metrics| {
            metrics.child(
                div()
                    .relative()
                    .overflow_hidden()
                    .child(
                        svg()
                            .path("visuals/operation-grid.svg")
                            .absolute()
                            .inset_0()
                            .size_full()
                            .text_color(theme.text_muted.opacity(0.4)),
                    )
                    .child(graph::render(operation.speed_history.clone(), color)),
            )
        })
        .when_some(percentage, |metrics, percentage| {
            metrics.child(
                div()
                    .h(rems(0.3125))
                    .w_full()
                    .rounded_full()
                    .bg(theme.control_hover)
                    .overflow_hidden()
                    .child(
                        div()
                            .h_full()
                            .rounded_full()
                            .w(relative(percentage as f32 / 100.0))
                            .bg(color.opacity(0.7)),
                    ),
            )
        })
}

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
