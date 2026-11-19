//! IME composition: the provisional text an input method (Japanese, Chinese, Korean) shows,
//! underlined, until it commits. The composition lives in the field's text so it lays out
//! and scrolls like the rest; each update replaces it, and the commit replaces it once more
//! with the final text.
use super::TextInput;
use gpui::*;
use std::ops::Range;

/// A stretch of the shown line with one look: selected, composed, both or neither.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct Span {
    pub(super) len: usize,
    pub(super) selected: bool,
    pub(super) composing: bool,
}

/// Cuts a line of `len` bytes where the selection or the composition starts or ends.
pub(super) fn spans(len: usize, selected: &Range<usize>, marked: Option<&Range<usize>>) -> Vec<Span> {
    let mut cuts = vec![0, len, selected.start.min(len), selected.end.min(len)];
    if let Some(marked) = marked {
        cuts.extend([marked.start.min(len), marked.end.min(len)]);
    }
    cuts.sort_unstable();
    cuts.dedup();
    let within = |range: &Range<usize>, start: usize, end: usize| range.start <= start && end <= range.end;
    cuts.windows(2)
        .map(|cut| Span {
            len: cut[1] - cut[0],
            selected: within(selected, cut[0], cut[1]),
            composing: marked.is_some_and(|marked| within(marked, cut[0], cut[1])),
        })
        .collect()
}

/// Where a composition stands once the bytes of `edited` give way to `inserted` bytes, or
/// `None` when the edit reaches into it.
pub(super) fn shift_marked(marked: &Range<usize>, edited: &Range<usize>, inserted: usize) -> Option<Range<usize>> {
    if edited.end <= marked.start {
        let start = marked.start - edited.len() + inserted;
        Some(start..start + marked.len())
    } else if edited.start >= marked.end {
        Some(marked.clone())
    } else {
        None
    }
}

/// The byte offset of `units` UTF-16 units into `text`, kept on a character boundary.
fn byte_for_utf16(text: &str, units: usize) -> usize {
    let mut seen = 0;
    for (byte, character) in text.char_indices() {
        if seen >= units {
            return byte;
        }
        seen += character.len_utf16();
    }
    text.len()
}

/// Shows `text` as the composition in place of `target`, else the current composition, else
/// the selection. Returns the new selection: `caret`, given in UTF-16 units within `text` as
/// input methods do, or the end of the composition.
pub(super) fn compose(
    content: &mut String,
    marked: &mut Option<Range<usize>>,
    selected: Range<usize>,
    target: Option<Range<usize>>,
    text: &str,
    caret: Option<Range<usize>>,
) -> Range<usize> {
    let range = target.or_else(|| marked.clone()).unwrap_or(selected);
    content.replace_range(range.clone(), text);
    let end = range.start + text.len();
    *marked = (!text.is_empty()).then_some(range.start..end);
    caret
        .map(|caret| {
            range.start + byte_for_utf16(text, caret.start.min(caret.end))
                ..range.start + byte_for_utf16(text, caret.start.max(caret.end))
        })
        .unwrap_or(end..end)
}

/// Puts final `text` in place of `target`, else the composition, else the selection, and
/// ends the composition. Returns the new selection, or `None` when nothing changes: an
/// empty text with nothing composed is a composition already gone (the Japanese IME sends
/// one), which must not erase the selection.
pub(super) fn commit(
    content: &mut String,
    marked: &mut Option<Range<usize>>,
    selected: Range<usize>,
    target: Option<Range<usize>>,
    text: &str,
) -> Option<Range<usize>> {
    if target.is_none() && text.is_empty() && marked.is_none() {
        return None;
    }
    let range = target.or_else(|| marked.take()).unwrap_or(selected);
    content.replace_range(range.clone(), text);
    *marked = None;
    let caret = range.start + text.len();
    Some(caret..caret)
}

impl TextInput {
    /// Replaces `range` with `text` from the field itself (keys, clipboard): a composition
    /// elsewhere follows the text around it, one the edit reaches into ends.
    pub(super) fn splice(&mut self, range: Range<usize>, text: &str) {
        self.marked_range = self.marked_range.take().and_then(|marked| shift_marked(&marked, &range, text.len()));
        self.content.replace_range(range, text);
    }

    fn apply_ime_selection(&mut self, selected: Range<usize>, window: &mut Window, cx: &mut Context<Self>) {
        self.selection_anchor = selected.start;
        self.selected_range = selected;
        self.selection_reversed = false;
        self.is_selecting = false;
        (self.on_change)(&self.content, window, cx);
        // Completions wait for the commit: their menu would cover the candidate window.
        self.refresh_completions(cx);
        self.reset_blinking(cx);
    }

    pub(super) fn ime_compose(
        &mut self,
        range_utf16: Option<Range<usize>>,
        text: &str,
        caret_utf16: Option<Range<usize>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let target = range_utf16.map(|range| self.range_from_utf16(range));
        let selected = compose(&mut self.content, &mut self.marked_range, self.selected_range.clone(), target, text, caret_utf16);
        self.apply_ime_selection(selected, window, cx);
    }

    pub(super) fn ime_commit(
        &mut self,
        range_utf16: Option<Range<usize>>,
        text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let target = range_utf16.map(|range| self.range_from_utf16(range));
        if let Some(selected) = commit(&mut self.content, &mut self.marked_range, self.selected_range.clone(), target, text) {
            self.apply_ime_selection(selected, window, cx);
        }
    }

    /// The input method keeps the composed text as it stands.
    pub(super) fn ime_unmark(&mut self, cx: &mut Context<Self>) {
        if self.marked_range.take().is_some() {
            self.refresh_completions(cx);
            cx.notify();
        }
    }
}

#[cfg(test)]
#[path = "tests/composition.rs"]
mod tests;
