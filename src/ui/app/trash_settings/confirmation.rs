use super::*;
use crate::ui::components::{confirmation, dialog};
use crate::ui::components::modal::Modal;
use crate::ui::elements::button::ButtonVariant;
use crate::ui::i18n::FluentArgs;
use crate::ui::theme::ThemeContext;

pub(crate) fn render_delete_confirmation(
    view: &OxydeApp,
    handle: Entity<OxydeApp>,
    window: &mut Window,
    cx: &mut Context<OxydeApp>,
) -> impl IntoElement {
    let pending = view.pending_trash_delete.as_ref().unwrap();
    if !pending.focus.is_focused(window) && !pending.confirm_focus.is_focused(window) {
        window.focus(&pending.focus, cx);
    }
    let tone = if pending.permanent_prompt { confirmation::Tone::Danger } else { confirmation::Tone::Neutral };
    let body = delete_body(pending, tone, cx);
    let title = cx.i18n().text(if pending.permanent_prompt { Message::TrashDeletePermanent } else { Message::Delete });
    let answer = move |handle: &Entity<OxydeApp>, confirm: bool, window: &mut Window, cx: &mut App| {
        handle.update(cx, |this, cx| {
            if let Some(pending) = this.pending_trash_delete.take() {
                if confirm {
                    this.queue_trash_delete(pending.paths, pending.permanent, cx);
                }
            }
            window.focus(&this.focus_handle, cx);
            cx.notify();
        });
    };
    let (cancel, confirm, close, keyboard) = (handle.clone(), handle.clone(), handle.clone(), handle);
    // Cancel holds the keyboard first.
    div()
        .absolute()
        .inset_0()
        .on_key_down(move |event, window, cx| {
            match event.keystroke.key.as_str() {
                "escape" => answer(&keyboard, false, window, cx),
                "tab" => {
                    let Some(pending) = &keyboard.read(cx).pending_trash_delete else { return };
                    let order = [pending.focus.clone(), pending.confirm_focus.clone()];
                    dialog::cycle(&order, event.keystroke.modifiers.shift, window, cx);
                    keyboard.update(cx, |_, cx| cx.notify());
                }
                _ => return,
            }
            cx.stop_propagation();
        })
        .child(
            Modal::new("trash-delete-confirmation", title.clone(), body)
                .title_icon("icons/trash-2.svg")
                .on_close(move |window, cx| answer(&close, false, window, cx))
                .action(
                    dialog::button("trash-delete-cancel", cx.i18n().text(Message::Cancel), ButtonVariant::Secondary)
                        .focus(&pending.focus)
                        .on_press(move |window, cx| answer(&cancel, false, window, cx)),
                )
                .action(
                    dialog::button("trash-delete-confirm", title, tone.variant())
                        .focus(&pending.confirm_focus)
                        .on_press(move |window, cx| answer(&confirm, true, window, cx)),
                ),
        )
}

/// The question, the first names concerned, and what follows: a way back from the
/// trash, or a warning when nothing will be kept.
fn delete_body(pending: &PendingDelete, tone: confirmation::Tone, cx: &App) -> Div {
    const SHOWN: usize = 3;
    let theme = *cx.theme();
    let language = cx.i18n().language();
    let count = pending.paths.len();
    let mut args = FluentArgs::new();
    args.set("count", count as i64);
    args.set("number", crate::ui::format::format_number(count as u64, language));
    let question = cx.i18n().text_with_args(
        if pending.permanent_prompt { Message::DeletePermanentPromptCount } else { Message::DeletePromptCount },
        &args,
    );
    let mut details: Vec<AnyElement> = pending
        .paths
        .iter()
        .take(SHOWN)
        .map(|path| {
            let name = std::path::Path::new(path)
                .file_name()
                .map(|name| oxyde_lib::utils::path_text::display(&name.to_string_lossy()))
                .unwrap_or_else(|| path.clone());
            div().min_w_0().truncate().child(name).into_any_element()
        })
        .collect();
    if count > SHOWN {
        let mut args = FluentArgs::new();
        args.set("count", (count - SHOWN) as i64);
        args.set("number", crate::ui::format::format_number((count - SHOWN) as u64, language));
        details.push(confirmation::detail(cx.i18n().text_with_args(Message::DeleteMore, &args), theme));
    }
    if !pending.permanent_prompt {
        details.push(div().mt(rems(0.25)).child(confirmation::detail(cx.i18n().text(Message::DeleteRestoreHint), theme)).into_any_element());
    }
    let warning = pending.permanent_prompt.then(|| cx.i18n().text(Message::DeleteIrreversible));
    confirmation::body("icons/trash-2.svg", tone, question, details, warning, theme)
}
