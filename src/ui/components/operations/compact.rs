use crate::ui::app::OxydeApp;
use crate::ui::i18n::{I18nContext, Message};
use crate::ui::elements::progress_bar::progress_bar;
use crate::ui::style::TabularFigures;
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
            let operation = op.id.clone();
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
            let paused = matches!(op.status, OpStatus::Paused);
            let ratio = ratio.clamp(0.0, 1.0) as f32;
            // A pause changes lightness, not only hue: the bar, the icon and the label dim.
            let color = if paused { theme.text_muted } else { theme.accent };
            let icon = if paused { "icons/pause.svg" } else { icon };
            let progress_label = if paused {
                cx.i18n().text(Message::OpsPaused)
            } else if finalizing {
                cx.i18n().text(Message::OpsFinalizing)
            } else {
                format!("{}%", (ratio * 100.0).round() as u32).into()
            };
            div()
                .id(SharedString::from(format!("operation-progress-{}", op.id)))
                .flex_none()
                .on_click(move |_, _, cx| {
                    super::open_operations_window(owner.clone(), Some(operation.clone()), cx);
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
                        .text_color(color),
                )
                .child(div().flex_none().child(cx.i18n().text(label)))
                .child(div().flex_1().min_w_0().child(progress_bar(
                    format!("operation-progress-bar-{}", op.id),
                    ratio,
                    color,
                    finalizing,
                    cx,
                )))
                .child(
                    div()
                        .min_w(rems(2.5))
                        .flex_none()
                        .text_right()
                        .tabular_figures()
                        .when(paused, |label| label.text_color(theme.text_muted))
                        .child(progress_label),
                )
                .into_any_element()
        })
        .collect()
}
