//! The password an encrypted archive needs, asked as File Roller asks it.
use crate::ui::app::OxydeApp;
use crate::ui::components::dialog;
use crate::ui::components::modal::{form_row, Modal};
use crate::ui::elements::button::ButtonVariant;
use crate::ui::i18n::{FluentArgs, I18nContext, Message};
use crate::ui::theme::ThemeContext;
use gpui::prelude::*;
use gpui::*;

const CANCEL: usize = 0;
const UNLOCK: usize = 1;

pub(crate) fn render_archive_password_dialog(
    view: &OxydeApp,
    handle: Entity<OxydeApp>,
    window: &mut Window,
    cx: &mut Context<OxydeApp>,
) -> impl IntoElement {
    let prompt = view.archive_password.as_ref().expect("archive password");
    let theme = *cx.theme();
    let mut args = FluentArgs::new();
    let name = prompt.archive.file_name().map_or_else(
        || prompt.archive.to_string_lossy().into_owned(),
        |name| name.to_string_lossy().into_owned(),
    );
    args.set("name", name);
    if prompt.refocus.take() {
        let focus = prompt.input.read(cx).focus_handle.clone();
        window.focus(&focus, cx);
    }
    let reveal = handle.clone();
    let field = crate::ui::components::input::password_field(
        &prompt.input,
        "archive-password-prompt-reveal",
        prompt.revealed,
        move |_, cx| reveal.update(cx, |view, cx| view.toggle_archive_password_prompt_reveal(cx)),
        window,
        cx,
    );
    let field = if prompt.wrong {
        field
            .with_animation(
                SharedString::from(format!("archive-password-shake-{}", prompt.shake_id)),
                Animation::new(std::time::Duration::from_millis(300)),
                |field, delta| {
                    let amplitude = (1.0 - delta) * crate::ui::scale::logical(8.0);
                    let offset = (delta * std::f32::consts::PI * 6.0).sin() * amplitude;
                    field.relative().left(px(offset))
                },
            )
            .into_any_element()
    } else {
        field.into_any_element()
    };
    let mut body = div()
        .flex()
        .flex_col()
        .gap(rems(0.375))
        .child(div().pb(rems(0.375)).child(cx.i18n().text_with_args(Message::ArchivePasswordPrompt, &args)))
        .child(form_row(cx.i18n().text(Message::NetworkPassword), field, cx));
    if prompt.wrong {
        body = body.child(crate::ui::components::modal::field_error(cx.i18n().text(Message::ArchivePasswordWrong), theme));
    }
    let (cancel, submit, close) = (handle.clone(), handle.clone(), handle.clone());
    let modal = Modal::new("archive-password", cx.i18n().text(Message::ArchivePasswordTitle), body)
        .title_icon("icons/key.svg")
        .on_close(move |window, cx| close.update(cx, |view, cx| view.answer_archive_password(true, window, cx)))
        .action(
            dialog::button("archive-password-cancel", cx.i18n().text(Message::Cancel), ButtonVariant::Secondary)
                .focus(&prompt.controls[CANCEL])
                .on_press(move |window, cx| cancel.update(cx, |view, cx| view.answer_archive_password(true, window, cx))),
        )
        .action(
            dialog::button("archive-password-unlock", cx.i18n().text(Message::ArchivePasswordUnlock), ButtonVariant::Primary)
                .focus(&prompt.controls[UNLOCK])
                .enabled(!prompt.value.is_empty() && !prompt.checking)
                .on_press(move |window, cx| submit.update(cx, |view, cx| view.answer_archive_password(false, window, cx))),
        );
    let keyboard = handle;
    div()
        .absolute()
        .inset_0()
        .track_focus(&prompt.focus)
        .on_key_down(move |event, window, cx| {
            keyboard.update(cx, |view, cx| {
                let Some(prompt) = &view.archive_password else { return };
                match event.keystroke.key.as_str() {
                    "escape" => view.answer_archive_password(true, window, cx),
                    "tab" => {
                        let order = [
                            prompt.input.read(cx).focus_handle.clone(),
                            prompt.controls[CANCEL].clone(),
                            prompt.controls[UNLOCK].clone(),
                        ];
                        dialog::cycle(&order, event.keystroke.modifiers.shift, window, cx);
                    }
                    "enter" => view.answer_archive_password(false, window, cx),
                    _ => return,
                }
                cx.stop_propagation();
            })
        })
        .child(modal)
}
