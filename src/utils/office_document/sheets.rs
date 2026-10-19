//! SpreadsheetML: every worksheet in workbook order, with shared strings.
//! Values are shown as stored; number formats and formulas are not evaluated.
use super::{attribute, entry, push_text, tags, Archive, Builder, MAX_CELLS};
use crate::models::CommandError;

/// Shared-string table entries kept; later references stay empty.
const MAX_SHARED_STRINGS: usize = 1_000_000;

fn shared_strings(archive: &mut Archive) -> Vec<String> {
    let Ok(xml) = entry(archive, "xl/sharedStrings.xml") else { return Vec::new() };
    let mut strings = Vec::new();
    let mut current = String::new();
    let (mut in_text, mut phonetic) = (false, false);
    tags(&xml, |text, name, tag, closing| {
        if in_text && !phonetic {
            push_text(&mut current, text);
        }
        match (name, closing) {
            ("t", false) => in_text = !tag.ends_with('/'),
            ("t", true) => in_text = false,
            ("rPh", false) => phonetic = true,
            ("rPh", true) => phonetic = false,
            ("si", true) => strings.push(std::mem::take(&mut current)),
            _ => {}
        }
        strings.len() < MAX_SHARED_STRINGS
    });
    strings
}

/// Worksheet names and archive paths in workbook order.
fn worksheets(archive: &mut Archive) -> Result<Vec<(String, String)>, CommandError> {
    let workbook = entry(archive, "xl/workbook.xml")?;
    let relations = entry(archive, "xl/_rels/workbook.xml.rels").unwrap_or_default();
    let mut targets = std::collections::HashMap::new();
    tags(&relations, |_, name, tag, closing| {
        if name == "Relationship" && !closing {
            if let (Some(id), Some(target)) = (attribute(tag, "Id"), attribute(tag, "Target")) {
                let path = match target.strip_prefix('/') {
                    Some(absolute) => absolute.to_owned(),
                    None => format!("xl/{target}"),
                };
                targets.insert(id.to_owned(), path);
            }
        }
        true
    });
    let mut sheets = Vec::new();
    tags(&workbook, |_, name, tag, closing| {
        if name == "sheet" && !closing {
            let mut title = String::new();
            push_text(&mut title, attribute(tag, "name").unwrap_or_default());
            if let Some(path) = attribute(tag, "r:id").and_then(|id| targets.get(id)) {
                sheets.push((title, path.clone()));
            }
        }
        true
    });
    Ok(sheets)
}

/// Zero-based column of an `A1` reference.
fn column(reference: &str) -> Option<usize> {
    let mut index = 0usize;
    let mut letters = 0;
    for c in reference.chars().take_while(char::is_ascii_alphabetic) {
        index = index.checked_mul(26)?.checked_add((c.to_ascii_uppercase() as u8 - b'A') as usize + 1)?;
        letters += 1;
    }
    (letters > 0).then(|| index - 1)
}

pub(super) fn xlsx(archive: &mut Archive) -> Result<String, CommandError> {
    let strings = shared_strings(archive);
    let mut builder = Builder { tag: "p", ..Default::default() };
    for (title, path) in worksheets(archive)? {
        if builder.full() {
            break;
        }
        builder.line("sheet", &title);
        let Ok(xml) = entry(archive, &path) else { continue };
        let mut value = String::new();
        let mut kind = String::new();
        let mut in_value = false;
        tags(&xml, |text, name, tag, closing| {
            if in_value {
                push_text(&mut value, text);
            }
            match (name, closing) {
                ("row", false) => builder.cells.clear(),
                ("row", true) => builder.end_row(),
                ("c", false) => {
                    let target = attribute(tag, "r").and_then(column).unwrap_or(builder.cells.len());
                    while builder.cells.len() <= target.min(MAX_CELLS) && builder.cells.len() < MAX_CELLS {
                        builder.cell();
                    }
                    kind = attribute(tag, "t").unwrap_or_default().to_owned();
                    value.clear();
                }
                ("v" | "t", false) => in_value = !tag.ends_with('/'),
                ("v" | "t", true) => in_value = false,
                ("c", true) => {
                    let shown = match kind.as_str() {
                        "s" => value.trim().parse::<usize>().ok().and_then(|index| strings.get(index)).cloned().unwrap_or_default(),
                        "b" => if value.trim() == "1" { "TRUE".into() } else { "FALSE".into() },
                        _ => value.clone(),
                    };
                    if let Some(cell) = builder.cells.last_mut() {
                        *cell = shown.trim().to_owned();
                    }
                }
                _ => {}
            }
            !builder.full()
        });
    }
    Ok(builder.output)
}
