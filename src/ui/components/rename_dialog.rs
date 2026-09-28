use crate::ui::app::OxydeApp;
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
    let error = view.rename_error.clone();

    div()
        .absolute()
        .inset_0()
        .flex()
        .items_center()
        .justify_center()
        .bg(rgba(0x00000088u32))
        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
            backdrop_handle.update(cx, |this, cx| this.cancel_rename(cx));
        })
        .child(
            div()
                .id("rename-dialog")
                .w(px(480.0))
                .p(px(20.0))
                .rounded_lg()
                .bg(theme.surface)
                .text_color(theme.text)
                .border_1()
                .border_color(theme.border)
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(
                    div()
                        .text_size(px(16.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(theme.text)
                        .child("Renommer"),
                )
                .child(
                    div().mt(px(16.0)).h(px(52.0)).w_full().child(
                        input
                            .map(|input| input.into_any_element())
                            .unwrap_or_else(|| div().into_any_element()),
                    ),
                )
                .when_some(error, |dialog, error| {
                    dialog.child(
                        div()
                            .mt(px(10.0))
                            .text_size(px(12.0))
                            .text_color(rgb(0xe06c75))
                            .child(error),
                    )
                })
                .child(
                    div()
                        .mt(px(20.0))
                        .flex()
                        .justify_end()
                        .gap(px(8.0))
                        .child(
                            div()
                                .id("rename-cancel")
                                .px(px(12.0))
                                .py(px(8.0))
                                .rounded_md()
                                .cursor_pointer()
                                .hover(|d| d.bg(theme.header))
                                .text_color(theme.text_muted)
                                .child("Annuler")
                                .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                    cancel_handle.update(cx, |this, cx| this.cancel_rename(cx));
                                }),
                        )
                        .child(
                            div()
                                .id("rename-submit")
                                .px(px(12.0))
                                .py(px(8.0))
                                .rounded_md()
                                .cursor_pointer()
                                .bg(theme.accent)
                                .text_color(theme.background)
                                .hover(|d| d.opacity(0.85))
                                .child("Renommer")
                                .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                    submit_handle.update(cx, |this, cx| this.submit_rename(cx));
                                }),
                        ),
                ),
        )
}
