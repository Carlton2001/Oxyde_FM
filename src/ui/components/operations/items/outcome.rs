use crate::ui::i18n::Message;
use crate::ui::theme::OxydeTheme;
use gpui::prelude::*;
use gpui::*;
use oxyde_lib::systems::file_ops::ItemOutcome;

/// What the State column says of an item, and its badge's fill and text: solid for
/// work under way and failures, tinted for success and partial results, neutral
/// for the rest, so the states also differ by lightness.
pub(in super::super) fn label(outcome: ItemOutcome, theme: OxydeTheme) -> (Message, Hsla, Hsla) {
    let neutral = (theme.badge_surface, theme.text_muted);
    let (message, (fill, text)) = match outcome {
        ItemOutcome::Pending => (Message::OpsQueued, neutral),
        ItemOutcome::InProgress => (Message::OpsRunning, (theme.accent, theme.text_on_accent)),
        ItemOutcome::Completed => (Message::OpsItemDone, (theme.success_text.opacity(0.16), theme.success_text)),
        ItemOutcome::NotProcessed => (Message::OpsItemNotProcessed, neutral),
        ItemOutcome::Skipped => (Message::OpsItemSkipped, neutral),
        ItemOutcome::Partial => (Message::OpsItemPartial, (theme.warning_background, theme.warning_text)),
        ItemOutcome::Failed => (Message::OpsItemFailed, (theme.danger_background, theme.text_on_danger)),
        ItemOutcome::Interrupted => (Message::OpsItemInterrupted, neutral),
        ItemOutcome::Unknown => (Message::OpsItemUnknown, neutral),
    };
    (message, fill, text)
}

/// Horizontal room a badge adds around its text.
pub(in super::super) const BADGE_PADDING: Rems = rems(0.375);

/// A state written on its color, as the Duplicates window's media badges.
pub(in super::super) fn badge(text: impl Into<SharedString>, fill: Hsla, color: Hsla) -> Div {
    div()
        .flex_none()
        .rounded(rems(0.25))
        .px(BADGE_PADDING)
        .py(rems(0.0625))
        .bg(fill)
        .text_color(color)
        .text_size(rems(0.6875))
        .font_weight(FontWeight::MEDIUM)
        .child(text.into())
}

/// Sort order of the State column: work under way first, then what needs a look.
pub(super) fn rank(outcome: Option<ItemOutcome>) -> u8 {
    match outcome {
        Some(ItemOutcome::InProgress) => 0,
        Some(ItemOutcome::Failed) => 1,
        Some(ItemOutcome::Partial) => 2,
        Some(ItemOutcome::Interrupted) => 3,
        Some(ItemOutcome::Pending) => 4,
        Some(ItemOutcome::Skipped) => 5,
        Some(ItemOutcome::NotProcessed) => 6,
        Some(ItemOutcome::Unknown) | None => 7,
        Some(ItemOutcome::Completed) => 8,
    }
}
