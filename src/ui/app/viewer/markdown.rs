//! Markdown parsing is bounded and performed off the UI thread. Images use
//! the existing isolated preview decoder and its shared memory budget.
mod loading;
mod parse;
use super::{content::Content, Viewer};
use crate::ui::{
    i18n::{I18nContext, Message},
    theme::ThemeContext,
};
use gpui::{prelude::*, *};
use parse::{Block, Kind};
use std::{cell::RefCell, collections::HashMap, rc::Rc, sync::Arc};

pub(super) struct Markdown {
    blocks: Rc<RefCell<Vec<Block>>>,
    pub(super) list: ListState,
    images: HashMap<usize, ImageState>,
    truncated: bool,
}

enum ImageState {
    Pending { _task: Task<()> },
    Ready(Arc<RenderImage>),
    Failed,
}

impl Markdown {
    pub(super) fn images(&self) -> Vec<Arc<RenderImage>> {
        self.images
            .values()
            .filter_map(|state| match state {
                ImageState::Ready(image) => Some(image.clone()),
                _ => None,
            })
            .collect()
    }
}

pub(super) fn is_markdown(path: &str) -> bool {
    std::path::Path::new(path)
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| {
            ["md", "markdown", "mdown", "mkd", "mkdn"]
                .iter()
                .any(|name| ext.eq_ignore_ascii_case(name))
        })
}

impl Viewer {
    fn request_markdown_image(&mut self, index: usize, path: String, cx: &mut Context<Self>) {
        let Content::Markdown(document) = &mut self.content else {
            return;
        };
        if document.images.contains_key(&index) || document.images.len() >= 32 {
            return;
        }
        let renderer = cx.svg_renderer();
        let generation = self.generation;
        let wanted = self.wanted.clone();
        let work = crate::ui::preview_jobs::load(move || {
            if wanted.load(std::sync::atomic::Ordering::Acquire) != generation {
                return Err("Markdown image no longer shown".to_string());
            }
            crate::ui::image_preview::load_path(std::path::Path::new(&path), &renderer)
                .map(|preview| {
                    crate::ui::memory_diagnostics::track(&preview.image, crate::ui::memory_diagnostics::State::Decoded);
                    preview.image
                })
                .map_err(|error| error.to_string())
        });
        let task = cx.spawn(async move |this, cx| {
            let result = work.await;
            let _ = this.update(cx, |this, cx| {
                if this.generation != generation {
                    return;
                }
                let Content::Markdown(document) = &mut this.content else {
                    return;
                };
                document.images.insert(
                    index,
                    match result {
                        Ok(image) => {
                            crate::ui::memory_diagnostics::track(&image, crate::ui::memory_diagnostics::State::Shown);
                            ImageState::Ready(image)
                        },
                        Err(error) => {
                            log::debug!("Markdown image unavailable: {error}");
                            ImageState::Failed
                        }
                    },
                );
                // Re-measure only this block, preserving the logical scroll anchor.
                document.list.remeasure_items(index..index + 1);
                cx.notify();
            });
        });
        document
            .images
            .insert(index, ImageState::Pending { _task: task });
    }

    pub(super) fn render_markdown(&self, cx: &mut Context<Self>) -> AnyElement {
        let Content::Markdown(document) = &self.content else {
            return div().into_any_element();
        };
        let blocks = document.blocks.clone();
        let handle = cx.entity().downgrade();
        let body = list(document.list.clone(), move |index, _, cx| {
            let blocks = blocks.borrow();
            let Some(block) = blocks.get(index) else {
                return div().into_any_element();
            };
            let theme = *cx.theme();
            if let Kind::TableRow(header) = block.kind {
                return div()
                    .w_full()
                    .px(rems(2.0))
                    .flex()
                    .children(block.text.trim_end_matches('\t').split('\t').map(|cell| {
                        div()
                            .flex_1()
                            .min_w_0()
                            .px(rems(0.5))
                            .py(rems(0.375))
                            .border_1()
                            .border_color(theme.border)
                            .when(header, |cell| {
                                cell.bg(theme.sidebar).font_weight(FontWeight::SEMIBOLD)
                            })
                            .child(cell.to_owned())
                    }))
                    .into_any_element();
            }
            if let Kind::Image(path) = &block.kind {
                let image = handle
                    .update(cx, |this, _| match &this.content {
                        Content::Markdown(document) => match document.images.get(&index) {
                            Some(ImageState::Ready(image)) => Some(image.clone()),
                            _ => None,
                        },
                        _ => None,
                    })
                    .ok()
                    .flatten();
                return div()
                    .w_full()
                    .px(rems(2.0))
                    .py(rems(0.5))
                    .relative()
                    .when_some(path.clone(), |frame, path| {
                        let handle = handle.clone();
                        frame.child(
                            canvas(
                                |_, _, _| (),
                                move |_, _, _, cx| {
                                    // Measuring the document must not decode offscreen images.
                                    // Only painted list rows request their image.
                                    let _ = handle.update(cx, |this, cx| {
                                        this.request_markdown_image(index, path.clone(), cx);
                                    });
                                },
                            )
                            .absolute()
                            .size_0(),
                        )
                    })
                    .map(|frame| {
                        if let Some(image) = image {
                            let size = image.size(0);
                            frame.child(
                                div()
                                    .w_full()
                                    .max_w(px(size.width.0 as f32))
                                    .aspect_ratio(
                                        size.width.0 as f32 / (size.height.0 as f32).max(1.0),
                                    )
                                    .child(img(image).size_full().object_fit(ObjectFit::Contain)),
                            )
                        } else {
                            frame
                                .text_color(theme.text_muted)
                                .child(if block.text.is_empty() {
                                    cx.i18n().text(Message::PreviewUnavailable)
                                } else {
                                    block.text.clone().into()
                                })
                        }
                    })
                    .into_any_element();
            }
            let mut frame = div()
                .w_full()
                .px(rems(2.0))
                .py(rems(0.375))
                .text_color(theme.text);
            frame = match block.kind {
                Kind::Heading(level) => frame
                    .pt(rems(0.75))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_size(rems(match level {
                        1 => 1.75,
                        2 => 1.375,
                        3 => 1.125,
                        _ => 1.0,
                    })),
                Kind::Code => frame
                    .font_family("JetBrains Mono")
                    .text_size(rems(0.8125))
                    .bg(theme.sidebar),
                Kind::Quote => frame
                    .border_l(rems(0.1875))
                    .border_color(theme.border)
                    .text_color(theme.text_muted),
                Kind::Rule => {
                    return frame
                        .child(div().h(rems(0.0625)).bg(theme.border))
                        .into_any_element()
                }
                _ => frame,
            };
            let highlights = block
                .spans
                .iter()
                .filter_map(|span| {
                    let color = match span.syntax {
                        1 => Some(theme.text_muted),
                        2 => Some(theme.success_text),
                        3 => Some(theme.warning_text),
                        4 => Some(theme.accent),
                        5 => Some(theme.file_icons.code),
                        6 => Some(theme.error),
                        7 => Some(theme.accent),
                        _ if span.link => Some(theme.accent),
                        _ => None,
                    };
                    Some((
                        span.range.clone(),
                        HighlightStyle {
                            color,
                            font_weight: span.bold.then_some(FontWeight::BOLD),
                            font_style: span.italic.then_some(FontStyle::Italic),
                            background_color: span.code.then_some(theme.sidebar),
                            strikethrough: span.strike.then_some(StrikethroughStyle {
                                thickness: px(1.0),
                                color: None,
                            }),
                            ..Default::default()
                        },
                    ))
                })
                .collect::<Vec<_>>();
            frame
                .child(StyledText::new(block.text.clone()).with_highlights(highlights))
                .into_any_element()
        })
        .size_full()
        .py(rems(1.0));
        div()
            .size_full()
            .flex()
            .flex_col()
            .when(document.truncated, |frame| {
                frame.child(
                    div()
                        .px(rems(2.0))
                        .text_color(cx.theme().text_muted)
                        .child(cx.i18n().text(Message::ViewerTextTruncated)),
                )
            })
            .child(div().flex_1().min_h_0().child(body))
            .into_any_element()
    }
}
