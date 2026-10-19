use super::{PreviewData, PreviewTask};
use crate::ui::{
    components::{icons::PreviewType, scroll_area::scroll_area},
    i18n::{I18nContext, Message},
    theme::ThemeContext,
};
use gpui::{prelude::*, *};

impl PreviewTask {
    pub(crate) fn new_viewer(path: String, kind: PreviewType, cx: &mut Context<Self>) -> Self {
        let mut preview = Self::new(path, kind, cx);
        preview.viewer = true;
        preview
    }

    pub(super) fn render_viewer(&self, window: &mut Window, cx: &mut Context<Self>) -> Div {
        let theme = *cx.theme();
        let frame = div()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .min_h_0()
            .min_w_0();
        if let Some((image, _)) = self.loaded_image(window, cx) {
            return frame.child(
                img(image)
                    .id("viewer-preview-image")
                    .size_full()
                    .object_fit(ObjectFit::Contain),
            );
        }
        if let Some(PreviewData::Text(text)) = &self.data {
            return scroll_area(
                "viewer-text-scroll",
                div().size_full(),
                div()
                    .id("viewer-text")
                    .size_full()
                    .p(rems(1.0))
                    .font_family("JetBrains Mono")
                    .text_size(rems(0.8125))
                    .text_color(theme.text)
                    .child(text.clone()),
                true,
                window,
                cx,
            );
        }
        let message = match self.preview_message(window, cx) {
            None | Some(Message::PreviewLoading) => Message::ViewerLoading,
            Some(message) => message,
        };
        frame
            .text_color(theme.text_muted)
            .child(cx.i18n().text(message))
    }
}
