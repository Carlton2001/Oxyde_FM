//! Locations named on the command line, which is how a desktop's "Open With" reaches a
//! file manager. As Nautilus does, a file stands for its folder; other schemes than
//! `file` stay URIs for GIO to connect. Names that are not UTF-8 cannot be told from
//! their lookalikes here and are left out, as a location that does not exist.
use percent_encoding::percent_decode_str;
use std::ffi::OsString;
use std::path::{Component, Path, PathBuf};

pub(super) fn locations(arguments: impl Iterator<Item = OsString>, current: &Path) -> Vec<String> {
    arguments.filter_map(|argument| {
        let found = argument.to_str().and_then(|argument| location(argument, current));
        if found.is_none() {
            log::warn!("Ignoring startup location {argument:?}");
        }
        found
    }).collect()
}

fn location(argument: &str, current: &Path) -> Option<String> {
    if argument.starts_with('-') {
        return None;
    }
    let path = match argument.split_once("://") {
        Some((scheme, rest)) if scheme.eq_ignore_ascii_case("file") => {
            let path = rest.strip_prefix("localhost").unwrap_or(rest);
            if !path.starts_with('/') {
                return None;
            }
            PathBuf::from(percent_decode_str(path).decode_utf8().ok()?.into_owned())
        }
        Some((scheme, _)) if is_scheme(scheme) => return Some(argument.to_owned()),
        _ => current.join(argument),
    };
    // Before the first window: nothing is on screen yet that a slow mount could freeze.
    let metadata = std::fs::metadata(&path).ok()?;
    let folder = if metadata.is_dir() { path.as_path() } else { path.parent()? };
    let folder = if folder.components().any(|part| part == Component::ParentDir) {
        std::fs::canonicalize(folder).ok()?
    } else {
        folder.components().collect()
    };
    folder.into_os_string().into_string().ok()
}

fn is_scheme(scheme: &str) -> bool {
    scheme.starts_with(|first: char| first.is_ascii_alphabetic())
        && scheme.chars().all(|part| part.is_ascii_alphanumeric() || matches!(part, '+' | '-' | '.'))
}

#[cfg(test)]
#[path = "linux_startup/tests/locations.rs"]
mod tests;
