use super::{content::Content, markdown::is_markdown, Viewer};
use crate::ui::{
    components::tooltip::Tooltip,
    elements::button::toolbar_button_with_tooltip,
    i18n::{I18nContext, Message},
    theme::ThemeContext,
};
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
                .files
                .get(self.index)
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
        toolbar_button_with_tooltip(
            id.into(),
            Tooltip::text(cx.i18n().text(target)),
            Some(icon),
            Some(cx.i18n().text(label).to_string()),
            *cx.theme(),
            true,
            cx.listener(move |this, _, _, cx| this.toggle_display(markdown, cx)),
        )
        .w_auto()
        .px(rems(0.375))
        .gap(rems(0.375))
        .text_size(rems(0.8125))
        .when(!markdown && self.syntax_highlighting, |button| button.bg(cx.theme().control_hover))
    }

    pub(super) fn render_display_options(&self, cx: &mut Context<Self>) -> Div {
        let markdown = self.files.get(self.index).is_some_and(|file| is_markdown(&file.path));
        div().flex().flex_none().items_center().gap(rems(0.25))
            .when(self.native_media, |bar| {
                bar.child(toolbar_button_with_tooltip(
                    "viewer-autoplay".into(),
                    Tooltip::text(cx.i18n().text(if self.media_autoplay {
                        Message::ViewerDisableAutoplay
                    } else { Message::ViewerEnableAutoplay })),
                    Some("icons/play.svg"),
                    Some(cx.i18n().text(Message::ViewerAutoplay).to_string()),
                    *cx.theme(), true,
                    cx.listener(|this, _, _, cx| this.toggle_autoplay(cx)),
                ).w_auto().px(rems(0.375)).gap(rems(0.375)).text_size(rems(0.8125))
                    .when(self.media_autoplay, |button| button.bg(cx.theme().control_hover)))
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
