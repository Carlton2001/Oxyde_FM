use crate::ui::i18n::{I18nContext, Message};
use crate::ui::theme::ThemeContext;
use gpui::prelude::*;
use gpui::*;
use oxyde_lib::systems::file_ops::ItemOutcome;

pub(super) fn badge(outcome: ItemOutcome, cx: &App) -> Div {
    let theme = *cx.theme();
    let (message, color) = match outcome {
        ItemOutcome::Pending => (Message::OpsQueued, theme.text_muted),
        ItemOutcome::InProgress => (Message::OpsRunning, theme.accent),
        ItemOutcome::Completed => (Message::OpsItemDone, theme.success_text),
        ItemOutcome::NotProcessed => (Message::OpsItemNotProcessed, theme.text_muted),
        ItemOutcome::Skipped => (Message::OpsItemSkipped, theme.text_muted),
        ItemOutcome::Partial => (Message::OpsItemPartial, theme.warning_text),
        ItemOutcome::Interrupted => (Message::OpsItemInterrupted, theme.text_muted),
        ItemOutcome::Unknown => (Message::OpsItemUnknown, theme.text_muted),
    };
    div()
        .flex_none()
        .px(rems(0.5))
        .py(rems(0.25))
        .rounded(rems(0.375))
        .text_size(rems(0.75))
        .bg(color.opacity(0.08))
        .text_color(color)
        .child(cx.i18n().text(message))
}
