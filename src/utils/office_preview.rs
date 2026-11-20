//! Filesystem-free Office text fallback; native workers receive a read-only object.
use crate::models::CommandError;
use std::fs::File;

/// XML read for a tooltip's first thousand characters.
const PREVIEW_XML_BYTES: u64 = 64 * 1024;

pub fn text(file: File, extension: &str) -> Result<String, CommandError> {
    let mut archive = zip::ZipArchive::new(file).map_err(|e| CommandError::Other(e.to_string()))?;
    let ext = extension.to_lowercase();
    let targets = match ext.as_str() {
        "docx" | "docm" => vec!["word/document.xml"],
        "xlsx" | "xlsm" => vec!["xl/sharedStrings.xml"],
        "pptx" | "pptm" => vec!["ppt/slides/slide1.xml", "ppt/slides/slide2.xml"],
        "odt" | "ods" | "odp" => vec!["content.xml"],
        _ => vec!["word/document.xml", "xl/sharedStrings.xml", "content.xml"],
    };
    let mut data = Vec::new();
    for target in targets {
        if let Ok(content) = archive.by_name(target) {
            use std::io::Read;
            // Read on until enough XML for the preview: one read may return only a few bytes.
            if content.take(PREVIEW_XML_BYTES).read_to_end(&mut data).is_ok() && !data.is_empty() {
                break;
            }
            data.clear();
        }
    }
    let xml = String::from_utf8_lossy(&data);
    let mut preview = String::new();
    let mut rest = xml.as_ref();
    while let Some(start) = rest.find('<') {
        super::office_document::push_text(&mut preview, &rest[..start]);
        let Some(length) = rest[start..].find('>') else { break };
        let tag = &rest[start + 1..start + length];
        rest = &rest[start + length + 1..];
        // The element's whole name: `w:pPr` or `w:pStyle` are no paragraph.
        let name = tag.trim_end_matches('/').split_whitespace().next().unwrap_or("");
        let separator = match name {
            "t" | "/t" => ' ',
            "w:p" | "/w:p" | "a:p" | "/a:p" | "text:p" | "/text:p" | "text:h" | "/text:h" => '\n',
            _ => continue,
        };
        if !preview.ends_with([' ', '\n']) {
            preview.push(separator);
        }
        if preview.len() > 1000 {
            break;
        }
    }
    Ok(preview.trim().to_string())
}

/// Complete Viewer document; see `office_document` for its line format.
pub fn document(file: File, extension: &str) -> Result<String, CommandError> {
    super::office_document::document(file, extension)
}

#[cfg(test)]
#[path = "office_preview/tests/text.rs"]
mod tests;
