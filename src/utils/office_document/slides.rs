//! PresentationML: the text of every slide, in presentation order.
use super::{attribute, entry, push_text, tags, Archive, Builder};
use crate::models::CommandError;

/// Slide parts in presentation order, falling back to numbered part names.
fn slide_paths(archive: &mut Archive) -> Vec<String> {
    let presentation = entry(archive, "ppt/presentation.xml").unwrap_or_default();
    let relations = entry(archive, "ppt/_rels/presentation.xml.rels").unwrap_or_default();
    let mut targets = std::collections::HashMap::new();
    tags(&relations, |_, name, tag, closing| {
        if name == "Relationship" && !closing {
            if let (Some(id), Some(target)) = (attribute(tag, "Id"), attribute(tag, "Target")) {
                let path = match target.strip_prefix('/') {
                    Some(absolute) => absolute.to_owned(),
                    None => format!("ppt/{target}"),
                };
                targets.insert(id.to_owned(), path);
            }
        }
        true
    });
    let mut paths = Vec::new();
    tags(&presentation, |_, name, tag, closing| {
        if name == "p:sldId" && !closing {
            if let Some(path) = attribute(tag, "r:id").and_then(|id| targets.get(id)) {
                paths.push(path.clone());
            }
        }
        true
    });
    if paths.is_empty() {
        let mut numbered: Vec<(u32, String)> = archive
            .file_names()
            .filter_map(|name| {
                let name = name.ok()?;
                let number = name.as_ref().strip_prefix("ppt/slides/slide")?.strip_suffix(".xml")?;
                Some((number.parse().ok()?, name.to_string()))
            })
            .collect();
        numbered.sort();
        paths = numbered.into_iter().map(|(_, name)| name).collect();
    }
    paths
}

pub(super) fn pptx(archive: &mut Archive) -> Result<String, CommandError> {
    let paths = slide_paths(archive);
    if paths.is_empty() {
        return Err(CommandError::Other("Presentation has no slides".into()));
    }
    let mut builder = Builder { tag: "p", ..Default::default() };
    for (index, path) in paths.iter().enumerate() {
        if builder.full() {
            break;
        }
        builder.line("slide", &(index + 1).to_string());
        let Ok(xml) = entry(archive, path) else { continue };
        tags(&xml, |text, name, tag, closing| {
            if builder.in_text {
                push_text(&mut builder.paragraph, text);
            }
            match (name, closing) {
                ("a:t", false) => builder.in_text = !tag.ends_with('/'),
                ("a:t", true) => builder.in_text = false,
                ("a:br", false) => builder.paragraph.push(' '),
                ("a:tbl", false) => builder.table_depth += 1,
                ("a:tbl", true) => builder.table_depth = builder.table_depth.saturating_sub(1),
                ("a:tr", false) if builder.table_depth == 1 => builder.cells.clear(),
                ("a:tr", true) if builder.table_depth == 1 => builder.end_row(),
                ("a:tc", false) if builder.table_depth == 1 => builder.cell(),
                ("a:p", true) => {
                    if builder.paragraph.trim().is_empty() && builder.table_depth == 0 {
                        builder.paragraph.clear();
                    } else {
                        builder.end_paragraph();
                    }
                }
                _ => {}
            }
            !builder.full()
        });
    }
    Ok(builder.output)
}
