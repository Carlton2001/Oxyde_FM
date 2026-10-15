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

    let body = div()
        .child(
            div().w_full().child(
                input
                    .map(|input| input.into_any_element())
                    .unwrap_or_else(|| div().into_any_element()),
            ),
        )
        .when_some(error, |body, error| {
            body.child(
                div()
                    .mt(rems(0.625))
                    .text_size(rems(0.75))
                    .text_color(theme.error)
                    .child(error),
            )
        });

    let mut modal = Modal::new("rename-dialog", cx.i18n().text(title), body)
        .title_icon(if view.archive_creation.is_some() {
            "icons/archive.svg"
        } else {
            "icons/pencil.svg"
        })
        .width(rems(30.0))
        .body_gap(rems(0.375))
        .on_backdrop_dismiss(move |_, cx| {
            backdrop_handle.update(cx, |this, cx| this.cancel_rename(cx));
        });
    if let Some((_, extension)) = &view.archive_creation {
        let mut formats = div().flex().flex_1().min_w_0().gap(rems(0.5));
        for (format, label, suffix) in [
            ("zip", "Zip", "zip"),
            ("7z", "7z", "7z"),
            ("tar", "Tar", "tar"),
            ("zst", "Zst", "tar.zst"),
        ] {
            let format_handle = cancel_handle.clone();
            formats = formats.child(
                button_style(
                    action_button(format!("archive-format-{format}"))
                        .w(rems(3.25))
                        .flex_none()
                        .text_center(),
                    theme,
                    if extension == suffix {
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
        modal = modal.action(formats);
    }
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
