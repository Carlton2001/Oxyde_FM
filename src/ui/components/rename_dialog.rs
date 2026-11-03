use crate::ui::app::OxydeApp;
use crate::ui::components::modal::{action_button, Modal};
use crate::ui::elements::button::{button_style, ButtonVariant};
use crate::ui::i18n::{I18nContext, Message};
use crate::ui::theme::ThemeContext;
use gpui::prelude::*;
use gpui::*;

pub fn render_rename_dialog(
    view: &OxydeApp,
    handle: Entity<OxydeApp>,
    cx: &mut Context<OxydeApp>,
) -> impl IntoElement {
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

    // Eight formats do not fit beside the actions: they get their own row.
    let formats = view.archive_creation.as_ref().map(|(_, extension)| {
        let mut formats = div().flex().w_full().mt(rems(0.625)).gap(rems(0.375));
        for (format, label) in OxydeApp::ARCHIVE_FORMATS {
            let format_handle = cancel_handle.clone();
            formats = formats.child(
                button_style(
                    action_button(format!("archive-format-{format}"))
                        .flex_1()
                        .min_w_0()
                        .text_center()
                        .whitespace_nowrap(),
                    theme,
                    if *extension == crate::ui::app::archive_extension(format) {
                        ButtonVariant::Primary
                    } else {
                        ButtonVariant::Secondary
                    },
                    true,
                )
                .child(label)
                .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                    format_handle.update(cx, |this, cx| this.select_archive_format(format, cx));
                }),
            );
        }
        formats
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
        // Eight format buttons need room for "Bz2" and "Lz4" on one line.
        .width(rems(if view.archive_creation.is_some() { 34.0 } else { 30.0 }))
        .body_gap(rems(0.375))
        .on_backdrop_dismiss(move |_, cx| {
            backdrop_handle.update(cx, |this, cx| this.cancel_rename(cx));
        });
    modal
        .action(
            button_style(
                action_button("rename-cancel"),
                theme,
                ButtonVariant::Secondary,
                true,
            )
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
            .child(cx.i18n().text(title))
            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                submit_handle.update(cx, |this, cx| this.submit_rename(cx));
            }),
        )
}
