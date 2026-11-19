//! The prompt shown before opening many items at once: « Cancel » holds the keyboard first.
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
    let confirmation = view.open_many.as_ref().expect("open confirmation");
    let theme = *cx.theme();
    let mut args = FluentArgs::new();
    args.set("count", confirmation.count() as i64);
    args.set("number", crate::ui::format::format_number(confirmation.count() as u64, cx.i18n().language()));
    let (keyboard, cancel, open, close) = (handle.clone(), handle.clone(), handle.clone(), handle);
    div()
        .absolute()
        .inset_0()
        .on_key_down(move |event, window, cx| {
            keyboard.update(cx, |view, cx| match event.keystroke.key.as_str() {
                "escape" => view.resolve_open_many(false, window, cx),
                "tab" => {
                    if let Some(confirmation) = &view.open_many {
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
                "open-many-confirmation",
                cx.i18n().text(Message::OpenManyTitle),
                confirmation::body(
                    "icons/external-link.svg",
                    Tone::Neutral,
                    cx.i18n().text_with_args(Message::OpenManyPrompt, &args),
                    Vec::new(),
                    None,
                    theme,
                ),
            )
            .title_icon("icons/external-link.svg")
            .on_close(move |window, cx| {
                close.update(cx, |view, cx| view.resolve_open_many(false, window, cx));
            })
            .action(
                dialog::button("open-many-cancel", cx.i18n().text(Message::Cancel), ButtonVariant::Secondary)
                    .focus(&confirmation.focus)
                    .on_press(move |window, cx| {
                        cancel.update(cx, |view, cx| view.resolve_open_many(false, window, cx));
                    }),
            )
            .action(
                dialog::button("open-many-open", cx.i18n().text(Message::OpenManyConfirm), Tone::Neutral.variant())
                    .focus(&confirmation.confirm_focus)
                    .on_press(move |window, cx| {
                        open.update(cx, |view, cx| view.resolve_open_many(true, window, cx));
                    }),
            ),
        )
}
