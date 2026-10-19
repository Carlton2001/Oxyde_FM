//! Complete, bounded Office Open XML / OpenDocument text extraction for the
//! Viewer. Output lines are `tag<TAB>text`; table rows separate cells with
//! U+001F. Tags: title, h1-h4, p, li, tr, sheet and slide.
use crate::models::CommandError;
use std::io::Read;

/// Decompressed XML read from the archive; larger documents are truncated.
const MAX_XML_BYTES: u64 = 128 * 1024 * 1024;
/// Stays below the worker's text reply limit.
const MAX_OUTPUT_BYTES: usize = crate::utils::preview_protocol::MAX_TEXT - 1024;
pub const CELL_SEPARATOR: char = '\u{1f}';
/// Wide sheets keep their first columns; repeated empty ODF cells never expand.
const MAX_CELLS: usize = 64;

mod sheets;
mod slides;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Odf {
    Text,
    Sheet,
    Slides,
}

pub fn document(file: std::fs::File, extension: &str) -> Result<String, CommandError> {
    let mut archive =
        zip::ZipArchive::new(file).map_err(|error| CommandError::Other(error.to_string()))?;
    match extension.to_ascii_lowercase().as_str() {
        "docx" | "docm" | "dotx" | "dotm" => {
            Ok(parse(&entry(&mut archive, "word/document.xml")?, None))
        }
        "xlsx" | "xlsm" | "xltx" | "xltm" => sheets::xlsx(&mut archive),
        "pptx" | "pptm" | "potx" | "potm" | "ppsx" | "ppsm" => slides::pptx(&mut archive),
        extension => {
            let kind = match extension {
                "odt" | "ott" => Odf::Text,
                "ods" | "ots" => Odf::Sheet,
                "odp" | "otp" => Odf::Slides,
                _ => return Err(CommandError::Other("Unsupported Office document".into())),
            };
            Ok(parse(&entry(&mut archive, "content.xml")?, Some(kind)))
        }
    }
}

type Archive = zip::ZipArchive<std::fs::File>;

/// One bounded, decompressed XML part.
fn entry(archive: &mut Archive, name: &str) -> Result<String, CommandError> {
    let entry = archive
        .by_name(name)
        .map_err(|error| CommandError::Other(error.to_string()))?;
    let mut xml = Vec::new();
    entry.take(MAX_XML_BYTES).read_to_end(&mut xml)?;
    Ok(String::from_utf8_lossy(&xml).into_owned())
}

/// Visits every tag with the text that precedes it; `false` stops the walk.
fn tags(xml: &str, mut visit: impl FnMut(&str, &str, &str, bool) -> bool) {
    let mut rest = xml;
    while let Some(start) = rest.find('<') {
        let text = &rest[..start];
        let Some(end) = rest[start..].find('>') else { break };
        let tag = &rest[start + 1..start + end];
        rest = &rest[start + end + 1..];
        let closing = tag.starts_with('/');
        let name = tag
            .trim_start_matches('/')
            .split(|c: char| c.is_whitespace() || c == '/')
            .next()
            .unwrap_or("");
        if !visit(text, name, tag, closing) {
            break;
        }
    }
}

#[derive(Default)]
struct Builder {
    output: String,
    paragraph: String,
    tag: &'static str,
    cells: Vec<String>,
    table_depth: usize,
    in_text: bool,
    /// Speaker notes and other non-body ODF text.
    skip: bool,
}

impl Builder {
    fn full(&self) -> bool {
        self.output.len() >= MAX_OUTPUT_BYTES
    }
    fn end_paragraph(&mut self) {
        let text = std::mem::take(&mut self.paragraph);
        let tag = std::mem::replace(&mut self.tag, "p");
        if self.table_depth > 0 {
            if let Some(cell) = self.cells.last_mut() {
                if !cell.is_empty() && !text.trim().is_empty() {
                    cell.push(' ');
                }
                cell.push_str(text.trim());
            }
            return;
        }
        self.line(tag, text.trim_end());
    }
    fn line(&mut self, tag: &str, text: &str) {
        if self.full() {
            return;
        }
        let clean: String = text
            .chars()
            .map(|c| if c == '\n' || c == '\r' || c == '\t' { ' ' } else { c })
            .collect();
        self.output.push_str(tag);
        self.output.push('\t');
        self.output.push_str(&clean);
        self.output.push('\n');
    }
    fn cell(&mut self) {
        if self.cells.len() < MAX_CELLS {
            self.cells.push(String::new());
        }
    }
    fn end_row(&mut self) {
        let mut cells = std::mem::take(&mut self.cells);
        while cells.last().is_some_and(String::is_empty) {
            cells.pop();
        }
        if cells.iter().any(|cell| !cell.is_empty()) {
            let row = cells.join(&CELL_SEPARATOR.to_string());
            if !self.full() {
                self.output.push_str("tr\t");
                self.output.push_str(&row.replace(['\n', '\r', '\t'], " "));
                self.output.push('\n');
            }
        }
    }
}

fn attribute<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let start = tag.find(&format!("{name}=\""))? + name.len() + 2;
    let end = tag[start..].find('"')?;
    Some(&tag[start..start + end])
}

fn heading(style: &str) -> Option<&'static str> {
    let lower = style.to_ascii_lowercase();
    if lower == "title" || lower == "titre" {
        return Some("title");
    }
    let digits = lower
        .strip_prefix("heading")
        .or_else(|| lower.strip_prefix("titre"))
        .or_else(|| lower.strip_prefix("berschrift"))
        .or_else(|| lower.strip_prefix("überschrift"))
        .or_else(|| lower.strip_prefix("titolo"))
        .or_else(|| lower.strip_prefix("ttulo"))
        .or_else(|| lower.strip_prefix("título"))?;
    match digits.trim() {
        "1" => Some("h1"),
        "2" => Some("h2"),
        "3" => Some("h3"),
        "4" | "5" | "6" | "7" | "8" | "9" => Some("h4"),
        _ => None,
    }
}

fn outline(level: Option<&str>) -> &'static str {
    match level {
        Some("1") => "h1",
        Some("2") => "h2",
        Some("3") => "h3",
        _ => "h4",
    }
}

fn decode_entity(entity: &str) -> Option<char> {
    match entity {
        "amp" => Some('&'),
        "lt" => Some('<'),
        "gt" => Some('>'),
        "quot" => Some('"'),
        "apos" => Some('\''),
        _ => {
            let number = entity.strip_prefix('#')?;
            let value = match number.strip_prefix('x').or_else(|| number.strip_prefix('X')) {
                Some(hex) => u32::from_str_radix(hex, 16).ok()?,
                None => number.parse().ok()?,
            };
            char::from_u32(value)
        }
    }
}

fn push_text(target: &mut String, text: &str) {
    let mut rest = text;
    while let Some(index) = rest.find('&') {
        target.push_str(&rest[..index]);
        let after = &rest[index + 1..];
        match after.find(';').filter(|end| *end <= 10) {
            Some(end) => {
                match decode_entity(&after[..end]) {
                    Some(c) => target.push(c),
                    None => target.push_str(&rest[index..index + end + 2]),
                }
                rest = &after[end + 1..];
            }
            None => {
                target.push('&');
                rest = after;
            }
        }
    }
    target.push_str(rest);
}

fn parse(xml: &str, odf: Option<Odf>) -> String {
    let mut builder = Builder { tag: "p", ..Default::default() };
    tags(xml, |text, name, tag, closing| {
        if builder.in_text && !text.is_empty() {
            push_text(&mut builder.paragraph, text);
        }
        match odf {
            Some(kind) => parse_odf(&mut builder, kind, name, tag, closing),
            None => parse_word(&mut builder, name, tag, closing),
        }
        !builder.full()
    });
    if !builder.paragraph.trim().is_empty() {
        builder.end_paragraph();
    }
    builder.output
}

fn parse_word(builder: &mut Builder, name: &str, tag: &str, closing: bool) {
    match (name, closing) {
        ("w:t", false) => builder.in_text = !tag.ends_with('/'),
        ("w:t", true) => builder.in_text = false,
        ("w:tab", false) => builder.paragraph.push(' '),
        ("w:br" | "w:cr", false) => builder.paragraph.push(' '),
        ("w:pStyle", false) => {
            if let Some(tag) = attribute(tag, "w:val").and_then(heading) {
                builder.tag = tag;
            }
        }
        ("w:outlineLvl", false) => {
            let level = attribute(tag, "w:val").and_then(|value| value.parse::<u8>().ok());
            if let Some(level) = level {
                builder.tag = outline(Some(&(level + 1).to_string()));
            }
        }
        ("w:numPr", false) if builder.tag == "p" => builder.tag = "li",
        ("w:p", true) => builder.end_paragraph(),
        ("w:tbl", false) => builder.table_depth += 1,
        ("w:tbl", true) => builder.table_depth = builder.table_depth.saturating_sub(1),
        ("w:tr", false) if builder.table_depth == 1 => builder.cells.clear(),
        ("w:tr", true) if builder.table_depth == 1 => builder.end_row(),
        ("w:tc", false) if builder.table_depth == 1 => builder.cell(),
        _ => {}
    }
}

fn parse_odf(builder: &mut Builder, kind: Odf, name: &str, tag: &str, closing: bool) {
    let self_closing = tag.ends_with('/');
    match (name, closing) {
        ("presentation:notes", false) => builder.skip = true,
        ("presentation:notes", true) => builder.skip = false,
        _ if builder.skip => {}
        ("draw:page", false) if kind == Odf::Slides => {
            let mut name = String::new();
            push_text(&mut name, attribute(tag, "draw:name").unwrap_or_default());
            builder.line("slide", &name);
        }
        ("text:h", false) => {
            builder.in_text = true;
            builder.tag = outline(attribute(tag, "text:outline-level"));
        }
        ("text:p", false) if !self_closing => {
            builder.in_text = true;
            if builder.tag != "li" {
                builder.tag = "p";
            }
        }
        ("text:h" | "text:p", true) => {
            builder.in_text = false;
            builder.end_paragraph();
        }
        ("text:p", false) => builder.line("p", ""),
        ("text:list-item", false) => builder.tag = "li",
        ("text:list-item", true) => builder.tag = "p",
        ("text:tab" | "text:s" | "text:line-break", false) => builder.paragraph.push(' '),
        ("table:table", false) => {
            if kind == Odf::Sheet && builder.table_depth == 0 {
                let mut name = String::new();
                push_text(&mut name, attribute(tag, "table:name").unwrap_or_default());
                builder.line("sheet", &name);
            }
            builder.table_depth += 1;
        }
        ("table:table", true) => builder.table_depth = builder.table_depth.saturating_sub(1),
        ("table:table-row", false) if builder.table_depth == 1 => builder.cells.clear(),
        ("table:table-row", true) if builder.table_depth == 1 => builder.end_row(),
        ("table:table-cell" | "table:covered-table-cell", false) if builder.table_depth == 1 => {
            let repeated = attribute(tag, "table:number-columns-repeated")
                .and_then(|value| value.parse::<usize>().ok())
                .unwrap_or(1)
                .clamp(1, MAX_CELLS);
            for _ in 0..repeated {
                builder.cell();
            }
        }
        _ => {}
    }
}

#[cfg(test)]
#[path = "office_document/tests/parse.rs"]
mod tests;
