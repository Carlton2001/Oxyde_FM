use crate::ui::app::OxydeApp;
use crate::ui::i18n::{I18nContext, Message};
use crate::ui::theme::ThemeContext;
use gpui::prelude::*;
use gpui::*;
use oxyde_lib::systems::file_ops::{FileOpType, OpStatus};

/// One nonblocking notification per active operation, sharing the completion stack.
pub(crate) fn render_operation_progress(app: &OxydeApp, cx: &Context<OxydeApp>) -> Vec<AnyElement> {
    let theme = *cx.theme();
    app.file_ops
        .list_active_progress()
        .into_iter()
        .map(|op| {
            let owner = cx.entity();
            let (label, icon) = match op.op_type {
                FileOpType::Copy => (Message::OpCopy, "icons/copy.svg"),
                FileOpType::Move => (Message::OpMove, "icons/arrow-right.svg"),
                FileOpType::Delete => (Message::OpDelete, "icons/delete.svg"),
                FileOpType::Trash => (Message::OpDelete, "icons/trash.svg"),
                FileOpType::Compress => (Message::OpCompress, "icons/archive.svg"),
                FileOpType::Extract => (Message::OpExtract, "icons/archive-restore.svg"),
            };
            let ratio = if op.total_bytes > 0 {
                op.processed_bytes as f64 / op.total_bytes as f64
            } else if op.total_files > 0 {
                op.processed_files as f64 / op.total_files as f64
            } else {
                0.0
            };
            let finalizing = matches!(op.status, OpStatus::Running) && ratio >= 1.0;
            let ratio = ratio.clamp(0.0, 1.0) as f32;
            let progress_label = if finalizing {
                cx.i18n().text(Message::OpsFinalizing)
            } else {
                format!("{}%", (ratio * 100.0).round() as u32).into()
            };
            div()
                .id(SharedString::from(format!("operation-progress-{}", op.id)))
                .on_click(move |_, _, cx| {
                    super::open_operations_window(owner.clone(), cx);
                })
                .w(rems(24.0))
                .min_h(rems(3.0))
                .max_w_full()
                .px(rems(1.0))
                .py(rems(0.75))
                .rounded(rems(0.5))
                .border(rems(0.0625))
                .border_color(theme.border)
                .bg(theme.surface)
                .shadow(crate::ui::style::rem_shadow(false))
                .occlude()
                .flex()
                .items_center()
                .gap(rems(0.75))
                .text_size(rems(0.8125))
                .text_color(theme.text)
                .child(
                    svg()
                        .path(icon)
                        .size(rems(1.125))
                        .flex_none()
                        .text_color(theme.accent),
                )
                .child(div().flex_none().child(cx.i18n().text(label)))
                .child(
                    div()
                        .relative()
                        .flex_1()
                        .min_w_0()
                        .h(rems(0.25))
                        .rounded_full()
                        .bg(theme.border)
                        .overflow_hidden()
                        .child(div().h_full().w(relative(ratio)).bg(theme.accent))
                        .when(finalizing, |bar| {
                            bar.child(
                                div()
                                    .absolute()
                                    .top_0()
                                    .h_full()
                                    .w(relative(0.4))
                                    .flex()
                                    .child(div().flex_1().h_full().bg(linear_gradient(
                                        90.0,
                                        linear_color_stop(theme.text_on_accent.opacity(0.0), 0.0),
                                        linear_color_stop(theme.text_on_accent.opacity(0.45), 1.0),
                                    )))
                                    .child(div().flex_1().h_full().bg(linear_gradient(
                                        90.0,
                                        linear_color_stop(theme.text_on_accent.opacity(0.45), 0.0),
                                        linear_color_stop(theme.text_on_accent.opacity(0.0), 1.0),
                                    )))
                                    .with_animation(
                                        "operation-finalizing-shimmer",
                                        Animation::new(std::time::Duration::from_millis(1800))
                                            .repeat(),
                                        |reflection, delta| {
                                            reflection.left(relative(-0.4 + 1.4 * delta))
                                        },
                                    ),
                            )
                        }),
                )
                .child(
                    div()
                        .min_w(rems(2.5))
                        .flex_none()
                        .text_right()
                        .child(progress_label),
                )
                .into_any_element()
        })
        .collect()
}
