//! Markdown parsing is bounded and performed off the UI thread. Images use
//! the existing isolated preview decoder and its shared memory budget.
mod loading;
mod parse;
mod selection;
use super::{content::Content, Viewer};
use crate::ui::{
    components::tooltip::{NativeTooltipExt, Tooltip},
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
    selection: Option<selection::Selection>,
    pub(super) selecting: bool,
    /// 1 drags by characters, 2 by words, 3 by blocks.
    granularity: usize,
    /// The code block just copied, shown with a check until the task clears it.
    copied: Option<(usize, Task<()>)>,
    /// Header row of the table under the pointer, which shows its copy button.
    hovered_table: Option<usize>,
    /// The unit pressed first, which a drag always keeps selected.
    origin: (selection::Position, selection::Position),
}

enum ImageState {
    Pending { _task: Task<()> },
    Ready(Arc<RenderImage>),
    Failed,
}

impl Markdown {
    pub(super) fn block_count(&self) -> usize {
        self.blocks.borrow().len()
    }

    pub(super) fn block_len(&self, index: usize) -> usize {
        self.blocks.borrow().get(index).map_or(0, |block| block.text.len())
    }

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
    /// The copy button shows a check for a moment.
    fn show_code_copied(&mut self, index: usize, cx: &mut Context<Self>) {
        let Content::Markdown(document) = &mut self.content else { return };
        let task = cx.spawn(async move |this, cx| {
            cx.background_executor().timer(std::time::Duration::from_millis(1500)).await;
            let _ = this.update(cx, |this, cx| {
                if let Content::Markdown(document) = &mut this.content {
                    if document.copied.as_ref().is_some_and(|(copied, _)| *copied == index) {
                        document.copied = None;
                        cx.notify();
                    }
                }
            });
        });
        document.copied = Some((index, task));
        cx.notify();
    }

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
        let selection = document.selection;
        let copied = document.copied.as_ref().map(|(index, _)| *index);
        let hovered_table = document.hovered_table;
        let body = list(document.list.clone(), move |index, _, cx| {
            let blocks = blocks.borrow();
            let Some(block) = blocks.get(index) else {
                return div().into_any_element();
            };
            let theme = *cx.theme();
            if let Kind::TableRow(header) = block.kind {
                // The table starts at the nearest header at or above this row.
                let table = (0..=index).rev().find(|row| matches!(blocks[*row].kind, Kind::TableRow(true))).unwrap_or(index);
                let hover = handle.clone();
                let button = header.then(|| {
                    // Tab-separated rows paste as a table in spreadsheets.
                    let source = blocks[index..]
                        .iter()
                        .enumerate()
                        .take_while(|(offset, row)| matches!(row.kind, Kind::TableRow(false)) || *offset == 0)
                        .map(|(_, row)| row.text.trim_end_matches('\t'))
                        .collect::<Vec<_>>()
                        .join("\n");
                    let done = copied == Some(index);
                    // Same inset as on code blocks, from the table's own edges.
                    copy_button(index, source, done, &handle, cx)
                        .right(rems(2.375))
                        .when(!done && hovered_table != Some(index), |button| button.opacity(0.0))
                });
                return div()
                    .id(("viewer-markdown-table-row", index))
                    .relative()
                    .on_hover(move |hovered, _, cx| {
                        let _ = hover.update(cx, |this, cx| {
                            let Content::Markdown(document) = &mut this.content else { return };
                            let next = if *hovered { Some(table) } else { document.hovered_table.filter(|current| *current != table) };
                            if document.hovered_table != next {
                                document.hovered_table = next;
                                cx.notify();
                            }
                        });
                    })
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
                    // Painted after the next rows, which it may overlap slightly.
                    .children(button.map(|button| deferred(button).with_priority(1)))
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
                Kind::Code => frame.py(rems(0.5)),
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
                .map(|span| {
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
                    (
                        span.range.clone(),
                        HighlightStyle {
                            color,
                            font_weight: span.bold.then_some(FontWeight::BOLD),
                            font_style: span.italic.then_some(FontStyle::Italic),
                            // A tint of the text color stands out on any theme's background.
                            background_color: span.code.then_some(theme.text.opacity(0.12)),
                            strikethrough: span.strike.then_some(StrikethroughStyle {
                                thickness: px(1.0),
                                color: None,
                            }),
                            ..Default::default()
                        },
                    )
                })
                .collect::<Vec<_>>();
            let highlights = selection::with_selection(
                highlights,
                selection.and_then(|selection| selection.within(index, block.text.len())),
                theme.accent.opacity(0.35),
            );
            // Inline code reads in the monospace face, like fenced blocks.
            let mut monospace: Vec<(std::ops::Range<usize>, SharedString)> = Vec::new();
            if !matches!(block.kind, Kind::Code) {
                for span in block.spans.iter().filter(|span| span.code) {
                    match monospace.last_mut() {
                        Some((range, _)) if range.end == span.range.start => range.end = span.range.end,
                        _ => monospace.push((span.range.clone(), "JetBrains Mono".into())),
                    }
                }
            }
            let text = StyledText::new(block.text.clone())
                .with_highlights(highlights)
                .with_font_family_overrides(monospace);
            // Painted rows only receive the pointer, so their layout is measured.
            let (press, drag) = (handle.clone(), handle.clone());
            let (press_layout, drag_layout) = (text.layout().clone(), text.layout().clone());
            let frame = frame
                .id(("viewer-markdown-block", index))
                .cursor(CursorStyle::IBeam)
                .on_mouse_down(MouseButton::Left, move |event: &MouseDownEvent, _, cx| {
                    let offset = press_layout.index_for_position(event.position).unwrap_or_else(|offset| offset);
                    let _ = press.update(cx, |this, cx| this.markdown_pointer(index, offset, true, event.modifiers.shift, event.click_count, cx));
                })
                .on_mouse_move(move |event: &MouseMoveEvent, _, cx| {
                    if event.pressed_button == Some(MouseButton::Left) {
                        let offset = drag_layout.index_for_position(event.position).unwrap_or_else(|offset| offset);
                        let _ = drag.update(cx, |this, cx| this.markdown_pointer(index, offset, false, false, 1, cx));
                    }
                });
            if matches!(block.kind, Kind::Code) {
                // A fenced block reads as its own framed, monospace panel.
                let source = block.text.clone();
                let done = copied == Some(index);
                let group: SharedString = format!("viewer-markdown-code-{index}").into();
                let button = copy_button(index, source, done, &handle, cx)
                    .when(!done, |button| button.opacity(0.0).group_hover(group.clone(), |style| style.opacity(1.0)));
                return frame
                    .child(
                        div()
                            .group(group)
                            .relative()
                            .w_full()
                            .px(rems(0.875))
                            .py(rems(0.625))
                            .rounded(rems(0.375))
                            .border(rems(0.0625))
                            .border_color(theme.border)
                            .bg(theme.text.opacity(0.07))
                            .font_family("JetBrains Mono")
                            .text_size(rems(0.8125))
                            .child(text)
                            .child(button),
                    )
                    .into_any_element();
            }
            frame
                .child(text)
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
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .on_mouse_up(MouseButton::Left, cx.listener(|this, _, _, _| this.markdown_pointer_released()))
                    .on_mouse_up_out(MouseButton::Left, cx.listener(|this, _, _, _| this.markdown_pointer_released()))
                    .child(self.selection_drag_listener(cx))
                    .child(body),
            )
            .into_any_element()
    }
}/// The copy button at a code block's or table's top right; a check replaces
/// its icon for a moment after a copy.
fn copy_button(index: usize, source: String, done: bool, handle: &WeakEntity<Viewer>, cx: &App) -> Stateful<Div> {
    let theme = *cx.theme();
    let copy = handle.clone();
    div()
        .id(("viewer-markdown-copy", index))
        .absolute()
        .top(rems(0.375))
        .right(rems(0.375))
        .size(rems(1.75))
        // The block shows a text cursor; its button keeps the arrow.
        .cursor(CursorStyle::Arrow)
        .flex()
        .items_center()
        .justify_center()
        .rounded(rems(0.375))
        .border(rems(0.0625))
        .border_color(theme.border)
        .bg(theme.surface)
        .hover(move |style| style.bg(theme.control_hover))
        .fixed_tooltip(Tooltip::text(cx.i18n().text(Message::Copy)))
        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
            // Never starts a text selection under the button.
            cx.stop_propagation();
            cx.write_to_clipboard(ClipboardItem::new_string(source.clone()));
            let _ = copy.update(cx, |this, cx| this.show_code_copied(index, cx));
        })
        .child(
            svg()
                .path(if done { "icons/check.svg" } else { "icons/copy.svg" })
                .size(rems(0.875))
                .text_color(if done { theme.success_text } else { theme.text }),
        )
}


