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
    pub(super) fn render_unavailable_file(&self, cx: &mut Context<Self>) -> AnyElement {
        self.unavailable_file_notice(cx).size_full().into_any_element()
    }

    /// Sized by its parent: the whole Viewer, or the player bar's place under an audio card.
    pub(super) fn unavailable_file_notice(&self, cx: &mut Context<Self>) -> Div {
        let owner = self.owner.clone();
        let file = self.file.clone();
        let source_tab = self.source.tab.clone();
        div().flex().flex_col().items_center().justify_center()
            .gap(rems(1.0)).p(rems(1.0))
            .text_color(cx.theme().text_muted)
            .child(cx.i18n().text(Message::ViewerFileUnavailable))
            .when_some(file, |body, file| body.child(
                crate::ui::elements::button::Button::new("viewer-open-file")
                    .label(cx.i18n().text(Message::Open))
                    .icon("icons/external-link.svg")
                    .on_click(move |_, cx| {
                        let _ = owner.update(cx, |owner, cx| {
                            let mut pane_id = None;
                            owner.state.layout.visit_panes(&mut |pane| {
                                if pane.active_tab().id == source_tab { pane_id = Some(pane.id.clone()); }
                            });
                            if let Some(pane_id) = pane_id {
                                owner.open_file_item(&pane_id, file.path.clone(), file.is_dir, cx);
                            }
                        });
                    })
            ))
    }

    pub(super) fn render_content(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let theme = *cx.theme();
        let handle = cx.entity().downgrade();
        match &mut self.content {
            Content::Pdf(pdf) => pdf.update_visible(),
            Content::Text(_) => self.sync_text_layout(window, cx),
            _ => {}
        }
        match &self.content {
            Content::Audio(_) => self.render_audio(window, cx),
            Content::None if self.preparation.is_some() => {
                message(cx.i18n().text(Message::ViewerLoading), cx)
            }
            Content::None if self.file.as_ref().is_none() => {
                message(cx.i18n().text(Message::ViewerEmpty), cx)
            }
            Content::None | Content::Unavailable => {
                self.render_unavailable_file(cx)
            }
            Content::Loading => message(cx.i18n().text(Message::ViewerLoading), cx),
            // GPUI's img imposes the image ratio on its layout box, so the height
            // would follow the width. Paint into the available bounds instead.
            Content::Image(image) => self.render_interactive_image(image.clone(), window.is_fullscreen(), cx),
            Content::Preview(preview) if preview.read(cx).file_unavailable() => self.render_unavailable_file(cx),
            Content::Preview(preview) => preview.clone().into_any_element(),
            Content::Markdown(_) => self.render_markdown(cx),
            Content::Text(text) => {
                use super::text::{ROW_HEIGHT, TEXT_PADDING, TEXT_SIZE};
                let rows = text.rows();
                let highlighting = self.syntax_highlighting
                    && !self.file.as_ref().is_some_and(|file| super::markdown::is_markdown(&file.path));
                let gutter = text.gutter_width();
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
                        .child(div().w(gutter).flex_none().pr(rems(1.0)).text_right()
                            .text_color(cx.theme().text_muted).child(rows.line_number(index)))
                        .child(if highlighting {
                            rows.styled_row(index, cx.theme())
                        } else {
                            StyledText::new(rows.row(index).unwrap_or_default())
                        })
                        .into_any_element()
                })
                .size_full()
                .py(rems(1.0));
                div()
                    .size_full()
                    .bg(theme.sidebar)
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
                .py(rems(1.0))
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
                .py(rems(1.0))
                .into_any_element()
            }
        }
    }
}
