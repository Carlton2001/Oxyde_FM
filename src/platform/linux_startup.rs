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

/// A named location: a local path, absolute or from `current`, or a URI for GIO.
enum Named {
    Path(PathBuf),
    Uri(String),
}

fn named(argument: &str, current: &Path) -> Option<Named> {
    if argument.starts_with('-') {
        return None;
    }
    Some(match argument.split_once("://") {
        Some((scheme, rest)) if scheme.eq_ignore_ascii_case("file") => {
            let path = rest.strip_prefix("localhost").unwrap_or(rest);
            if !path.starts_with('/') {
                return None;
            }
            Named::Path(PathBuf::from(percent_decode_str(path).decode_utf8().ok()?.into_owned()))
        }
        Some((scheme, _)) if is_scheme(scheme) => Named::Uri(argument.to_owned()),
        _ => Named::Path(current.join(argument)),
    })
}

/// Without `..`, the path as named; otherwise resolved, as `..` after a link would not
/// lead where it reads.
fn plain(path: &Path) -> Option<String> {
    let path = if path.components().any(|part| part == Component::ParentDir) {
        std::fs::canonicalize(path).ok()?
    } else {
        path.components().collect()
    };
    path.into_os_string().into_string().ok()
}

fn location(argument: &str, current: &Path) -> Option<String> {
    let path = match named(argument, current)? {
        Named::Uri(uri) => return Some(uri),
        Named::Path(path) => path,
    };
    // Before the first window: nothing is on screen yet that a slow mount could freeze.
    let metadata = std::fs::metadata(&path).ok()?;
    plain(if metadata.is_dir() { path.as_path() } else { path.parent()? })
}

/// Folders to show, as FileManager1's `ShowFolders` and a second launch name them.
pub(super) fn folders(uris: &[String]) -> Vec<String> {
    uris.iter().filter_map(|uri| location(uri, Path::new("/"))).collect()
}

/// Existing local items to select in their folder, as FileManager1's `ShowItems` and
/// `ShowItemProperties` name them. Other schemes have no folder Oxyde can list here.
pub(super) fn items(uris: &[String]) -> Vec<String> {
    uris.iter().filter_map(|uri| match named(uri, Path::new("/"))? {
        Named::Path(path) if path.is_absolute() && std::fs::symlink_metadata(&path).is_ok() => plain(&path),
        _ => None,
    }).collect()
}

fn is_scheme(scheme: &str) -> bool {
    scheme.starts_with(|first: char| first.is_ascii_alphabetic())
        && scheme.chars().all(|part| part.is_ascii_alphanumeric() || matches!(part, '+' | '-' | '.'))
}

#[cfg(test)]
#[path = "linux_startup/tests/locations.rs"]
mod tests;
