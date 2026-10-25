//! Complete-or-error Office search text, sharing the Viewer's XML text decoding.
//! Windows calls this only inside the bounded, zero-capability Office worker.
use super::{attribute, push_text, tags};
use std::{
    collections::HashSet,
    fs::File,
    io::{self, Read},
};

pub(crate) const MAX_TEXT: usize = 8 * 1024 * 1024;
pub(crate) const MAX_SOURCE: u64 = 128 * 1024 * 1024;
const MAX_PART: usize = 16 * 1024 * 1024;
const MAX_XML: usize = 64 * 1024 * 1024;
const MAX_ENTRIES: usize = 10_000;
const MAX_NAMES: usize = 1024 * 1024;
const MAX_STRINGS: usize = 1_000_000;

fn limit() -> io::Error {
    io::Error::new(
        io::ErrorKind::FileTooLarge,
        "Office content search limit reached",
    )
}
fn invalid() -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, "Incomplete Office document")
}

pub(crate) fn extract(
    file: File,
    extension: &str,
    checkpoint: &dyn Fn() -> io::Result<()>,
) -> io::Result<String> {
    checkpoint()?;
    if file.metadata()?.len() > MAX_SOURCE {
        return Err(limit());
    }
    let mut archive = zip::ZipArchive::new(file).map_err(io::Error::other)?;
    if archive.len() > MAX_ENTRIES {
        return Err(limit());
    }
    let extension = extension.to_ascii_lowercase();
    let (word, sheet, slides, odf) = match extension.as_str() {
        "docx" | "docm" | "dotx" | "dotm" => (true, false, false, false),
        "xlsx" | "xlsm" | "xltx" | "xltm" => (false, true, false, false),
        "pptx" | "pptm" | "potx" | "potm" | "ppsx" | "ppsm" => (false, false, true, false),
        "odt" | "ods" | "odp" | "ott" | "ots" | "otp" => (false, false, false, true),
        _ => return Err(invalid()),
    };
    let mut names = HashSet::new();
    let mut parts = Vec::new();
    let mut name_bytes = 0usize;
    let mut shared = false;
    for index in 0..archive.len() {
        checkpoint()?;
        let part = archive.by_index_raw(index).map_err(io::Error::other)?;
        let name = part.name().map_err(io::Error::other)?.into_owned();
        name_bytes = name_bytes
            .checked_add(name.len())
            .filter(|&size| size <= MAX_NAMES)
            .ok_or_else(limit)?;
        if !names.insert(name.clone()) {
            return Err(invalid());
        }
        if sheet && name == "xl/sharedStrings.xml" {
            shared = true;
        }
        if (word && name == "word/document.xml")
            || (sheet && name.starts_with("xl/worksheets/") && name.ends_with(".xml"))
            || (slides && name.starts_with("ppt/slides/") && name.ends_with(".xml"))
            || (odf && name == "content.xml")
        {
            parts.push(name);
        }
    }
    if parts.is_empty() {
        return Err(invalid());
    }
    let mut remaining = MAX_XML;
    let strings = if shared {
        shared_strings(
            &part(
                &mut archive,
                "xl/sharedStrings.xml",
                &mut remaining,
                checkpoint,
            )?,
            checkpoint,
        )?
    } else {
        Vec::new()
    };
    let mut output = Text::default();
    for name in parts {
        checkpoint()?;
        let xml = part(&mut archive, &name, &mut remaining, checkpoint)?;
        if sheet {
            worksheet(&xml, &strings, &mut output, checkpoint)?;
        } else {
            paragraphs(&xml, word, odf, &mut output, checkpoint)?;
        }
    }
    checkpoint()?;
    Ok(output.output)
}

fn part(
    archive: &mut zip::ZipArchive<File>,
    name: &str,
    remaining: &mut usize,
    checkpoint: &dyn Fn() -> io::Result<()>,
) -> io::Result<String> {
    checkpoint()?;
    let mut part = archive.by_name(name).map_err(io::Error::other)?;
    let bound = MAX_PART.min(*remaining);
    if part.size() > bound as u64 {
        return Err(limit());
    }
    let mut bytes = Vec::new();
    let mut block = [0; 64 * 1024];
    loop {
        checkpoint()?;
        let read = part.read(&mut block)?;
        if read == 0 {
            break;
        }
        if read > bound - bytes.len() {
            return Err(limit());
        }
        bytes.extend_from_slice(&block[..read]);
    }
    *remaining -= bytes.len();
    String::from_utf8(bytes).map_err(io::Error::other)
}

fn visit(
    xml: &str,
    checkpoint: &dyn Fn() -> io::Result<()>,
    mut visitor: impl FnMut(&str, &str, &str, bool) -> io::Result<()>,
) -> io::Result<()> {
    let mut result = Ok(());
    tags(xml, |text, name, tag, closing| {
        result = checkpoint().and_then(|()| visitor(text, name, tag, closing));
        result.is_ok()
    });
    result
}

#[derive(Default)]
struct Text {
    output: String,
    line: String,
}
impl Text {
    fn append(&mut self, text: &str) -> io::Result<()> {
        if text.len() > MAX_TEXT.saturating_sub(self.output.len() + self.line.len()) {
            return Err(limit());
        }
        push_text(&mut self.line, text);
        Ok(())
    }
    fn end_line(&mut self) -> io::Result<()> {
        if self.line.len() + 1 > MAX_TEXT.saturating_sub(self.output.len()) {
            return Err(limit());
        }
        self.output.push_str(&self.line);
        self.output.push('\n');
        self.line.clear();
        Ok(())
    }
}

fn paragraphs(
    xml: &str,
    word: bool,
    odf: bool,
    output: &mut Text,
    checkpoint: &dyn Fn() -> io::Result<()>,
) -> io::Result<()> {
    let mut in_text = false;
    let mut notes = false;
    visit(xml, checkpoint, |text, name, tag, closing| {
        if in_text && !notes {
            output.append(text)?;
        }
        if name == "presentation:notes" {
            notes = !closing;
        }
        if odf {
            if matches!(name, "text:p" | "text:h") && !notes {
                in_text = !closing && !tag.ends_with('/');
                if closing {
                    output.end_line()?;
                }
            }
            if matches!(name, "text:tab" | "text:s" | "text:line-break") && !closing && !notes {
                output.append(" ")?;
            }
        } else {
            let text_tag = if word { "w:t" } else { "a:t" };
            let paragraph = if word { "w:p" } else { "a:p" };
            if name == text_tag {
                in_text = !closing && !tag.ends_with('/');
            }
            if name == paragraph && closing {
                output.end_line()?;
            }
            if matches!(name, "w:tab" | "w:br" | "w:cr" | "a:br") && !closing {
                output.append(" ")?;
            }
        }
        Ok(())
    })?;
    if !output.line.is_empty() {
        output.end_line()?;
    }
    Ok(())
}

fn shared_strings(xml: &str, checkpoint: &dyn Fn() -> io::Result<()>) -> io::Result<Vec<String>> {
    let mut strings = Vec::new();
    let mut current = Text::default();
    let mut bytes = 0usize;
    let (mut in_text, mut phonetic) = (false, false);
    visit(xml, checkpoint, |text, name, tag, closing| {
        if in_text && !phonetic {
            current.append(text)?;
        }
        match (name, closing) {
            ("t", false) => in_text = !tag.ends_with('/'),
            ("t", true) => in_text = false,
            ("rPh", _) => phonetic = !closing && !tag.ends_with('/'),
            ("si", true) => {
                if strings.len() >= MAX_STRINGS {
                    return Err(limit());
                }
                bytes += current.line.len();
                if bytes > MAX_PART {
                    return Err(limit());
                }
                strings.push(std::mem::take(&mut current.line));
            }
            ("si", false) if tag.ends_with('/') => {
                if strings.len() >= MAX_STRINGS {
                    return Err(limit());
                }
                strings.push(String::new());
            }
            _ => {}
        }
        Ok(())
    })?;
    Ok(strings)
}

fn worksheet(
    xml: &str,
    strings: &[String],
    output: &mut Text,
    checkpoint: &dyn Fn() -> io::Result<()>,
) -> io::Result<()> {
    let (mut value, mut kind) = (String::new(), String::new());
    let mut in_value = false;
    visit(xml, checkpoint, |text, name, tag, closing| {
        if in_value {
            if text.len() > MAX_TEXT.saturating_sub(value.len()) {
                return Err(limit());
            }
            push_text(&mut value, text);
        }
        match (name, closing) {
            ("c", false) => {
                value.clear();
                kind = attribute(tag, "t").unwrap_or_default().to_owned();
            }
            ("v" | "t", false) => in_value = !tag.ends_with('/'),
            ("v" | "t", true) => in_value = false,
            ("c", true) => {
                let shown = match kind.as_str() {
                    "s" => strings
                        .get(value.trim().parse::<usize>().map_err(|_| invalid())?)
                        .ok_or_else(invalid)?
                        .as_str(),
                    "b" => {
                        if value.trim() == "1" {
                            "TRUE"
                        } else {
                            "FALSE"
                        }
                    }
                    _ => &value,
                };
                // The cell text has already had entities decoded above.
                if shown.len() > MAX_TEXT.saturating_sub(output.output.len()) {
                    return Err(limit());
                }
                output.line.push_str(shown);
                output.end_line()?;
            }
            _ => {}
        }
        Ok(())
    })
}

#[cfg(test)]
#[path = "search/tests/extraction.rs"]
mod tests;
