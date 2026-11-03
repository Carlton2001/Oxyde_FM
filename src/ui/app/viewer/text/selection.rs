//! Selecting and copying a text file's content. Rows are monospace, so a pointer
//! maps to a row by its index and to a character by its column.
use super::{columns_of, Rows, TextFile};
use crate::ui::app::viewer::{content::Content, Viewer};
use gpui::*;

/// Byte offsets into the text: where the selection started and where it ends now.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(in crate::ui::app::viewer) struct Selection {
    pub anchor: usize,
    pub head: usize,
}

impl Selection {
    pub(in crate::ui::app::viewer) fn range(&self) -> std::ops::Range<usize> {
        self.anchor.min(self.head)..self.anchor.max(self.head)
    }
}

/// The word around `offset` (letters, digits and `_`), else the single character there.
pub(in crate::ui::app::viewer) fn word_at(text: &str, offset: usize) -> std::ops::Range<usize> {
    let mut offset = offset.min(text.len());
    while !text.is_char_boundary(offset) {
        offset -= 1;
    }
    let word = |c: char| c.is_alphanumeric() || c == '_';
    let after = text[offset..].chars().next();
    let before = text[..offset].chars().next_back();
    // Pressing just past a word's end still selects that word.
    let inside = after.is_some_and(word) || before.is_some_and(word);
    if !inside {
        return offset..after.map_or(offset, |c| offset + c.len_utf8());
    }
    let start = text[..offset].char_indices().rev().take_while(|(_, c)| word(*c)).last().map_or(offset, |(index, _)| index);
    let end = text[offset..].char_indices().find(|(_, c)| !word(*c)).map_or(text.len(), |(index, _)| offset + index);
    start..end
}

impl Rows {
    fn bounds(&self, index: usize) -> Option<(usize, usize)> {
        let start = *self.rows.get(index)? as usize;
        let end = self.rows.get(index + 1).map_or(self.text.len(), |end| *end as usize);
        Some((start, start + self.text[start..end].trim_end_matches(['\n', '\r']).len()))
    }

    /// The byte offset nearest to `column` on displayed row `index`.
    pub(in crate::ui::app::viewer) fn offset_at(&self, index: usize, column: f32) -> usize {
        let index = index.min(self.rows.len().saturating_sub(1));
        let Some((start, end)) = self.bounds(index) else { return 0 };
        let mut used = self.indent(index) as f32;
        for (offset, c) in self.text[start..end].char_indices() {
            let width = columns_of(c) as f32;
            if column < used + width / 2.0 {
                return start + offset;
            }
            used += width;
        }
        end
    }

    /// Display columns of `range` on row `index`, and whether it runs past the row's end.
    pub(in crate::ui::app::viewer) fn selected_columns(&self, index: usize, range: &std::ops::Range<usize>) -> Option<(usize, usize, bool)> {
        let (start, end) = self.bounds(index)?;
        let next = self.rows.get(index + 1).map_or(self.text.len(), |next| *next as usize);
        if range.end <= start || range.start >= next.max(end + 1) || range.is_empty() {
            return None;
        }
        let indent = self.indent(index);
        let column = |offset: usize| indent + self.text[start..offset.clamp(start, end)].chars().map(columns_of).sum::<usize>();
        Some((column(range.start), column(range.end), range.end > end))
    }
}

impl TextFile {
    pub(in crate::ui::app::viewer) fn selected_text(&self) -> Option<&str> {
        let range = self.selection?.range();
        (!range.is_empty()).then(|| &self.text[range])
    }
}

impl Viewer {
    /// Starts (or with Shift extends) a selection at a press on row `index`.
    /// `clicks` is 2 for a word, 3 for the whole line.
    pub(in crate::ui::app::viewer) fn text_pointer(&mut self, index: usize, position: Point<Pixels>, start: bool, extend: bool, clicks: usize, window: &Window, cx: &mut Context<Self>) {
        let Content::Text(text) = &mut self.content else { return };
        if !start && !text.selecting {
            return;
        }
        let rem = window.rem_size();
        let left = text.list.viewport_bounds().origin.x
            + (super::TEXT_PADDING + text.gutter_width()).to_pixels(rem);
        let column = ((position.x - left) / text.advance.max(px(1.0))).max(0.0);
        let offset = text.rows().offset_at(index, column);
        if start {
            text.granularity = if extend { 1 } else { clicks.clamp(1, 3) };
        }
        // A double or triple press then drags by words or lines, as in editors.
        let unit = |offset: usize| match text.granularity {
            2 => word_at(&text.text, offset),
            3 => {
                let line = text.lines.partition_point(|line| *line as usize <= offset).saturating_sub(1);
                let from = text.lines[line] as usize;
                let to = text.lines.get(line + 1).map_or(text.text.len(), |next| *next as usize);
                from..from + text.text[from..to].trim_end_matches(['\n', '\r']).len()
            }
            _ => offset..offset,
        };
        let range = unit(offset);
        let selection = match (start, text.selection) {
            (true, Some(current)) if extend => Selection { head: offset, ..current },
            (true, _) => {
                text.origin = range.clone();
                Selection { anchor: range.start, head: range.end }
            }
            (false, _) if range.start < text.origin.start => Selection { anchor: text.origin.end, head: range.start },
            (false, _) => Selection { anchor: text.origin.start, head: range.end.max(text.origin.end) },
        };
        text.selecting |= start;
        if text.selection != Some(selection) {
            text.selection = Some(selection);
            cx.notify();
        }
    }

    pub(in crate::ui::app::viewer) fn text_pointer_released(&mut self) {
        if let Content::Text(text) = &mut self.content {
            text.selecting = false;
        }
    }

    /// Ctrl+A and Ctrl+C on a text file; `false` when not handled.
    pub(in crate::ui::app::viewer) fn text_shortcut(&mut self, key: &str, cx: &mut Context<Self>) -> bool {
        let Content::Text(text) = &mut self.content else { return false };
        match key {
            "a" => {
                text.selection = Some(Selection { anchor: 0, head: text.text.len() });
                cx.notify();
                true
            }
            "c" => {
                if let Some(selected) = text.selected_text() {
                    cx.write_to_clipboard(ClipboardItem::new_string(selected.to_owned()));
                }
                true
            }
            _ => false,
        }
    }
}

#[cfg(test)]
#[path = "../tests/text_selection.rs"]
mod tests;
