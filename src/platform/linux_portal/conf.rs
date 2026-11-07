//! The FileChooser line of xdg-desktop-portal's `portals.conf`. Oxyde puts its portal
//! first and keeps the others after it, so that removing its own entry gives back the
//! line as it was. A line Oxyde had to add carries a comment, and goes with it.

pub(super) const KEY: &str = "org.freedesktop.impl.portal.FileChooser";
const SECTION: &str = "preferred";
/// Above a line Oxyde added, which it removes again rather than leaving it emptied.
pub(super) const ADDED: &str = "# Added by Oxyde for its file chooser.";
/// Opens a file Oxyde wrote from the system's configuration, followed by its path.
pub(super) const CREATED: &str = "# Written by Oxyde from ";

fn section(line: &str) -> Option<&str> {
    line.trim().strip_prefix('[')?.strip_suffix(']').map(str::trim)
}

/// The value of `key` on a `key=value` line.
fn value<'a>(line: &'a str, key: &str) -> Option<&'a str> {
    let (name, value) = line.split_once('=')?;
    (name.trim() == key).then(|| value.trim())
}

fn split(value: &str) -> impl Iterator<Item = &str> {
    value.split(';').map(str::trim).filter(|portal| !portal.is_empty())
}

fn joined<'a>(portals: impl Iterator<Item = &'a str>) -> String {
    portals.map(|portal| format!("{portal};")).collect()
}

/// The `[preferred]` value of `key`.
fn preferred<'a>(text: &'a str, key: &str) -> Option<&'a str> {
    let mut current = None;
    let mut found = None;
    for line in text.lines() {
        if let Some(name) = section(line) {
            current = Some(name);
        } else if current == Some(SECTION) {
            found = value(line, key).or(found);
        }
    }
    found
}

/// Portals asked for FileChooser, in order: its own line, otherwise the default.
pub(super) fn portals(text: &str) -> Vec<String> {
    preferred(text, KEY).or_else(|| preferred(text, "default"))
        .map(|value| split(value).map(str::to_owned).collect())
        .unwrap_or_default()
}

fn finish(lines: Vec<String>) -> String {
    let mut text = lines.join("\n");
    text.push('\n');
    text
}

/// `text` with `id` first for FileChooser. Without such a line, one goes at the end of
/// `[preferred]`, with the default's portals after `id` so they remain the fallback.
pub(super) fn with_portal(text: &str, id: &str) -> String {
    let mut lines: Vec<String> = Vec::new();
    let mut current = None;
    let mut replaced = false;
    // Where the added line goes: after the last entry of `[preferred]`.
    let mut end_of_preferred = None;
    for line in text.lines() {
        if let Some(name) = section(line) {
            current = Some(name.to_owned());
            if name == SECTION {
                end_of_preferred = Some(lines.len() + 1);
            }
        } else if current.as_deref() == Some(SECTION) {
            if let (false, Some(value)) = (replaced, value(line, KEY)) {
                lines.push(format!("{KEY}={id};{}", joined(split(value).filter(|portal| *portal != id))));
                replaced = true;
                continue;
            }
            if !line.trim().is_empty() {
                end_of_preferred = Some(lines.len() + 1);
            }
        }
        lines.push(line.to_owned());
    }
    if !replaced {
        let fallback = preferred(text, "default").map(|value| joined(split(value).filter(|portal| *portal != id))).unwrap_or_default();
        let added = [ADDED.to_owned(), format!("{KEY}={id};{fallback}")];
        match end_of_preferred {
            Some(index) => {
                lines.splice(index..index, added);
            }
            None => {
                if lines.last().is_some_and(|line| !line.trim().is_empty()) {
                    lines.push(String::new());
                }
                lines.push(format!("[{SECTION}]"));
                lines.extend(added);
            }
        }
    }
    finish(lines)
}

/// `text` without `id` for FileChooser: the line Oxyde added goes with its comment, an
/// existing one keeps its other portals, or goes when none is left.
pub(super) fn without_portal(text: &str, id: &str) -> String {
    let mut lines: Vec<String> = Vec::new();
    let mut current = None;
    for line in text.lines() {
        if let Some(name) = section(line) {
            current = Some(name.to_owned());
        } else if current.as_deref() == Some(SECTION) {
            if let Some(value) = value(line, KEY) {
                let added = lines.last().is_some_and(|previous| previous == ADDED);
                if added {
                    lines.pop();
                }
                if split(value).any(|portal| portal == id) {
                    let others = joined(split(value).filter(|portal| *portal != id));
                    if !added && !others.is_empty() {
                        lines.push(format!("{KEY}={others}"));
                    }
                    continue;
                }
            }
        }
        lines.push(line.to_owned());
    }
    finish(lines)
}

/// A file Oxyde wrote from `source`'s configuration, as Oxyde writes it.
pub(super) fn created(source: &str, contents: &str) -> String {
    format!("{CREATED}{source}\n{contents}")
}

/// The configuration a file Oxyde wrote came from, and the rest of the file.
pub(super) fn creation(text: &str) -> Option<(&str, &str)> {
    let (first, rest) = text.split_once('\n')?;
    Some((first.strip_prefix(CREATED)?, rest))
}

#[cfg(test)]
#[path = "tests/conf.rs"]
mod tests;
