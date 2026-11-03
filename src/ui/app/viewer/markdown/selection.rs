//! Selecting and copying rendered Markdown. Positions are (block, byte) pairs,
//! hit-tested on each block's laid-out text, so selections span blocks.
use super::{parse::Kind, Markdown};
use crate::ui::app::viewer::{content::Content, Viewer};
use gpui::*;
use std::ops::Range;

pub(super) type Position = (usize, usize);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct Selection {
    pub anchor: Position,
    pub head: Position,
}

impl Selection {
    fn ordered(&self) -> (Position, Position) {
        (self.anchor.min(self.head), self.anchor.max(self.head))
    }

    /// The selected bytes of block `index`, whose text is `length` bytes long.
    pub(super) fn within(&self, index: usize, length: usize) -> Option<Range<usize>> {
        let (start, end) = self.ordered();
        if index < start.0 || index > end.0 {
            return None;
        }
        let from = if index == start.0 { start.1.min(length) } else { 0 };
        let to = if index == end.0 { end.1.min(length) } else { length };
        (from < to).then_some(from..to)
    }
}

/// Adds the selection's background to `highlights`, splitting the styles it crosses.
/// Highlights are sorted and do not overlap, like the parser's spans.
pub(super) fn with_selection(
    highlights: Vec<(Range<usize>, HighlightStyle)>,
    selected: Option<Range<usize>>,
    background: Hsla,
) -> Vec<(Range<usize>, HighlightStyle)> {
    let Some(selected) = selected else { return highlights };
    let mark = |style: HighlightStyle| HighlightStyle { background_color: Some(background), ..style };
    let mut result = Vec::with_capacity(highlights.len() + 3);
    let mut cursor = selected.start;
    for (range, style) in highlights {
        // A selected gap before this span.
        if cursor < selected.end && cursor < range.start {
            let end = range.start.min(selected.end);
            result.push((cursor..end, mark(HighlightStyle::default())));
            cursor = end;
        }
        let inner = range.start.max(selected.start)..range.end.min(selected.end);
        if inner.is_empty() {
            result.push((range, style));
            continue;
        }
        if range.start < inner.start {
            result.push((range.start..inner.start, style));
        }
        result.push((inner.clone(), mark(style)));
        if inner.end < range.end {
            result.push((inner.end..range.end, style));
        }
        cursor = cursor.max(inner.end);
    }
    if cursor < selected.end {
        result.push((cursor..selected.end, mark(HighlightStyle::default())));
    }
    result
}

impl Markdown {
    fn selected_text(&self) -> Option<String> {
        let selection = self.selection?;
        let blocks = self.blocks.borrow();
        let parts: Vec<&str> = blocks
            .iter()
            .enumerate()
            .filter(|(_, block)| !matches!(block.kind, Kind::Rule | Kind::Image(_)))
            .filter_map(|(index, block)| Some(&block.text[selection.within(index, block.text.len())?]))
            .collect();
        (!parts.is_empty()).then(|| parts.join("\n"))
    }
}

impl Viewer {
    /// A press (`start`) or a drag at `offset` in block `index`.
    /// `clicks` is 2 for a word, 3 for the whole block.
    pub(in crate::ui::app::viewer) fn markdown_pointer(&mut self, index: usize, offset: usize, start: bool, extend: bool, clicks: usize, cx: &mut Context<Self>) {
        let Content::Markdown(document) = &mut self.content else { return };
        if !start && !document.selecting {
            return;
        }
        if start {
            document.granularity = if extend { 1 } else { clicks.clamp(1, 3) };
        }
        // A double or triple press then drags by words or blocks, as in editors.
        let range = {
            let blocks = document.blocks.borrow();
            let text = blocks.get(index).map_or("", |block| block.text.as_str());
            match document.granularity {
                2 => crate::ui::app::viewer::text::word_at(text, offset),
                3 => 0..text.len(),
                _ => offset..offset,
            }
        };
        let (from, to) = ((index, range.start), (index, range.end));
        let selection = match (start, document.selection) {
            (true, Some(current)) if extend => Selection { head: (index, offset), ..current },
            (true, _) => {
                document.origin = (from, to);
                Selection { anchor: from, head: to }
            }
            (false, _) if from < document.origin.0 => Selection { anchor: document.origin.1, head: from },
            (false, _) => Selection { anchor: document.origin.0, head: to.max(document.origin.1) },
        };
        document.selecting |= start;
        if document.selection != Some(selection) {
            document.selection = Some(selection);
            cx.notify();
        }
    }

    pub(in crate::ui::app::viewer) fn markdown_pointer_released(&mut self) {
        if let Content::Markdown(document) = &mut self.content {
            document.selecting = false;
        }
    }

    /// Ctrl+A and Ctrl+C on rendered Markdown; `false` when not handled.
    pub(in crate::ui::app::viewer) fn markdown_shortcut(&mut self, key: &str, cx: &mut Context<Self>) -> bool {
        let Content::Markdown(document) = &mut self.content else { return false };
        match key {
            "a" => {
                let end = document.blocks.borrow().last().map_or((0, 0), |block| {
                    (document.blocks.borrow().len() - 1, block.text.len())
                });
                document.selection = Some(Selection { anchor: (0, 0), head: end });
                cx.notify();
                true
            }
            "c" => {
                if let Some(text) = document.selected_text() {
                    cx.write_to_clipboard(ClipboardItem::new_string(text));
                }
                true
            }
            _ => false,
        }
    }
}

#[cfg(test)]
#[path = "tests/selection.rs"]
mod tests;
