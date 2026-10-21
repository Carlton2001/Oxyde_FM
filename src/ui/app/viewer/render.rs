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
        .fixed_tooltip(Tooltip::text(cx.i18n().text(label)))
        .child(
            svg()
                .path(icon)
                .size(rems(1.0))
                .text_color(theme.text)
                .when(!enabled, |icon| icon.opacity(0.35)),
        )
}

/// Toolbar button with the explorer navigation bar's metrics.
pub(super) fn bar_button(
    id: &'static str,
    icon: &'static str,
    label: Option<SharedString>,
    tooltip: SharedString,
    active: bool,
    cx: &App,
) -> Stateful<Div> {
    let theme = *cx.theme();
    div()
        .id(id)
        .flex_none()
        .min_w(rems(1.5))
        .h_6()
        .px(rems(0.125))
        .flex()
        .items_center()
        .justify_center()
        .rounded_md()
        .text_color(theme.text)
        .when(active, |button| button.bg(theme.control_hover))
        .hover(move |style| style.bg(theme.control_hover))
        .fixed_tooltip(Tooltip::text(tooltip))
        .child(svg().path(icon).size_4().text_color(theme.text))
        .when_some(label, |button, label| {
            button.px(rems(0.375)).gap(rems(0.375)).text_size(rems(0.8125)).child(label)
        })
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
        if self.fullscreen_pending && self.detached {
            self.fullscreen_pending = false;
            if !window.is_fullscreen() { window.toggle_fullscreen(); }
        }
        if self.focus_pending {
            self.focus_pending = false;
            window.focus(&self.focus, cx);
        }
        if !matches!(self.content, super::content::Content::Image(_)) {
            use raw_window_handle::HasWindowHandle;
            if let Ok(handle) = HasWindowHandle::window_handle(window) {
                oxyde_lib::platform::platform().viewer_image_cursor(handle.as_raw(), false, false);
            }
        }
        let theme = *cx.theme();

        let failed = self
            .media
            .as_ref()
            .is_some_and(|media| media.control.status() == oxyde_lib::platform::MEDIA_FAILED);
        self.enable_fullscreen_controls(window.is_fullscreen() && self.native_media && !self.audio && !failed && !self.blocked, cx);
        let overlay = self.render_overlay(window, failed, cx);
        let current = self.file.as_ref();
        let title = current.map(|file| file.name.clone()).unwrap_or_default();
        let surface = div()
            .id("viewer-surface")
            .size_full()
            .relative()
            .min_w_0()
            .min_h_0()
            .flex()
            .flex_col()
            .bg(theme.darkest_background())
            .when(window.is_fullscreen(), |surface| surface.bg(rgb(0x000000)))
            .text_color(theme.text)
            .text_size(rems(0.8125))
            .track_focus(&self.focus)
            .key_context("Viewer")
            .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, _, cx| {
                this.controls_pointer_moved(event.position, cx);
                this.overlay_pointer_moved(Some(event.position), cx);
            }))
            .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                if !*hovered {
                    this.overlay_pointer_moved(None, cx);
                }
            }))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, window, cx| this.activate(window, cx)),
            )
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                if event.keystroke.key == "escape" && window.is_fullscreen() {
                    this.exit_fullscreen(window, cx);
                    cx.stop_propagation();
                    return;
                }
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
            .when(!window.is_fullscreen() && !self.detached, |surface| surface.child(self.render_action_bar(title, cx)))
            .child(
                // Content fills the remaining space; its rail reaches the pane's right edge.
                div()
                    .flex_1()
                    .min_h_0()
                    .w_full()
                    .relative()
                    .on_mouse_down(MouseButton::Left, cx.listener(|this, event: &MouseDownEvent, window, cx| {
                        // Every press in an edge band skips, so quick clicks never toggle fullscreen.
                        if this.overlay_click(event.position, cx) {
                            cx.stop_propagation();
                            return;
                        }
                        if event.click_count == 2 && (matches!(this.content, super::content::Content::Image(_)) || (this.native_media && !this.audio)) {
                            this.toggle_preview_fullscreen(window, cx);
                            cx.stop_propagation();
                        }
                    }))
                    // Resolve percentage heights against the remaining viewport,
                    // rather than letting intrinsic image dimensions size this flex item.
                    .child(div().absolute().inset_0().size_full().flex().flex_col().min_h_0().min_w_0().map(
                        |content| {
                            if self.native_media && !self.audio && !failed {
                                content.child(self.render_video(window, cx))
                            } else if self.native_media {
                                let handle = cx.entity().downgrade();
                                content
                                    .relative()
                                    .items_center()
                                    .justify_center()
                                    .when(self.audio, |content| {
                                        content.child(self.render_audio(window, cx))
                                    })
                                    .when(failed, |content| {
                                        content.child(self.render_unavailable_file(cx))
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
                            } else {
                                content.child({
                                    let body = self.render_content(window, cx);
                                    self.with_scrollbar(body, cx)
                                })
                            }
                        },
                    ))
                    .child(overlay),
            );
        surface.into_any_element()
    }
}

impl Viewer {
    /// The file name leads in the primary text color.
    fn render_file_title(&self, title: String, cx: &App) -> Div {
        div().flex_1().min_w_0().truncate().text_color(cx.theme().text)
            .font_weight(FontWeight::SEMIBOLD).child(title)
    }

    fn render_action_bar(&self, title: String, cx: &mut Context<Self>) -> Div {
        let theme = *cx.theme();
        div()
            .h(crate::ui::style::NAV_BAR_HEIGHT)
            .w_full()
            .flex_none()
            .flex()
            .items_center()
            .pl(rems(0.5))
            .pr(rems(0.5))
            .gap_1()
            .bg(theme.surface)
            .border_b(rems(0.0625))
            .border_color(theme.border)
            .child(self.render_file_title(title, cx))
            .child(self.render_display_options(cx))
            .when(!self.detached, |bar| {
                bar.child(
                    bar_button(
                        "viewer-detach",
                        "icons/external-link.svg",
                        None,
                        cx.i18n().text(Message::ViewerDetach),
                        false,
                        cx,
                    )
                    .on_click(cx.listener(|this, _, window, cx| this.detach(window, cx))),
                )
            })
    }

    fn render_detached(&self, cx: &mut Context<Self>) -> Div {
        let theme = *cx.theme();
        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(theme.darkest_background())
            .text_color(theme.text_muted)
            .text_size(rems(0.8125))
            .child(self.render_action_bar(String::new(), cx))
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
    }
}
