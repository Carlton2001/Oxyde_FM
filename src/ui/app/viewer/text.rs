//! Whole text files, loaded in memory and wrapped at word boundaries for the
//! current width. In a monospace font every wrapped row has the same height, so
//! the row list (and the scrollbar) is exact from the first frame. Rows keep
//! byte ranges into the text and their logical line, which line numbers and
//! syntax highlighting can build on.
use super::{content::Content, Viewer};
use gpui::*;
use std::sync::Arc;
mod syntax;

pub(super) fn code_highlights(language: &str, text: &str) -> Vec<(std::ops::Range<usize>, u8)> {
    syntax::highlight(&format!("block.{language}"), text).into_iter()
        .map(|span| (span.range, span.kind)).collect()
}

/// Text kept in memory for display; a larger file shows its beginning. Below
/// 4 GiB, line and row offsets fit in `u32`, half the memory of `usize`.
pub(super) const MAX_TEXT_BYTES: u64 = 512 * 1024 * 1024;
type Offset = u32;
pub(super) const TEXT_SIZE: Rems = rems(0.8125);
pub(super) const ROW_HEIGHT: Rems = rems(1.25);
pub(super) const TEXT_PADDING: Rems = rems(1.0);
const TAB_COLUMNS: usize = 4;

pub(super) struct TextFile {
    text: Arc<String>,
    /// Byte offset where each logical line starts.
    lines: Arc<Vec<Offset>>,
    highlights: Arc<Vec<syntax::Span>>,
    /// Byte offset where each displayed row starts, for `columns`.
    rows: Arc<Vec<Offset>>,
    columns: usize,
    /// Width requested while a rewrap runs in the background.
    wrapping: Option<(usize, Task<()>)>,
    pub(super) truncated: bool,
    pub(super) list: ListState,
    row_height: Pixels,
}

/// Columns a character occupies in a monospace font (wide East Asian
/// characters and emoji take two).
fn columns_of(c: char) -> usize {
    match c {
        '\t' => TAB_COLUMNS,
        '\r' | '\n' => 0,
        '\u{1100}'..='\u{115F}'
        | '\u{2E80}'..='\u{A4CF}'
        | '\u{AC00}'..='\u{D7A3}'
        | '\u{F900}'..='\u{FAFF}'
        | '\u{FE30}'..='\u{FE4F}'
        | '\u{FF00}'..='\u{FF60}'
        | '\u{FFE0}'..='\u{FFE6}'
        | '\u{1F300}'..='\u{1FAFF}'
        | '\u{20000}'..='\u{3FFFD}' => 2,
        _ => 1,
    }
}

pub(super) fn line_starts(text: &str) -> Vec<Offset> {
    std::iter::once(0)
        .chain(text.match_indices('\n').map(|(index, _)| index + 1))
        .filter(|start| *start < text.len() || *start == 0)
        .map(|start| start as Offset)
        .collect()
}

/// Keep at least half the row for content in narrow panes or deeply indented files.
fn continuation_indent(line: &str, columns: usize) -> usize {
    let limit = columns / 2;
    let mut indent = 0;
    for c in line.chars().take_while(|c| matches!(c, ' ' | '\t')) {
        indent = (indent + columns_of(c)).min(limit);
        if indent == limit { break; }
    }
    indent
}

/// Greedy word wrap, preserving the logical line's indentation on continuations.
pub(super) fn wrap(text: &str, lines: &[Offset], columns: usize) -> Vec<Offset> {
    let columns = columns.max(1);
    let mut rows: Vec<Offset> = Vec::with_capacity(lines.len());
    for (index, &start) in lines.iter().enumerate() {
        let start = start as usize;
        let end = lines.get(index + 1).map_or(text.len(), |end| *end as usize);
        let indent = continuation_indent(&text[start..end], columns);
        let mut row_start = start;
        loop {
            rows.push(row_start as Offset);
            let available = columns - if row_start == start { 0 } else { indent };
            let mut used = 0;
            let mut breakable = None;
            let mut next = end;
            for (offset, c) in text[row_start..end].char_indices() {
                let at = row_start + offset;
                let width = columns_of(c);
                if width > 0 && used + width > available && used > 0 {
                    next = breakable.unwrap_or(at);
                    break;
                }
                used += width;
                if c == ' ' || c == '\t' {
                    breakable = Some(at + c.len_utf8());
                }
            }
            if next == end { break; }
            row_start = next;
        }
    }
    if rows.is_empty() {
        rows.push(0);
    }
    rows
}

fn read(path: &str) -> std::io::Result<(String, bool)> {
    use std::io::Read;
    let file = std::fs::File::open(path)?;
    let length = file.metadata()?.len();
    let mut bytes = Vec::new();
    file.take(MAX_TEXT_BYTES).read_to_end(&mut bytes)?;
    // Valid UTF-8 keeps its buffer; only invalid text is copied while repaired.
    let text = String::from_utf8(bytes)
        .unwrap_or_else(|error| String::from_utf8_lossy(error.as_bytes()).into_owned());
    Ok((text, length > MAX_TEXT_BYTES))
}

/// Shared, immutable view of the wrapped rows for the list's item renderer.
#[derive(Clone)]
pub(super) struct Rows {
    text: Arc<String>,
    rows: Arc<Vec<Offset>>,
    lines: Arc<Vec<Offset>>,
    highlights: Arc<Vec<syntax::Span>>,
    columns: usize,
}

impl TextFile {
    pub(super) fn has_highlighting(&self) -> bool {
        !self.highlights.is_empty()
    }

    pub(super) fn rows(&self) -> Rows {
        Rows { text: self.text.clone(), rows: self.rows.clone(), lines: self.lines.clone(), highlights: self.highlights.clone(), columns: self.columns }
    }

    pub(super) fn gutter_width(&self) -> Rems {
        rems((self.lines.len().max(1).ilog10() + 1) as f32 * 0.8125 * 0.6 + 1.0)
    }
}

impl Rows {
    fn indent(&self, index: usize) -> usize {
        let Some(&start) = self.rows.get(index) else { return 0 };
        let line = self.lines.partition_point(|line| *line <= start).saturating_sub(1);
        let origin = self.lines[line];
        if origin == start { return 0; }
        continuation_indent(&self.text[origin as usize..], self.columns)
    }

    pub(super) fn line_number(&self, index: usize) -> String {
        self.rows.get(index).and_then(|start| self.lines.binary_search(start).ok())
            .map(|line| (line + 1).to_string()).unwrap_or_default()
    }

    pub(super) fn styled_row(&self, index: usize, theme: &crate::ui::theme::OxydeTheme) -> StyledText {
        let value = self.row(index).unwrap_or_default();
        StyledText::new(value).with_highlights(self.row_highlights(index, theme))
    }

    fn row_highlights(&self, index: usize, theme: &crate::ui::theme::OxydeTheme) -> Vec<(std::ops::Range<usize>, HighlightStyle)> {
        let Some(&start) = self.rows.get(index) else { return Vec::new() };
        let start = start as usize;
        let end = self.rows.get(index + 1).map_or(self.text.len(), |end| *end as usize);
        let source = self.text[start..end].trim_end_matches(['\n', '\r']);
        let end = start + source.len();
        let first = self.highlights.partition_point(|span| span.range.end <= start);
        let indent = self.indent(index);
        self.highlights[first..].iter().take_while(|span| span.range.start < end).map(|span| {
            let range = span.range.start.max(start) - start..span.range.end.min(end) - start;
            let expanded = |offset| indent + offset + source[..offset].bytes().filter(|byte| *byte == b'\t').count() * (TAB_COLUMNS - 1);
            let color = match span.kind {
                1 => theme.text_muted,
                2 => theme.success_text,
                3 => theme.warning_text,
                4 => theme.accent,
                5 => theme.file_icons.code,
                7 => theme.accent,
                _ => theme.error,
            };
            (expanded(range.start)..expanded(range.end), HighlightStyle { color: Some(color), ..Default::default() })
        }).collect()
    }

    /// Displayed row, without its line ending; tabs become spaces.
    pub(super) fn row(&self, index: usize) -> Option<SharedString> {
        let start = *self.rows.get(index)? as usize;
        let end = self.rows.get(index + 1).map_or(self.text.len(), |end| *end as usize);
        let row = self.text[start..end].trim_end_matches(['\n', '\r']);
        Some(SharedString::from(format!("{}{}", " ".repeat(self.indent(index)), row.replace('\t', &" ".repeat(TAB_COLUMNS)))))
    }
}

impl Viewer {
    pub(super) fn load_text(&mut self, path: String, cx: &mut Context<Self>) {
        let generation = self.generation;
        let work = crate::ui::preview_jobs::load(move || {
            let (text, truncated) = read(&path)?;
            let lines = line_starts(&text);
            let highlights = syntax::highlight(&path, &text);
            Ok::<_, std::io::Error>((text, lines, highlights, truncated))
        });
        self.finish(
            generation,
            work,
            |(text, lines, highlights, truncated)| {
                // Until the first wrap, rows are the logical lines themselves;
                // the text moves into its Arc (an `Arc<str>` would copy it).
                let lines = Arc::new(lines);
                Content::Text(TextFile {
                    text: Arc::new(text),
                    list: ListState::new(lines.len(), ListAlignment::Top, px(800.0)),
                    rows: lines.clone(),
                    lines,
                    highlights: Arc::new(highlights),
                    columns: 0,
                    wrapping: None,
                    truncated,
                    row_height: px(0.0),
                })
            },
            cx,
        );
    }

    /// Rewraps in the background when the available columns change, keeping
    /// the first visible row's text in view.
    pub(super) fn sync_text_layout(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Content::Text(text) = &mut self.content else { return };
        let rem = window.rem_size();
        let height = ROW_HEIGHT.to_pixels(rem);
        if text.row_height != height {
            let top = text.list.logical_scroll_top();
            text.list.reset_with_uniform_height(text.rows.len(), height);
            text.list.scroll_to(top);
            text.row_height = height;
        }
        let width = text.list.viewport_bounds().size.width;
        if width <= px(0.0) {
            return;
        }
        let font = window.text_style().font();
        let font = Font { family: "JetBrains Mono".into(), ..font };
        let system = window.text_system();
        let advance = system
            .advance(system.resolve_font(&font), TEXT_SIZE.to_pixels(rem), 'm')
            .map(|size| size.width)
            .unwrap_or(TEXT_SIZE.to_pixels(rem) * 0.6);
        let usable = width - TEXT_PADDING.to_pixels(rem) * 2.0 - text.gutter_width().to_pixels(rem);
        let columns = ((usable / advance).floor() as usize).max(1);
        if text.columns == columns || text.wrapping.as_ref().is_some_and(|(pending, _)| *pending == columns) {
            return;
        }
        let (source, lines) = (text.text.clone(), text.lines.clone());
        let generation = self.generation;
        let work = cx.background_executor().spawn(async move { wrap(&source, &lines, columns) });
        let task = cx.spawn(async move |this, cx| {
            let rows = work.await;
            let _ = this.update(cx, |this, cx| {
                if this.generation != generation {
                    return;
                }
                let Content::Text(text) = &mut this.content else { return };
                // Keep the text at the top of the view, whatever its new row.
                let top = text.list.logical_scroll_top().item_ix;
                let anchor = text.rows.get(top).copied().unwrap_or(0);
                // Free the old rows before publishing the new ones.
                text.rows = Arc::new(Vec::new());
                let row = rows.partition_point(|start| *start <= anchor).saturating_sub(1);
                text.rows = Arc::new(rows);
                text.columns = columns;
                text.wrapping = None;
                text.list.reset_with_uniform_height(text.rows.len(), text.row_height);
                text.list.scroll_to(ListOffset { item_ix: row, offset_in_item: px(0.0) });
                cx.notify();
            });
        });
        text.wrapping = Some((columns, task));
    }
}

#[cfg(test)]
#[path = "tests/text.rs"]
mod tests;
