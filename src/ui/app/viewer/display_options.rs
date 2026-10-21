use super::{content::Content, markdown::is_markdown, render::bar_button, Viewer};
use crate::ui::i18n::{I18nContext, Message};
use gpui::{prelude::*, *};

impl Viewer {
    fn toggle_autoplay(&mut self, cx: &mut Context<Self>) {
        self.media_autoplay = !self.media_autoplay;
        let enabled = self.media_autoplay;
        if let Some(owner) = self.owner.upgrade() {
            owner.update(cx, |app, cx| {
                app.config.viewer_media_autoplay = enabled;
                app.save_preferences(cx);
                cx.notify();
            });
        }
        cx.notify();
    }

    pub(super) fn sync_display_options(
        &mut self,
        rendered: bool,
        highlighting: bool,
        cx: &mut Context<Self>,
    ) {
        let reload = self.markdown_rendered != rendered
            && self
                .file
                .as_ref()
                .is_some_and(|file| is_markdown(&file.path));
        self.markdown_rendered = rendered;
        self.syntax_highlighting = highlighting;
        if reload {
            self.load_current(cx);
        }
    }

    fn toggle_display(&mut self, markdown: bool, cx: &mut Context<Self>) {
        let rendered = if markdown {
            !self.markdown_rendered
        } else {
            self.markdown_rendered
        };
        let highlighting = if markdown {
            self.syntax_highlighting
        } else {
            !self.syntax_highlighting
        };
        self.sync_display_options(rendered, highlighting, cx);
        if let Some(owner) = self.owner.upgrade() {
            owner.update(cx, |app, cx| {
                app.config.viewer_markdown_rendered = rendered;
                app.config.viewer_syntax_highlighting = highlighting;
                app.save_preferences(cx);
                cx.notify();
            });
        }
        cx.notify();
    }

    fn display_mode_button(
        &self,
        id: &'static str,
        icon: &'static str,
        label: Message,
        target: Message,
        markdown: bool,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        bar_button(
            id,
            icon,
            Some(cx.i18n().text(label)),
            cx.i18n().text(target),
            !markdown && self.syntax_highlighting,
            cx,
        )
        .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| this.toggle_display(markdown, cx)))
    }

    pub(super) fn render_display_options(&self, cx: &mut Context<Self>) -> Div {
        let markdown = self.file.as_ref().is_some_and(|file| is_markdown(&file.path));
        div().flex().flex_none().items_center().gap_1()
            .when(self.native_media, |bar| {
                bar.child(bar_button(
                    "viewer-autoplay",
                    "icons/play.svg",
                    Some(cx.i18n().text(Message::ViewerAutoplay)),
                    cx.i18n().text(if self.media_autoplay {
                        Message::ViewerDisableAutoplay
                    } else { Message::ViewerEnableAutoplay }),
                    self.media_autoplay,
                    cx,
                )
                .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| this.toggle_autoplay(cx))))
            })
            .when(markdown, |bar| {
                let (icon, label, target) = if self.markdown_rendered {
                    ("icons/eye.svg", Message::ViewerRendered, Message::ViewerCode)
                } else {
                    ("icons/code.svg", Message::ViewerCode, Message::ViewerRendered)
                };
                bar.child(self.display_mode_button("viewer-markdown-mode", icon, label, target, true, cx))
            })
            .when(!markdown && matches!(&self.content, Content::Text(text) if text.has_highlighting()), |bar| {
                let target = if self.syntax_highlighting {
                    Message::ViewerDisableHighlighting
                } else {
                    Message::ViewerEnableHighlighting
                };
                bar.child(self.display_mode_button("viewer-syntax-mode", "icons/file-code.svg", Message::ViewerHighlighted, target, false, cx))
            })
    }
}
