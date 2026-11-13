//! Credentials a remote server asks for while its share mounts, as GTK's mount dialog,
//! or its question with choices, such as accepting an unknown SFTP host key.
use crate::ui::app::OxydeApp;
use crate::ui::components::dialog;
use crate::ui::components::modal::{form_row, Modal};
use crate::ui::elements::button::ButtonVariant;
use crate::ui::i18n::{I18nContext, Message};
use crate::ui::theme::ThemeContext;
use gpui::prelude::*;
use gpui::*;
use oxyde_lib::platform::PasswordSave;

const ANONYMOUS: usize = 0;
const REMEMBER: usize = 1;
/// The choices of GTK's mount dialog, in its order.
const SAVES: [(PasswordSave, Message); 3] = [
    (PasswordSave::Never, Message::RemoteRememberNever),
    (PasswordSave::Session, Message::RemoteRememberSession),
    (PasswordSave::Permanently, Message::RemoteRememberAlways),
];

/// The next choice, wrapping, for arrow keys and Space.
fn cycled(save: PasswordSave, backwards: bool) -> PasswordSave {
    let index = SAVES.iter().position(|(choice, _)| *choice == save).unwrap_or(0);
    let step = if backwards { SAVES.len() - 1 } else { 1 };
    SAVES[(index + step) % SAVES.len()].0
}
const CANCEL: usize = 2;
const CONNECT: usize = 3;

pub(crate) fn render_remote_login_dialog(
    view: &OxydeApp,
    handle: Entity<OxydeApp>,
    window: &mut Window,
    cx: &mut Context<OxydeApp>,
) -> AnyElement {
    let login = view.remote_login.as_ref().expect("remote login");
    if !login.prompt.choices.is_empty() {
        return render_question(view, handle, cx).into_any_element();
    }
    let theme = *cx.theme();
    let prompt = &login.prompt;
    // GVfs puts a title line first, which the dialog title already says.
    let message = prompt.message.split_once('\n').map_or(prompt.message.as_str(), |(_, rest)| rest).trim().to_string();
    let mut body = div().flex().flex_col().gap(rems(0.375))
        .child(div().pb(rems(0.375)).child(message));
    if prompt.anonymous {
        let toggle = handle.clone();
        body = body.child(div()
            .id("remote-anonymous")
            .track_focus(&login.controls[ANONYMOUS])
            .flex().items_center().gap(rems(0.5)).py_1()
            .on_click(move |_, window, cx| toggle.update(cx, |view, cx| {
                if let Some(login) = &view.remote_login {
                    window.focus(&login.controls[ANONYMOUS], cx);
                }
                view.toggle_remote_anonymous(cx);
            }))
            .child(crate::ui::elements::focus_ring::focus_ring(
                crate::ui::elements::checkbox::checkbox(theme, login.anonymous, false),
                crate::ui::elements::focus_ring::keyboard_focused(&login.controls[ANONYMOUS], window),
                false,
                theme,
            ))
            .child(cx.i18n().text(Message::RemoteAnonymous)));
    }
    for (index, needed, label) in [
        (0, prompt.needs_user, Message::NetworkUsername),
        (1, prompt.needs_domain, Message::RemoteDomain),
        (2, prompt.needs_password, Message::NetworkPassword),
    ] {
        if !needed {
            continue;
        }
        let input = login.inputs[index].clone();
        let field = if login.anonymous {
            let value = &login.values[index];
            let shown = if index == 2 { "*".repeat(value.chars().count()) } else { value.clone() };
            crate::ui::components::input::read_only_field(shown, cx).into_any_element()
        } else {
            input.into_any_element()
        };
        body = body.child(form_row(cx.i18n().text(label), field, cx));
    }
    if prompt.saving && !login.anonymous {
        // GTK's stacked radio sentences under the fields; one focus stop for the group.
        let mut choices = div()
            .id("remote-remember")
            .track_focus(&login.controls[REMEMBER])
            .flex().flex_col().min_w_0();
        let group_ring = crate::ui::elements::focus_ring::keyboard_focused(&login.controls[REMEMBER], window);
        for (index, (save, label)) in SAVES.into_iter().enumerate() {
            let select = handle.clone();
            choices = choices.child(
                div()
                    .id(("remote-remember-choice", index))
                    .flex().items_center().gap(rems(0.5)).py_1().min_w_0()
                    // The group is one focus stop: the ring marks its chosen radio.
                    .child(crate::ui::elements::focus_ring::focus_ring(
                        crate::ui::elements::radio::radio(theme, login.save == save),
                        group_ring && login.save == save,
                        true,
                        theme,
                    ))
                    .child(div().min_w_0().child(cx.i18n().text(label)))
                    .on_click(move |_, window, cx| select.update(cx, |view, cx| {
                        if let Some(login) = &view.remote_login {
                            window.focus(&login.controls[REMEMBER], cx);
                        }
                        view.set_remote_password_save(save, cx);
                    })),
            );
        }
        body = body.child(form_row(SharedString::default(), choices, cx));
    }
    if prompt.retry {
        body = body.child(crate::ui::components::modal::field_error(cx.i18n().text(Message::RemoteRetry), theme));
    }
    let cancel = handle.clone();
    let submit = handle.clone();
    let close = handle.clone();
    let modal = Modal::new("remote-login", cx.i18n().text(Message::RemoteLoginTitle), body)
        .title_icon("icons/server.svg")
        .on_close(move |window, cx| close.update(cx, |view, cx| view.answer_remote_login(true, window, cx)))
        .action(
            dialog::button("remote-cancel", cx.i18n().text(Message::Cancel), ButtonVariant::Secondary)
                .focus(&login.controls[CANCEL])
                .on_press(move |window, cx| cancel.update(cx, |view, cx| view.answer_remote_login(true, window, cx))),
        )
        .action(
            dialog::button("remote-connect", cx.i18n().text(Message::RemoteConnect), ButtonVariant::Primary)
                .focus(&login.controls[CONNECT])
                .on_press(move |window, cx| submit.update(cx, |view, cx| view.answer_remote_login(false, window, cx))),
        );
    let keyboard = handle.clone();
    div()
        .absolute()
        .inset_0()
        .track_focus(&login.focus)
        .on_key_down(move |event, window, cx| {
            keyboard.update(cx, |view, cx| {
                let Some(login) = &view.remote_login else { return };
                let focused = |control: usize| login.controls[control].is_focused(window);
                match event.keystroke.key.as_str() {
                    "escape" => view.answer_remote_login(true, window, cx),
                    "tab" => {
                        let prompt = &login.prompt;
                        let mut order = Vec::new();
                        if prompt.anonymous {
                            order.push(login.controls[ANONYMOUS].clone());
                        }
                        if !login.anonymous {
                            for (index, needed) in [prompt.needs_user, prompt.needs_domain, prompt.needs_password].into_iter().enumerate() {
                                if needed {
                                    order.push(login.inputs[index].read(cx).focus_handle.clone());
                                }
                            }
                            if prompt.saving {
                                order.push(login.controls[REMEMBER].clone());
                            }
                        }
                        order.extend(login.controls[CANCEL..].iter().cloned());
                        dialog::cycle(&order, event.keystroke.modifiers.shift, window, cx);
                    }
                    "space" if focused(ANONYMOUS) => view.toggle_remote_anonymous(cx),
                    "space" | "right" | "left" | "down" | "up" if focused(REMEMBER) => {
                        let save = cycled(login.save, matches!(event.keystroke.key.as_str(), "left" | "up"));
                        view.set_remote_password_save(save, cx);
                    }
                    "enter" => view.answer_remote_login(false, window, cx),
                    _ => return,
                }
                cx.stop_propagation();
            })
        })
        .child(modal)
        .into_any_element()
}

/// GVfs's question, its answers as buttons in its order; Escape refuses.
fn render_question(
    view: &OxydeApp,
    handle: Entity<OxydeApp>,
    cx: &mut Context<OxydeApp>,
) -> impl IntoElement {
    let login = view.remote_login.as_ref().expect("remote question");
    // GVfs's first line is a heading, such as « Identity verification failed ».
    let (heading, text) = login.prompt.message.split_once('\n').unwrap_or(("", login.prompt.message.as_str()));
    let body = div().flex().flex_col().gap(rems(0.5))
        .when(!heading.trim().is_empty(), |body| body.child(div().font_weight(FontWeight::MEDIUM).child(heading.trim().to_string())))
        .children(text.split("\n\n").map(|paragraph| div().child(paragraph.trim().to_string())));
    let close = handle.clone();
    let mut modal = Modal::new("remote-question", cx.i18n().text(Message::RemoteQuestionTitle), body)
        .title_icon("icons/server.svg")
        .on_close(move |window, cx| close.update(cx, |view, cx| view.answer_remote_login(true, window, cx)));
    for (index, choice) in login.prompt.choices.iter().enumerate() {
        let answer = handle.clone();
        modal = modal.action(
            dialog::button(("remote-choice", index), choice.clone(), ButtonVariant::Secondary)
                .focus(&login.controls[index])
                .on_press(move |window, cx| answer.update(cx, |view, cx| view.answer_remote_question(index, window, cx))),
        );
    }
    let keyboard = handle.clone();
    div()
        .absolute()
        .inset_0()
        .track_focus(&login.focus)
        .on_key_down(move |event, window, cx| {
            keyboard.update(cx, |view, cx| {
                let Some(login) = &view.remote_login else { return };
                match event.keystroke.key.as_str() {
                    "escape" => view.answer_remote_login(true, window, cx),
                    "tab" | "left" | "right" => {
                        let back = event.keystroke.modifiers.shift || event.keystroke.key == "left";
                        dialog::cycle(&login.controls, back, window, cx);
                    }
                    _ => return,
                }
                cx.stop_propagation();
            })
        })
        .child(modal)
}
