//! Filesystem-free Office text fallback; native workers receive a read-only object.
use crate::models::CommandError;
use std::fs::File;

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
    let mut preview = String::new();
    let mut data = Vec::new();
    for target in targets {
        if let Ok(mut content) = archive.by_name(target) {
            use std::io::Read;
            let mut buf = vec![0u8; 8192];
            if let Ok(n) = content.read(&mut buf) {
                data = buf[..n].to_vec();
                break;
            }
        }
    }
    if !data.is_empty() {
        let text = String::from_utf8_lossy(&data);
        let mut in_tag = false;
        let mut tag_content = String::new();
        for c in text.chars() {
            if c == '<' {
                in_tag = true;
                tag_content.clear();
            } else if c == '>' {
                in_tag = false;
                if tag_content == "/t" || tag_content == "t" {
                    if !preview.ends_with(' ') && !preview.ends_with('\n') {
                        preview.push(' ');
                    }
                } else if (tag_content.starts_with("w:p")
                    || tag_content == "/w:p"
                    || tag_content == "text:p")
                    && !preview.ends_with('\n')
                {
                    preview.push('\n');
                }
            } else if in_tag {
                if tag_content.len() < 10 {
                    tag_content.push(c);
                }
            } else {
                preview.push(c);
            }
            if preview.len() > 1000 {
                break;
            }
        }
    }
    Ok(preview.trim().to_string())
}

/// Complete Viewer document; see `office_document` for its line format.
pub fn document(file: File, extension: &str) -> Result<String, CommandError> {
    super::office_document::document(file, extension)
}
