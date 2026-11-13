//! The prompt shown when Oxyde is closed with operations unfinished: closing cancels
//! them, so « Do not quit » holds the keyboard first.
use crate::ui::app::OxydeApp;
use crate::ui::components::{
    confirmation::{self, Tone},
    dialog,
    modal::Modal,
};
use crate::ui::elements::button::ButtonVariant;
use crate::ui::i18n::{FluentArgs, I18nContext, Message};
use crate::ui::theme::ThemeContext;
use gpui::prelude::*;
use gpui::*;

pub(crate) fn render(
    view: &OxydeApp,
    handle: Entity<OxydeApp>,
    _window: &mut Window,
    cx: &mut Context<OxydeApp>,
) -> impl IntoElement {
    let confirmation = view.quit_confirmation.as_ref().expect("quit confirmation");
    let theme = *cx.theme();
    // Presses are ignored while cancelling; the prompt changes only if that lasts.
    let waiting = confirmation.waiting;
    let mut args = FluentArgs::new();
    args.set("count", confirmation.count as i64);
    args.set("number", crate::ui::format::format_number(confirmation.count as u64, cx.i18n().language()));
    let (keyboard, stay, quit, close) = (handle.clone(), handle.clone(), handle.clone(), handle);
    div()
        .absolute()
        .inset_0()
        .on_key_down(move |event, window, cx| {
            keyboard.update(cx, |view, cx| match event.keystroke.key.as_str() {
                "escape" => view.resolve_quit_confirmation(false, window, cx),
                "tab" => {
                    if let Some(confirmation) = &view.quit_confirmation {
                        let order = [confirmation.focus.clone(), confirmation.confirm_focus.clone()];
                        dialog::cycle(&order, event.keystroke.modifiers.shift, window, cx);
                    }
                }
                _ => {}
            });
            cx.stop_propagation();
        })
        .child(
            Modal::new(
                "quit-confirmation",
                cx.i18n().text(Message::QuitOperationsTitle),
                confirmation::body(
                    "icons/triangle-alert.svg",
                    Tone::Danger,
                    cx.i18n().text_with_args(Message::QuitOperationsPrompt, &args),
                    Vec::new(),
                    waiting.then(|| cx.i18n().text(Message::QuitOperationsCancelling)),
                    theme,
                ),
            )
            .title_icon("icons/triangle-alert.svg")
            .on_close(move |window, cx| {
                close.update(cx, |view, cx| view.resolve_quit_confirmation(false, window, cx));
            })
            .action(
                dialog::button("quit-confirmation-stay", cx.i18n().text(Message::QuitOperationsStay), ButtonVariant::Secondary)
                    .focus(&confirmation.focus)
                    .enabled(!waiting)
                    .on_press(move |window, cx| {
                        stay.update(cx, |view, cx| view.resolve_quit_confirmation(false, window, cx));
                    }),
            )
            .action(
                dialog::button("quit-confirmation-quit", cx.i18n().text(Message::QuitOperationsConfirm), Tone::Danger.variant())
                    .focus(&confirmation.confirm_focus)
                    .enabled(!waiting)
                    .on_press(move |window, cx| {
                        quit.update(cx, |view, cx| view.resolve_quit_confirmation(true, window, cx));
                    }),
            ),
        )
}
