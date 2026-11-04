use crate::ui::app::OxydeApp;
use crate::ui::components::modal::{action_button, Modal};
use crate::ui::elements::button::{button_style, ButtonVariant};
use crate::ui::elements::segmented::{outline, segment, segmented};
use crate::ui::i18n::{I18nContext, Message};
use crate::ui::theme::ThemeContext;
use gpui::prelude::*;
use gpui::*;

pub fn render_rename_dialog(
    view: &OxydeApp,
    handle: Entity<OxydeApp>,
    window: &mut Window,
    cx: &mut Context<OxydeApp>,
) -> impl IntoElement {
    use crate::ui::app::archive_protection::{CANCEL, FORMATS, PROTECTION, SUBMIT};
    let controls = view.archive_protection.as_ref().map(|protection| protection.controls.clone());
    // A focus ring for the dialog's controls reached by the keyboard, not by a click.
    let keyboard = window.last_input_was_keyboard();
    let ring = |control: usize, idle: Hsla| {
        controls.as_ref().filter(|controls| controls[control].is_focused(window) && keyboard).map_or(idle, |_| theme_accent(cx))
    };
    let key_handle = handle.clone();
    let title = if view.archive_creation.is_some() {
        Message::Compress
    } else {
        Message::RenameTitle
    };
    let theme = *cx.theme();
    let input = view.editing_view.clone();
    let backdrop_handle = handle.clone();
    let cancel_handle = handle.clone();
    let submit_handle = handle;
    let error = view.rename_error.map(|message| cx.i18n().text(message));

    // Formats and protection are single choices: each one is a segmented row.
    let formats = view.archive_creation.as_ref().map(|(_, extension)| {
        let mut formats = segmented("archive-formats", Some(ring(FORMATS, outline(theme))), theme)
            .w_full()
            .mt(rems(0.625));
        if let Some(controls) = &controls {
            formats = formats.track_focus(&controls[FORMATS]);
        }
        for (format, label) in OxydeApp::ARCHIVE_FORMATS {
            let format_handle = cancel_handle.clone();
            let selected = *extension == crate::ui::app::archive_extension(format);
            formats = formats.child(
                segment(format!("archive-format-{format}"), selected, true, theme)
                    .flex_1()
                    .child(label)
                    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                        format_handle.update(cx, |this, cx| this.select_archive_format(format, cx));
                    }),
            );
        }
        formats
    });
    let protection = view.archive_creation.as_ref().zip(view.archive_protection.as_ref()).map(|((_, extension), protection)| {
        let allowed = crate::ui::app::archive_protection::encryptable(extension);
        let protected = allowed && protection.protected;
        let mut choices = segmented("archive-protection", Some(ring(PROTECTION, outline(theme))), theme)
            .track_focus(&protection.controls[PROTECTION])
            .w_full()
            .mt(rems(0.5));
        for (value, label) in [(false, Message::ArchiveProtectionNone), (true, Message::ArchiveProtectionPassword)] {
            let select = cancel_handle.clone();
            choices = choices.child(
                segment(if value { "archive-protection-password" } else { "archive-protection-none" }, protected == value, allowed, theme)
                    .flex_1()
                    .child(cx.i18n().text(label))
                    .when(allowed, |segment| {
                        segment.on_mouse_down(MouseButton::Left, move |_, window, cx| {
                            select.update(cx, |this, cx| this.set_archive_protected(value, window, cx));
                        })
                    }),
            );
        }
        let mut section = div().w_full().child(choices);
        if protected {
            for (index, label) in [(0, Message::NetworkPassword), (1, Message::ArchivePasswordConfirm)] {
                let reveal = cancel_handle.clone();
                let field = crate::ui::components::input::password_field(
                    &protection.inputs[index],
                    ("archive-password-reveal", index),
                    protection.revealed[index],
                    move |_, cx| reveal.update(cx, |this, cx| this.toggle_archive_password_reveal(index, cx)),
                    window,
                    cx,
                );
                section = section.child(
                    div().mt(rems(0.5)).child(crate::ui::components::modal::form_row(cx.i18n().text(label), field, cx)),
                );
            }
        }
        section
    });
    let body = div()
        .child(
            div().w_full().child(
                input
                    .map(|input| input.into_any_element())
                    .unwrap_or_else(|| div().into_any_element()),
            ),
        )
        .children(formats)
        .children(protection)
        .when_some(error, |body, error| {
            body.child(
                div()
                    .mt(rems(0.625))
                    .text_size(rems(0.75))
                    .text_color(theme.error)
                    .child(error),
            )
        });

    let modal = Modal::new("rename-dialog", cx.i18n().text(title), body)
        .title_icon(if view.archive_creation.is_some() {
            "icons/archive.svg"
        } else {
            "icons/pencil.svg"
        })
        .standalone(false)
        .width(rems(30.0))
        .body_gap(rems(0.375))
        .on_backdrop_dismiss(move |_, cx| {
            backdrop_handle.update(cx, |this, cx| this.cancel_rename(cx));
        });
    let modal = modal
        .action(
            button_style(
                action_button("rename-cancel"),
                theme,
                ButtonVariant::Secondary,
                true,
            )
            .when_some(controls.as_ref(), |button, controls| {
                button.track_focus(&controls[CANCEL]).border(rems(0.0625)).border_color(ring(CANCEL, theme.border))
            })
            .child(cx.i18n().text(Message::Cancel))
            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                cancel_handle.update(cx, |this, cx| this.cancel_rename(cx));
            }),
        )
        .action(
            button_style(
                action_button("rename-submit"),
                theme,
                ButtonVariant::Primary,
                true,
            )
            .when_some(controls.as_ref(), |button, controls| {
                button.track_focus(&controls[SUBMIT]).border(rems(0.0625)).border_color(ring(SUBMIT, theme.accent))
            })
            .child(cx.i18n().text(title))
            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                submit_handle.update(cx, |this, cx| this.submit_rename(cx));
            }),
        );
    div()
        .absolute()
        .inset_0()
        .on_key_down(move |event, window, cx| {
            key_handle.update(cx, |view, cx| {
                if view.archive_creation.is_some()
                    && view.compression_key(event.keystroke.key.as_str(), event.keystroke.modifiers.shift, window, cx)
                {
                    cx.stop_propagation();
                }
            })
        })
        .child(modal)
}

fn theme_accent(cx: &App) -> Hsla {
    cx.theme().accent
}
