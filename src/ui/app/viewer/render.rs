use super::Viewer;
use crate::ui::{
    components::tooltip::{NativeTooltipExt, Tooltip},
    i18n::{I18nContext, Message},
    theme::ThemeContext,
};
use gpui::{prelude::*, *};

pub(super) fn icon_button(
    id: &'static str,
    icon: &'static str,
    label: Message,
    enabled: bool,
    cx: &App,
) -> Stateful<Div> {
    let theme = *cx.theme();
    div()
        .id(id)
        .size(rems(2.0))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded_md()
        .text_color(theme.text)
        .when(enabled, |button| {
            button.hover(move |style| style.bg(theme.control_hover))
        })
        .native_tooltip(Tooltip::text(cx.i18n().text(label)))
        .child(
            svg()
                .path(icon)
                .size(rems(1.0))
                .text_color(theme.text)
                .when(!enabled, |icon| icon.opacity(0.35)),
        )
}

impl Render for Viewer {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // A shared Viewer may be drawn by both windows. Only the secondary window
        // renders its preview and native player while detached.
        if self.detached
            && window
                .window_handle()
                .downcast::<super::window::ViewerWindow>()
                .is_none()
        {
            return self.render_detached(cx).into_any_element();
        }
        if self.focus_pending {
            self.focus_pending = false;
            window.focus(&self.focus, cx);
        }
        let theme = *cx.theme();
        let current = self.files.get(self.index);
        let has_previous = self.index > 0;
        let has_next = self.index + 1 < self.files.len();
        let failed = self
            .media
            .as_ref()
            .is_some_and(|media| media.control.status() == oxyde_lib::platform::MEDIA_FAILED);
        let title = current.map(|file| file.name.clone()).unwrap_or_default();
        let surface = div()
            .id("viewer-surface")
            .size_full()
            .relative()
            .min_w_0()
            .min_h_0()
            .flex()
            .flex_col()
            .bg(theme.background)
            .text_color(theme.text)
            .text_size(rems(0.8125))
            .track_focus(&self.focus)
            .key_context("Viewer")
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, window, cx| this.activate(window, cx)),
            )
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                if event.keystroke.modifiers.control || event.keystroke.modifiers.alt {
                    return;
                }
                match event.keystroke.key.as_str() {
                    "left" => this.step(false, cx),
                    "right" => this.step(true, cx),
                    "space" => this.toggle_media(cx),
                    _ => return,
                }
                cx.stop_propagation();
            }))
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .w_full()
                    .relative()
                    .pt(rems(2.75))
                    .pb(rems(3.0))
                    .px(rems(1.0))
                    .child(div().size_full().flex().flex_col().min_h_0().min_w_0().map(
                        |content| {
                            if self.native_media {
                                let handle = cx.entity().downgrade();
                                content
                                    .relative()
                                    .items_center()
                                    .justify_center()
                                    .when(self.audio && !failed, |content| {
                                        content.child(
                                            svg()
                                                .path("icons/music.svg")
                                                .size(rems(5.0))
                                                .text_color(theme.text_muted),
                                        )
                                    })
                                    .when(failed, |content| {
                                        content.child(cx.i18n().text(Message::PreviewUnavailable))
                                    })
                                    .child(
                                        canvas(
                                            |bounds, _, _| bounds,
                                            move |_, bounds, window, cx| {
                                                let _ = handle.update(cx, |this, cx| {
                                                    this.place_media(bounds, window, cx)
                                                });
                                            },
                                        )
                                        .absolute()
                                        .size_full(),
                                    )
                            } else if let Some(preview) = &self.preview {
                                content.child(preview.clone())
                            } else {
                                content
                                    .items_center()
                                    .justify_center()
                                    .text_color(theme.text_muted)
                                    .child(cx.i18n().text(if self.preparation.is_some() {
                                        Message::PreviewLoading
                                    } else if current.is_none() {
                                        Message::ViewerEmpty
                                    } else {
                                        Message::PreviewUnavailable
                                    }))
                            }
                        },
                    )),
            )
            .child(
                div()
                    .absolute()
                    .top(rems(0.5))
                    .left(rems(1.0))
                    .right(rems(3.0))
                    .truncate()
                    .child(title),
            )
            .when(!self.detached, |surface| {
                surface.child(
                    div()
                        .absolute()
                        .top(rems(0.375))
                        .right(rems(0.375))
                        .size(rems(2.0))
                        .child(
                            icon_button(
                                "viewer-detach",
                                "icons/external-link.svg",
                                Message::ViewerDetach,
                                true,
                                cx,
                            )
                            .on_click(cx.listener(|this, _, window, cx| this.detach(window, cx))),
                        ),
                )
            })
            .child(
                div()
                    .absolute()
                    .bottom(crate::ui::style::STATUS_BAR_HEIGHT + rems(0.375))
                    .w_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .gap(rems(0.5))
                    .when(self.files.len() > 1, |controls| {
                        controls
                            .child(
                                icon_button(
                                    "viewer-previous",
                                    "icons/chevron-left.svg",
                                    Message::ViewerPrevious,
                                    has_previous,
                                    cx,
                                )
                                .on_click(cx.listener(
                                    move |this, _, _, cx| {
                                        if has_previous {
                                            this.step(false, cx);
                                        }
                                    },
                                )),
                            )
                            .child(div().text_color(theme.text_muted).child(format!(
                                "{} / {}",
                                self.index + 1,
                                self.files.len()
                            )))
                    })
                    .when(self.native_media, |controls| {
                        controls.child(
                            icon_button(
                                "viewer-play",
                                if self.media_paused() {
                                    "icons/play.svg"
                                } else {
                                    "icons/pause.svg"
                                },
                                if self.media_paused() {
                                    Message::ViewerPlay
                                } else {
                                    Message::ViewerPause
                                },
                                true,
                                cx,
                            )
                            .on_click(cx.listener(|this, _, _, cx| this.toggle_media(cx))),
                        )
                    })
                    .when(self.files.len() > 1, |controls| {
                        controls.child(
                            icon_button(
                                "viewer-next",
                                "icons/chevron-right.svg",
                                Message::ViewerNext,
                                has_next,
                                cx,
                            )
                            .on_click(cx.listener(
                                move |this, _, _, cx| {
                                    if has_next {
                                        this.step(true, cx);
                                    }
                                },
                            )),
                        )
                    }),
            )
            .child(
                div()
                    .h(crate::ui::style::STATUS_BAR_HEIGHT)
                    .w_full()
                    .flex_none()
                    .bg(theme.surface)
                    .border_t(rems(0.0625))
                    .border_color(theme.border),
            );
        surface.into_any_element()
    }
}

impl Viewer {
    fn render_detached(&self, cx: &mut Context<Self>) -> Div {
        let theme = *cx.theme();
        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(theme.background)
            .text_color(theme.text_muted)
            .text_size(rems(0.8125))
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .gap(rems(0.5))
                    .child(cx.i18n().text(Message::ViewerDetached))
                    .child(
                        icon_button(
                            "viewer-show-window",
                            "icons/external-link.svg",
                            Message::ViewerShowWindow,
                            true,
                            cx,
                        )
                        .on_click(cx.listener(|this, _, window, cx| this.detach(window, cx))),
                    ),
            )
            .child(
                div()
                    .h(crate::ui::style::STATUS_BAR_HEIGHT)
                    .w_full()
                    .flex_none()
                    .bg(theme.surface)
                    .border_t(rems(0.0625))
                    .border_color(theme.border),
            )
    }
}
