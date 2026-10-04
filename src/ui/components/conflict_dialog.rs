use crate::ui::app::OxydeApp;
use crate::ui::components::modal::{action_button, Modal};
use crate::ui::format::format_number;
use crate::ui::i18n::{FluentArgs, I18nContext, Message};
use crate::ui::theme::ThemeContext;
use gpui::prelude::*;
use gpui::*;

pub fn render_conflict_dialog(
    conflict_count: usize,
    handle: Entity<OxydeApp>,
    cx: &mut Context<OxydeApp>,
) -> impl IntoElement {
    let theme = *cx.theme();
    let cancel_handle = handle.clone();
    let skip_handle = handle.clone();
    let replace_handle = handle;
    let mut args = FluentArgs::new();
    args.set("count", conflict_count);
    args.set(
        "number",
        format_number(conflict_count as u64, cx.i18n().language()),
    );

    // This dialog requires an explicit action: no backdrop or Escape dismissal.
    Modal::new(
        "conflict-dialog",
        cx.i18n().text(Message::ConflictTitle),
        div().text_color(theme.text_muted).child(
            cx.i18n()
                .text_with_args(Message::ConflictDescription, &args),
        ),
    )
    .title_icon("icons/copy-x.svg")
    .action(
        action_button("operation-cancel")
            .hover(|d| d.bg(theme.control_hover))
            .child(cx.i18n().text(Message::Cancel))
            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                cancel_handle.update(cx, |this, cx| this.cancel_pending_file_operation(cx));
            }),
    )
    .action(
        action_button("operation-skip")
            .hover(|d| d.bg(theme.control_hover))
            .child(cx.i18n().text(Message::ConflictSkipAll))
            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                skip_handle.update(cx, |this, cx| {
                    this.resolve_pending_file_operation(false, cx)
                });
            }),
    )
    .action(
        action_button("operation-replace")
            .bg(theme.accent)
            .text_color(theme.text_on_accent)
            .hover(|d| d.opacity(0.85))
            .child(cx.i18n().text(Message::ConflictReplaceAll))
            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                replace_handle.update(cx, |this, cx| this.resolve_pending_file_operation(true, cx));
            }),
    )
}
