use super::*;
use crate::ui::components::modal::{action_button, Modal};
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
                div().child(cx.i18n().text(if pending.permanent_prompt { Message::TrashPermanentDeletePrompt } else { Message::TrashDeletePrompt })),
            )
            .title_icon("icons/trash-2.svg")
            .on_backdrop_dismiss(move |window, cx| {
                close.update(cx, |this, cx| {
                    this.pending_trash_delete = None;
                    window.focus(&this.focus_handle, cx);
                    cx.notify();
                });
            })
            .action(
                action_button("trash-delete-cancel")
                    .track_focus(&pending.focus)
                    .child(cx.i18n().text(Message::Cancel))
                    .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                        cancel.update(cx, |this, cx| {
                            this.pending_trash_delete = None;
                            window.focus(&this.focus_handle, cx);
                            cx.notify();
                        });
                    }),
            )
            .action(
                action_button("trash-delete-confirm")
                    .track_focus(&pending.confirm_focus)
                    .bg(theme.accent)
                    .text_color(theme.text_on_accent)
                    .child(cx.i18n().text(if pending.permanent_prompt { Message::TrashDeletePermanent } else { Message::Delete }))
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
