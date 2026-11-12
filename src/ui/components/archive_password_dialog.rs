//! The password an encrypted archive needs, asked as File Roller asks it.
use crate::ui::app::OxydeApp;
use crate::ui::components::modal::{action_button, form_row, Modal};
use crate::ui::elements::button::{button_style, ButtonVariant};
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
        .text_size(rems(0.8125))
        .child(div().pb(rems(0.375)).child(cx.i18n().text_with_args(Message::ArchivePasswordPrompt, &args)))
        .child(form_row(cx.i18n().text(Message::NetworkPassword), field, cx));
    if prompt.wrong {
        body = body.child(crate::ui::components::modal::field_error(cx.i18n().text(Message::ArchivePasswordWrong), theme));
    }
    let focused = |control: usize| prompt.controls[control].is_focused(window);
    let (cancel, submit, close) = (handle.clone(), handle.clone(), handle.clone());
    let modal = Modal::new("archive-password", cx.i18n().text(Message::ArchivePasswordTitle), body)
        .title_icon("icons/key.svg")
        .standalone(false)
        .body_background(theme.active_background)
        .on_close(move |window, cx| close.update(cx, |view, cx| view.answer_archive_password(true, window, cx)))
        .width(rems(27.5))
        .action(
            button_style(action_button("archive-password-cancel"), theme, ButtonVariant::Secondary, true)
                .track_focus(&prompt.controls[CANCEL])
                .border(rems(0.0625))
                .border_color(if focused(CANCEL) { theme.accent } else { theme.border })
                .child(cx.i18n().text(Message::Cancel))
                .on_click(move |_, window, cx| cancel.update(cx, |view, cx| view.answer_archive_password(true, window, cx))),
        )
        .action(
            button_style(action_button("archive-password-unlock"), theme, ButtonVariant::Primary, !prompt.value.is_empty() && !prompt.checking)
                .track_focus(&prompt.controls[UNLOCK])
                .border(rems(0.0625))
                .border_color(if focused(UNLOCK) { theme.text_on_accent } else { theme.accent })
                .child(cx.i18n().text(Message::ArchivePasswordUnlock))
                .on_click(move |_, window, cx| submit.update(cx, |view, cx| view.answer_archive_password(false, window, cx))),
        );
    let keyboard = handle;
    div()
        .absolute()
        .inset_0()
        .track_focus(&prompt.focus)
        .on_key_down(move |event, window, cx| {
            keyboard.update(cx, |view, cx| {
                let Some(prompt) = &view.archive_password else { return };
                let focused = |control: usize| prompt.controls[control].is_focused(window);
                match event.keystroke.key.as_str() {
                    "escape" => view.answer_archive_password(true, window, cx),
                    "tab" => {
                        let order = [
                            prompt.input.read(cx).focus_handle.clone(),
                            prompt.controls[CANCEL].clone(),
                            prompt.controls[UNLOCK].clone(),
                        ];
                        let current = order.iter().position(|focus| focus.is_focused(window));
                        let next = if event.keystroke.modifiers.shift {
                            current.map_or(order.len() - 1, |current| (current + order.len() - 1) % order.len())
                        } else {
                            current.map_or(0, |current| (current + 1) % order.len())
                        };
                        window.focus(&order[next], cx);
                    }
                    "enter" | "space" if focused(CANCEL) => view.answer_archive_password(true, window, cx),
                    "space" if focused(UNLOCK) => view.answer_archive_password(false, window, cx),
                    "enter" => view.answer_archive_password(false, window, cx),
                    _ => return,
                }
                cx.stop_propagation();
            })
        })
        .on_action(|_: &crate::ui::app::Ouvrir, _, cx| cx.stop_propagation())
        .on_action(|_: &crate::ui::app::Supprimer, _, cx| cx.stop_propagation())
        .on_action(|_: &crate::ui::app::Renommer, _, cx| cx.stop_propagation())
        .on_action(|_: &crate::ui::app::Copier, _, cx| cx.stop_propagation())
        .on_action(|_: &crate::ui::app::Couper, _, cx| cx.stop_propagation())
        .on_action(|_: &crate::ui::app::Coller, _, cx| cx.stop_propagation())
        .on_action(|_: &crate::ui::app::ToutSelectionner, _, cx| cx.stop_propagation())
        .on_action(|_: &crate::ui::app::Annuler, _, cx| cx.stop_propagation())
        .on_action(|_: &crate::ui::app::Refaire, _, cx| cx.stop_propagation())
        .on_action(|_: &crate::ui::app::Actualiser, _, cx| cx.stop_propagation())
        .child(modal)
}
