use crate::ui::app::OxydeApp;
use crate::ui::components::modal::{action_button, Modal};
use crate::ui::i18n::{I18nContext, Message};
use crate::ui::theme::ThemeContext;
use gpui::prelude::*;
use gpui::*;

pub fn render_rename_dialog(
    view: &OxydeApp,
    handle: Entity<OxydeApp>,
    cx: &mut Context<OxydeApp>,
) -> impl IntoElement {
    let theme = *cx.theme();
    let input = view.editing_view.clone();
    let backdrop_handle = handle.clone();
    let cancel_handle = handle.clone();
    let submit_handle = handle;
    let error = view.rename_error.map(|message| cx.i18n().text(message));

    let body = div()
        .child(
            div().h(rems(2.0)).w_full().child(
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

    Modal::new("rename-dialog", cx.i18n().text(Message::RenameTitle), body)
        .title_icon("icons/pencil.svg")
        .width(rems(30.0))
        .body_gap(rems(0.375))
        .on_backdrop_dismiss(move |_, cx| {
            backdrop_handle.update(cx, |this, cx| this.cancel_rename(cx));
        })
        .action(
            action_button("rename-cancel")
                .hover(|d| d.bg(theme.control_hover))
                .text_color(theme.text_muted)
                .child(cx.i18n().text(Message::Cancel))
                .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                    cancel_handle.update(cx, |this, cx| this.cancel_rename(cx));
                }),
        )
        .action(
            action_button("rename-submit")
                .bg(theme.accent)
                .text_color(theme.text_on_accent)
                .hover(|d| d.opacity(0.85))
                .child(cx.i18n().text(Message::RenameTitle))
                .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                    submit_handle.update(cx, |this, cx| this.submit_rename(cx));
                }),
        )
}
