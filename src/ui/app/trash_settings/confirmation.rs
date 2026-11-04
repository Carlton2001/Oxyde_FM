use super::*;
use crate::ui::components::confirmation;
use crate::ui::components::modal::Modal;
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
    let theme = *cx.theme();
    let tone = if pending.permanent_prompt { confirmation::Tone::Danger } else { confirmation::Tone::Neutral };
    let body = delete_body(pending, tone, cx);
    let cancel = handle.clone();
    let close = handle.clone();
    let keyboard = handle.clone();
    div()
        .absolute()
        .inset_0()
        .track_focus(&pending.focus)
        .on_key_down(move |event, window, cx| {
            keyboard.update(cx, |this, cx| {
                match event.keystroke.key.as_str() {
                    "escape" => {
                        this.pending_trash_delete = None;
                        window.focus(&this.focus_handle, cx);
                    }
                    "tab" => {
                        if let Some(pending) = &this.pending_trash_delete {
                            window.focus(
                                if pending.confirm_focus.is_focused(window) {
                                    &pending.focus
                                } else {
                                    &pending.confirm_focus
                                },
                                cx,
                            );
                        }
                    }
                    "enter" | "space" => {
                        if let Some(pending) = this.pending_trash_delete.take() {
                            if pending.confirm_focus.is_focused(window) {
                                this.queue_trash_delete(pending.paths, pending.permanent, cx);
                            }
                            window.focus(&this.focus_handle, cx);
                        }
                    }
                    _ => {}
                }
                cx.notify();
            });
            cx.stop_propagation();
        })
        .on_action(|_: &Supprimer, _, cx| cx.stop_propagation())
        .on_action(|_: &Ouvrir, _, cx| cx.stop_propagation())
        .on_action(|_: &Renommer, _, cx| cx.stop_propagation())
        .on_action(|_: &Copier, _, cx| cx.stop_propagation())
        .on_action(|_: &Couper, _, cx| cx.stop_propagation())
        .on_action(|_: &Coller, _, cx| cx.stop_propagation())
        .child(
            Modal::new(
                "trash-delete-confirmation",
                cx.i18n().text(if pending.permanent_prompt { Message::TrashDeletePermanent } else { Message::Delete }),
                body,
            )
            .title_icon("icons/trash-2.svg")
            .standalone(false)
            .on_backdrop_dismiss(move |window, cx| {
                close.update(cx, |this, cx| {
                    this.pending_trash_delete = None;
                    window.focus(&this.focus_handle, cx);
                    cx.notify();
                });
            })
            .action(
                confirmation::cancel_button("trash-delete-cancel", pending.focus.is_focused(window), cx.i18n().text(Message::Cancel), theme)
                    .track_focus(&pending.focus)
                    .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                        cancel.update(cx, |this, cx| {
                            this.pending_trash_delete = None;
                            window.focus(&this.focus_handle, cx);
                            cx.notify();
                        });
                    }),
            )
            .action(
                confirmation::confirm_button(
                    "trash-delete-confirm",
                    pending.confirm_focus.is_focused(window),
                    tone,
                    cx.i18n().text(if pending.permanent_prompt { Message::TrashDeletePermanent } else { Message::Delete }),
                    theme,
                )
                .track_focus(&pending.confirm_focus)
                .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                    handle.update(cx, |this, cx| {
                        if let Some(pending) = this.pending_trash_delete.take() {
                            this.queue_trash_delete(pending.paths, pending.permanent, cx);
                        }
                        window.focus(&this.focus_handle, cx);
                        cx.notify();
                    });
                }),
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
