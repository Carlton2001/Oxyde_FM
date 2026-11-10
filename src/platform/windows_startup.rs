//! Folders named on the command line, which is how the Shell hands a folder to the
//! program registered for its `open` verb. A file stands for its folder. Switches and
//! Shell namespace names (`::{CLSID}`, `shell:`) name no folder Oxyde lists and are
//! left out, as a location that does not exist.
use std::ffi::OsString;
use std::path::{Path, PathBuf};

pub(super) fn locations(arguments: impl Iterator<Item = OsString>, current: &Path) -> Vec<String> {
    arguments.filter_map(|argument| {
        let found = argument.to_str().and_then(|argument| location(argument, current));
        if found.is_none() {
            log::warn!("Ignoring startup location {argument:?}");
        }
        found
    }).collect()
}

/// The path an argument names, absolute or from `current`, without `.`, `..` or a
/// trailing separator.
fn named(argument: &str, current: &Path) -> Option<PathBuf> {
    // `"C:\"` reaches the program as `C:"`: the backslash escaped the closing quote.
    let argument = argument.trim_end_matches('"');
    if argument.is_empty() || argument.starts_with(['-', '/', ':']) || argument.contains("::{") {
        return None;
    }
    let scheme = argument.split_once(':').map_or("", |(scheme, _)| scheme);
    if scheme.len() > 1 && !argument.starts_with(r"\\") {
        return None;
    }
    // A bare `C:` would name the drive's current folder, not its root.
    let path = if scheme.len() == 1 && argument.len() == 2 { current.join(format!(r"{argument}\")) } else { current.join(argument) };
    let path = std::path::absolute(path).ok()?;
    Some(path.components().collect())
}

fn location(argument: &str, current: &Path) -> Option<String> {
    let path = named(argument, current)?;
    // Before the first window: nothing is on screen yet that a slow share could freeze.
    let metadata = std::fs::metadata(&path).ok()?;
    let folder = if metadata.is_dir() { path.as_path() } else { path.parent()? };
    folder.to_str().map(str::to_owned)
}

/// Existing folders among the locations a later launch sent.
pub(super) fn folders(locations: &[String]) -> Vec<String> {
    locations.iter().filter_map(|sent| location(sent, Path::new(r"C:\"))).collect()
}

#[cfg(test)]
#[path = "windows_startup/tests/locations.rs"]
mod tests;
