use pulldown_cmark::{CodeBlockKind, Event, Options, Parser, Tag, TagEnd};
use std::{
    ops::Range,
    path::Path,
    time::{Duration, Instant},
};

#[derive(Clone)]
pub(super) enum Kind {
    Paragraph,
    Heading(u8),
    Code,
    Quote,
    Rule,
    TableRow(bool),
    Image(Option<String>),
}

#[derive(Clone, Default)]
pub(super) struct Span {
    pub(super) range: Range<usize>,
    pub(super) bold: bool,
    pub(super) italic: bool,
    pub(super) code: bool,
    pub(super) strike: bool,
    pub(super) link: bool,
    pub(super) syntax: u8,
}

pub(super) struct Block {
    pub(super) kind: Kind,
    pub(super) text: String,
    pub(super) spans: Vec<Span>,
}

fn local_image(base: &str, destination: &str) -> Option<String> {
    let destination = destination.split(['?', '#']).next()?;
    let decoded = percent_encoding::percent_decode_str(destination)
        .decode_utf8()
        .ok()?;
    let path = Path::new(decoded.as_ref());
    // Relative file references only: never treat URLs or UNC paths as images.
    if decoded.is_empty()
        || decoded.contains(':')
        || path.has_root()
        || decoded.starts_with(['/', '\\'])
    {
        return None;
    }
    let full = Path::new(base).parent()?.join(path);
    Some(full.to_string_lossy().into_owned())
}

struct Builder<'a> {
    publish: &'a mut dyn FnMut(Vec<Block>) -> bool,
    cancelled: &'a dyn Fn() -> bool,
    stopped: bool,
    first: bool,
    blocks: Vec<Block>,
    kind: Kind,
    text: String,
    spans: Vec<Span>,
    bold: usize,
    italic: usize,
    strike: usize,
    links: usize,
    /// Open inline HTML code elements (`<code>`, `<kbd>`…).
    code: usize,
    /// Inside an HTML block that started with `<pre>`.
    html_code: bool,
    lists: Vec<Option<u64>>,
    quote_depth: usize,
    language: String,
    started: Instant,
}

impl Builder<'_> {
    fn flush(&mut self) {
        if self.stopped || (self.cancelled)() {
            return;
        }
        if self.text.is_empty() && !matches!(self.kind, Kind::Image(_) | Kind::Rule) {
            return;
        }
        let text = std::mem::take(&mut self.text);
        let mut spans = std::mem::take(&mut self.spans);
        if matches!(self.kind, Kind::Code) && self.started.elapsed() < Duration::from_millis(200) {
            spans = super::super::text::code_highlights(&self.language, &text)
                .into_iter()
                .map(|(range, syntax)| Span {
                    range,
                    syntax,
                    ..Default::default()
                })
                .collect();
        }
        self.blocks.push(Block {
            kind: self.kind.clone(),
            text,
            spans,
        });
        if self.first || self.blocks.len() >= 32 {
            self.first = false;
            self.stopped = !(self.publish)(std::mem::take(&mut self.blocks));
        }
        self.kind = if self.quote_depth > 0 {
            Kind::Quote
        } else {
            Kind::Paragraph
        };
    }

    /// A code block's closing line break is not an empty last line.
    fn end_code(&mut self) {
        let kept = self.text.trim_end_matches('\n').len();
        self.text.truncate(kept);
        self.spans.retain(|span| span.range.start < kept);
        for span in &mut self.spans {
            span.range.end = span.range.end.min(kept);
        }
        self.flush();
    }

    fn append(&mut self, value: &str, code: bool) {
        let mut remaining = value;
        while !remaining.is_empty() && !self.stopped && !(self.cancelled)() {
            let mut end = remaining.len().min(16 * 1024 - self.text.len());
            while !remaining.is_char_boundary(end) {
                end -= 1;
            }
            if end == 0 {
                // Split the native layout without ending its Markdown context.
                let kind = self.kind.clone();
                self.flush();
                self.kind = kind;
                continue;
            }
            let start = self.text.len();
            self.text.push_str(&remaining[..end]);
            self.spans.push(Span {
                range: start..self.text.len(),
                bold: self.bold > 0,
                italic: self.italic > 0,
                strike: self.strike > 0,
                link: self.links > 0,
                code: code || self.code > 0,
                ..Default::default()
            });
            remaining = &remaining[end..];
        }
    }
}

/// Keep one parser alive so references, lists and fences span batch boundaries.
pub(super) fn parse_progressively(
    source: &str,
    path: &str,
    cancelled: &dyn Fn() -> bool,
    publish: &mut dyn FnMut(Vec<Block>) -> bool,
) {
    if cancelled() {
        return;
    }
    let mut builder = Builder {
        publish,
        cancelled,
        stopped: false,
        first: true,
        blocks: Vec::new(),
        kind: Kind::Paragraph,
        text: String::new(),
        spans: Vec::new(),
        bold: 0,
        italic: 0,
        strike: 0,
        links: 0,
        code: 0,
        html_code: false,
        lists: Vec::new(),
        quote_depth: 0,
        language: String::new(),
        started: Instant::now(),
    };
    let options =
        Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS;
    let mut image = None::<(Option<String>, String)>;
    for event in Parser::new_ext(source, options) {
        if builder.stopped || cancelled() {
            return;
        }
        if let Some((_, alt)) = &mut image {
            match event {
                Event::End(TagEnd::Image) => {
                    let (path, alt) = image.take().unwrap();
                    builder.kind = Kind::Image(path);
                    builder.append(&alt, false);
                    builder.flush();
                }
                Event::Text(value) | Event::Code(value) => alt.push_str(&value),
                Event::SoftBreak | Event::HardBreak => alt.push(' '),
                _ => {}
            }
            continue;
        }
        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                builder.flush();
                builder.kind = Kind::Heading(level as u8);
            }
            Event::Start(Tag::Paragraph) => {}
            Event::Start(Tag::CodeBlock(kind)) => {
                builder.flush();
                builder.kind = Kind::Code;
                builder.language = match kind {
                    CodeBlockKind::Fenced(info) => {
                        info.split_whitespace().next().unwrap_or("").to_owned()
                    }
                    _ => String::new(),
                };
            }
            Event::Start(Tag::Image { dest_url, .. }) => {
                builder.flush();
                image = Some((local_image(path, &dest_url), String::new()));
            }
            Event::Start(Tag::Strong) => builder.bold += 1,
            Event::End(TagEnd::Strong) => builder.bold = builder.bold.saturating_sub(1),
            Event::Start(Tag::Emphasis) => builder.italic += 1,
            Event::End(TagEnd::Emphasis) => builder.italic = builder.italic.saturating_sub(1),
            Event::Start(Tag::Strikethrough) => builder.strike += 1,
            Event::End(TagEnd::Strikethrough) => builder.strike = builder.strike.saturating_sub(1),
            Event::Start(Tag::Link { .. }) => builder.links += 1,
            Event::End(TagEnd::Link) => builder.links = builder.links.saturating_sub(1),
            Event::Start(Tag::List(start)) => {
                builder.flush();
                builder.lists.push(start);
            }
            Event::End(TagEnd::List(_)) => {
                builder.flush();
                builder.lists.pop();
            }
            Event::Start(Tag::Item) => {
                builder.flush();
                let indent = "  ".repeat(builder.lists.len().saturating_sub(1).min(32));
                let marker = match builder.lists.last_mut() {
                    Some(Some(number)) => {
                        let marker = format!("{number}. ");
                        *number = number.saturating_add(1);
                        marker
                    }
                    _ => "• ".to_owned(),
                };
                builder.append(&format!("{indent}{marker}"), false);
            }
            Event::Start(Tag::BlockQuote(_)) => {
                builder.flush();
                builder.quote_depth += 1;
                builder.kind = Kind::Quote;
            }
            Event::End(TagEnd::BlockQuote(_)) => {
                builder.flush();
                builder.quote_depth = builder.quote_depth.saturating_sub(1);
                builder.kind = Kind::Paragraph;
            }
            Event::Start(Tag::TableHead) => {
                builder.flush();
                builder.kind = Kind::TableRow(true);
                builder.bold += 1;
            }
            Event::Start(Tag::TableRow) => {
                builder.flush();
                builder.kind = Kind::TableRow(false);
            }
            Event::End(TagEnd::TableHead) => {
                builder.flush();
                builder.bold = builder.bold.saturating_sub(1);
            }
            Event::End(TagEnd::TableCell) => builder.append("\t", false),
            Event::End(TagEnd::CodeBlock) => builder.end_code(),
            Event::End(
                TagEnd::Paragraph
                | TagEnd::Heading(_)
                | TagEnd::Item
                | TagEnd::TableRow,
            ) => builder.flush(),
            Event::Text(value) => builder.append(&value.replace('\t', "    "), false),
            Event::Code(value) => builder.append(&value, true),
            Event::SoftBreak => builder.append(
                if matches!(builder.kind, Kind::Code) {
                    "\n"
                } else {
                    " "
                },
                false,
            ),
            Event::HardBreak => builder.append("\n", false),
            Event::TaskListMarker(checked) => {
                builder.append(if checked { "☑ " } else { "☐ " }, false)
            }
            Event::Rule => {
                builder.flush();
                builder.kind = Kind::Rule;
                builder.flush();
            }
            // Code elements are rendered as code; other raw HTML is displayed
            // literally. Nothing is ever interpreted or executed.
            Event::InlineHtml(value) => match html_code_tag(&value) {
                Some(true) => builder.code += 1,
                Some(false) => builder.code = builder.code.saturating_sub(1),
                None => builder.append(&value, true),
            },
            Event::Html(value) => {
                if let Some(text) = html_code_block(&value, builder.html_code) {
                    if !builder.html_code {
                        builder.html_code = true;
                        builder.flush();
                        builder.kind = Kind::Code;
                        builder.language.clear();
                    }
                    builder.append(&text, false);
                } else {
                    builder.append(&value, true);
                }
            }
            Event::End(TagEnd::HtmlBlock) if builder.html_code => {
                builder.html_code = false;
                builder.end_code();
            }
            _ => {}
        }
    }
    builder.flush();
    if !builder.stopped && !cancelled() && !builder.blocks.is_empty() {
        (builder.publish)(builder.blocks);
    }
}

const CODE_TAGS: [&str; 5] = ["code", "kbd", "samp", "tt", "var"];

/// `Some(true)` opens and `Some(false)` closes an inline code element.
fn html_code_tag(value: &str) -> Option<bool> {
    let inner = value.trim().strip_prefix('<')?.strip_suffix('>')?;
    let (closing, inner) = match inner.strip_prefix('/') {
        Some(rest) => (true, rest),
        None => (false, inner),
    };
    let name = inner.split(|c: char| c.is_whitespace() || c == '/').next()?;
    (CODE_TAGS.iter().any(|tag| name.eq_ignore_ascii_case(tag)) && !inner.ends_with('/'))
        .then_some(!closing)
}

/// A line of an HTML block opened by `<pre>`, tags removed
/// and the common entities decoded; `None` for any other HTML block.
fn html_code_block(value: &str, continued: bool) -> Option<String> {
    if !continued && !value.trim_start().to_ascii_lowercase().starts_with("<pre") {
        return None;
    }
    let mut text = String::with_capacity(value.len());
    let mut rest = value;
    while let Some(start) = rest.find('<') {
        text.push_str(&rest[..start]);
        let tail = &rest[start..];
        let end = tail.find('>').map(|end| end + 1).unwrap_or(tail.len());
        let tag = tail[1..end.saturating_sub(1).max(1)].trim_start_matches('/');
        let name = tag.split(|c: char| c.is_whitespace() || c == '/').next().unwrap_or("");
        if !(name.eq_ignore_ascii_case("pre") || CODE_TAGS.iter().any(|code| name.eq_ignore_ascii_case(code))) {
            text.push_str(&tail[..end]);
        }
        rest = &tail[end..];
    }
    text.push_str(rest);
    Some(
        text.replace("&lt;", "<")
            .replace("&gt;", ">")
            .replace("&quot;", "\"")
            .replace("&#39;", "'")
            .replace("&amp;", "&"),
    )
}

#[cfg(test)]
fn parse(source: &str, path: &str) -> (Vec<Block>, bool) {
    let mut blocks = Vec::new();
    parse_progressively(source, path, &|| false, &mut |batch| {
        blocks.extend(batch);
        true
    });
    (blocks, false)
}

#[cfg(test)]
#[path = "tests/parse.rs"]
mod tests;
