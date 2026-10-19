//! Presentation of complete Viewer content. Long content is virtualized by
//! `list`: only visible text blocks and PDF pages are laid out or decoded.
use super::{
    content::{Block, BlockKind, Content},
    Viewer,
};
use crate::ui::{
    i18n::{I18nContext, Message},
    theme::ThemeContext,
};
use gpui::{prelude::*, *};

fn message(text: SharedString, cx: &App) -> AnyElement {
    div()
        .size_full()
        .flex()
        .items_center()
        .justify_center()
        .text_color(cx.theme().text_muted)
        .child(text)
        .into_any_element()
}

fn block(block: &Block, cx: &App) -> AnyElement {
    let theme = *cx.theme();
    let row = div().w_full().px(rems(2.0)).text_color(theme.text);
    match block.kind {
        BlockKind::Title => row
            .pt(rems(1.0))
            .pb(rems(0.5))
            .text_size(rems(1.75))
            .font_weight(FontWeight::BOLD)
            .child(block.text.clone()),
        BlockKind::Heading(level) => row
            .pt(rems(if level == 1 { 1.0 } else { 0.75 }))
            .pb(rems(0.25))
            .text_size(rems(match level {
                1 => 1.375,
                2 => 1.125,
                3 => 1.0,
                _ => 0.9375,
            }))
            .font_weight(FontWeight::SEMIBOLD)
            .child(block.text.clone()),
        BlockKind::Item => row.py(rems(0.125)).flex().gap(rems(0.5)).children([
            div().flex_none().text_color(theme.text_muted).child("•"),
            div().flex_1().min_w_0().child(block.text.clone()),
        ]),
        BlockKind::Row => row.flex().children(block.cells.iter().map(|cell| {
            div()
                .flex_1()
                .min_w_0()
                .px(rems(0.375))
                .py(rems(0.25))
                .border_1()
                .border_color(theme.border)
                .child(cell.clone())
        })),
        BlockKind::Sheet | BlockKind::Slide => row
            .mt(rems(1.0))
            .mb(rems(0.375))
            .py(rems(0.25))
            .border_b_1()
            .border_color(theme.border)
            .text_color(theme.text_muted)
            .font_weight(FontWeight::SEMIBOLD)
            .child(if block.kind == BlockKind::Slide && block.text.chars().all(|c| c.is_ascii_digit()) {
                format!("{} {}", cx.i18n().text(Message::ViewerSlide), block.text).into()
            } else {
                block.text.clone()
            }),
        BlockKind::Paragraph => row
            .py(rems(0.25))
            .min_h(rems(0.75))
            .child(block.text.clone()),
    }
    .into_any_element()
}

impl Viewer {
    pub(super) fn render_content(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let theme = *cx.theme();
        let handle = cx.entity().downgrade();
        match &mut self.content {
            Content::Pdf(pdf) => pdf.update_visible(),
            Content::Text(_) => self.sync_text_layout(window, cx),
            _ => {}
        }
        match &self.content {
            Content::None if self.preparation.is_some() => {
                message(cx.i18n().text(Message::PreviewLoading), cx)
            }
            Content::None if self.files.get(self.index).is_none() => {
                message(cx.i18n().text(Message::ViewerEmpty), cx)
            }
            Content::None | Content::Unavailable => {
                message(cx.i18n().text(Message::PreviewUnavailable), cx)
            }
            Content::Loading => message(cx.i18n().text(Message::PreviewLoading), cx),
            // The absolute box follows the pane or window, never the image's own
            // size; GPUI's img takes its intrinsic size unless sized explicitly.
            Content::Image(image) => div()
                .size_full()
                .relative()
                .child(
                    div()
                        .absolute()
                        .top_0()
                        .bottom_0()
                        .left(rems(1.0))
                        .right(rems(1.0))
                        .child(
                            img(image.clone())
                                .id("viewer-image")
                                .size_full()
                                .object_fit(ObjectFit::ScaleDown),
                        ),
                )
                .into_any_element(),
            Content::Preview(preview) => preview.clone().into_any_element(),
            Content::Text(text) => {
                use super::text::{ROW_HEIGHT, TEXT_PADDING, TEXT_SIZE};
                let rows = text.rows();
                let body = list(text.list.clone(), move |index, _, cx| {
                    div()
                        .w_full()
                        .h(ROW_HEIGHT)
                        .px(TEXT_PADDING)
                        .flex()
                        .items_center()
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .font_family("JetBrains Mono")
                        .text_size(TEXT_SIZE)
                        .text_color(cx.theme().text)
                        .child(rows.row(index).unwrap_or_default())
                        .into_any_element()
                })
                .size_full();
                div()
                    .size_full()
                    .flex()
                    .flex_col()
                    .when(text.truncated, |frame| {
                        frame.child(
                            div()
                                .flex_none()
                                .px(TEXT_PADDING)
                                .py(rems(0.25))
                                .text_color(theme.text_muted)
                                .child(cx.i18n().text(Message::ViewerTextTruncated)),
                        )
                    })
                    .child(div().flex_1().min_h_0().child(body))
                    .into_any_element()
            }
            Content::Document(blocks, state) => {
                let blocks = blocks.clone();
                list(state.clone(), move |index, _, cx| match blocks.get(index) {
                    Some(item) => block(item, cx),
                    None => div().into_any_element(),
                })
                .size_full()
                .into_any_element()
            }
            Content::Pdf(pdf) => {
                let sizes = pdf.sizes.clone();
                let _ = window;
                list(pdf.list.clone(), move |index, _, cx| {
                    let (width, height) = sizes.get(index).copied().unwrap_or((1.0, 1.0));
                    let loaded = handle
                        .update(cx, |this, cx| {
                            this.request_pdf_page(index, cx);
                            match &this.content {
                                Content::Pdf(pdf) => {
                                    (pdf.pages.get(&index).cloned(), pdf.failed.contains(&index))
                                }
                                _ => (None, false),
                            }
                        })
                        .unwrap_or((None, false));
                    let page = div()
                        .w_full()
                        .max_w(px(width * 2.0))
                        .aspect_ratio(width / height)
                        .bg(theme.surface)
                        .shadow_sm();
                    let page = match loaded {
                        (Some(image), _) => page.child(img(image).size_full().object_fit(ObjectFit::Contain)),
                        (None, true) => page
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_color(theme.text_muted)
                            .child(cx.i18n().text(Message::PreviewUnavailable)),
                        (None, false) => page,
                    };
                    div()
                        .w_full()
                        .px(rems(1.0))
                        .pb(rems(0.75))
                        .flex()
                        .justify_center()
                        .child(page)
                        .into_any_element()
                })
                .size_full()
                .into_any_element()
            }
        }
    }
}
