use crate::ui::app::OxydeApp;
use crate::ui::components::modal::{action_button, Modal};
use crate::ui::elements::button::{button_style, ButtonVariant};
use crate::ui::elements::segmented::{outline, segment, segmented};
use crate::ui::i18n::{FluentArgs, I18nContext, Message};
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
    // A refused password shakes and explains its own field, not the name.
    let password_error = match view.rename_error {
        Some(Message::ArchivePasswordEmpty | Message::ArchivePasswordTooLong) => Some(0),
        Some(Message::ArchivePasswordMismatch) => Some(1),
        _ => None,
    };
    let error = view.rename_error.map(|message| cx.i18n().text(message));
    let name_error = error.clone().filter(|_| password_error.is_none());

    // Formats and protection are single choices: each one is a segmented row.
    let formats = view.archive_creation.as_ref().map(|(_, extension)| {
        let mut formats = segmented("archive-formats", Some(ring(FORMATS, outline(theme))), theme)
            .w_full();
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
            .w_full();
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
                let refused = password_error == Some(index);
                let field = if refused { shake(field, view.rename_shake_id, index) } else { field.into_any_element() };
                section = section.child(
                    div().mt(rems(0.5)).child(crate::ui::components::modal::form_row(cx.i18n().text(label), field, cx)),
                );
                if let Some(error) = error.clone().filter(|_| refused) {
                    section = section.child(crate::ui::components::modal::field_error(error, theme).mt(rems(0.375)));
                }
            }
        }
        section
    });
    let compressing = view.archive_creation.is_some();
    let path = view.editing_path.clone().unwrap_or_default();
    let original = std::path::Path::new(&path)
        .file_name()
        .map(|name| oxyde_lib::utils::path_text::display(&name.to_string_lossy()))
        .unwrap_or_default();
    // What is renamed, or where the archive goes, above the field.
    let context = if compressing {
        let folder = std::path::Path::new(&path).parent().map(|parent| parent.to_string_lossy().into_owned()).unwrap_or_default();
        div()
            .flex()
            .items_center()
            .gap(rems(0.5))
            .min_w_0()
            .mb(rems(0.5))
            .text_color(theme.text_muted)
            .child(svg().path("icons/folder.svg").size(rems(0.875)).flex_none().text_color(theme.text_muted))
            .child(div().min_w_0().truncate().child(folder))
    } else {
        let icon = crate::ui::components::icons::get_icon_type(&original, view.editing_is_dir, Some(&path), false, false);
        div()
            .flex()
            .items_center()
            .gap(rems(0.5))
            .min_w_0()
            .mb(rems(0.5))
            .text_color(theme.text_muted)
            .child(div().flex_none().child(crate::ui::components::icons::render_file_icon(
                path.clone(),
                icon,
                rems(1.0),
                &theme,
                cfg!(target_os = "windows") && view.use_system_icons(),
                false,
            )))
            .child(div().min_w_0().truncate().child(original.clone()))
    };
    let extension_notice = (!compressing)
        .then(|| extension_change(&original, &view.editing_text, view.editing_is_dir))
        .flatten()
        .map(|(from, to)| {
            let mut args = FluentArgs::new();
            args.set("from", from);
            let message = match to {
                Some(to) => {
                    args.set("to", to);
                    Message::RenameExtensionChanged
                }
                None => Message::RenameExtensionRemoved,
            };
            div()
                .mt(rems(0.5))
                .flex()
                .items_center()
                .gap(rems(0.375))
                .text_size(rems(0.75))
                .text_color(theme.warning_text)
                .child(svg().path("icons/triangle-alert.svg").size(rems(0.75)).flex_none().text_color(theme.warning_text))
                .child(cx.i18n().text_with_args(message, &args))
        });
    let body = div()
        .child(context)
        .child({
            let field = div().w_full().child(
                input
                    .map(|input| input.into_any_element())
                    .unwrap_or_else(|| div().into_any_element()),
            );
            // A refused name shakes the field, as the path bar and inline rename do.
            if name_error.is_some() {
                shake(field, view.rename_shake_id, usize::MAX)
            } else {
                field.into_any_element()
            }
        })
        .children(extension_notice)
        .children(formats.map(|formats| div().mt(rems(0.875)).child(label(cx.i18n().text(Message::CompressionFormat), theme)).child(formats)))
        .children(protection.map(|protection| div().mt(rems(0.875)).child(label(cx.i18n().text(Message::CompressionProtection), theme)).child(protection)))
        .when_some(name_error, |body, error| {
            body.child(crate::ui::components::modal::field_error(error, theme).mt(rems(0.5)))
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

/// Shakes a refused field once per refusal; `field` names it among the dialog's.
fn shake(element: impl IntoElement + Styled + 'static, refusal: u64, field: usize) -> AnyElement {
    element
        .with_animation(
            SharedString::from(format!("rename-dialog-shake-{field}-{refusal}")),
            Animation::new(std::time::Duration::from_millis(300)),
            |element, delta| {
                let amplitude = (1.0 - delta) * crate::ui::scale::logical(8.0);
                let offset = (delta * std::f32::consts::PI * 6.0).sin() * amplitude;
                element.relative().left(px(offset))
            },
        )
        .into_any_element()
}

/// A small uppercase label above a group of choices.
fn label(text: SharedString, theme: crate::ui::theme::OxydeTheme) -> Div {
    div()
        .mb(rems(0.375))
        .text_size(rems(0.6875))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(theme.text_muted)
        .child(text.to_uppercase())
}

/// The extension a file name loses or changes to, when it does; folders have none.
fn extension_change(original: &str, edited: &str, is_dir: bool) -> Option<(String, Option<String>)> {
    if is_dir {
        return None;
    }
    let extension = |name: &str| std::path::Path::new(name).extension().map(|extension| extension.to_string_lossy().into_owned());
    let from = extension(original)?;
    let to = extension(edited.trim());
    (to.as_deref() != Some(from.as_str())).then_some((from, to))
}

fn theme_accent(cx: &App) -> Hsla {
    cx.theme().accent
}

#[cfg(test)]
#[path = "rename_dialog/tests/extension.rs"]
mod tests;
