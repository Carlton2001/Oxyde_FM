//! Oxyde started for the portal and gone before showing a window, twice in a row, is a
//! crash at startup that would leave every application without a file chooser: the
//! portal keeps calling a backend that fails, and its own fallback never applies. The
//! next start then shows no window and hands the portal's calls to the portal after
//! Oxyde's, until a window of Oxyde's shows again, Oxyde ends by itself or it changes
//! version.
use std::path::PathBuf;

/// Failed starts after which Oxyde relays instead.
const LIMIT: u32 = 2;
const VERSION: &str = env!("CARGO_PKG_VERSION");

fn path() -> Option<PathBuf> {
    Some(crate::app_paths::cache_dir()?.join(format!("{}-portal-starts", super::ID)))
}

/// The starts gone without a window, as `text` records them for `version`.
pub(super) fn failed_starts(text: &str, version: &str) -> u32 {
    let mut parts = text.split_whitespace();
    match (parts.next(), parts.next().and_then(|count| count.parse().ok())) {
        (Some(recorded), Some(count)) if recorded == version => count,
        _ => 0,
    }
}

fn recorded() -> u32 {
    path().and_then(|path| std::fs::read_to_string(path).ok()).map_or(0, |text| failed_starts(&text, VERSION))
}

/// Whether the last starts failed: this one relays the portal's calls.
pub(super) fn tripped() -> bool {
    recorded() >= LIMIT
}

/// A start for the portal that is to show a window, counted as failed until one shows.
pub(super) fn starting() {
    let Some(path) = path() else { return };
    let written = path.parent().map_or(Ok(()), std::fs::create_dir_all)
        .and_then(|()| std::fs::write(&path, format!("{VERSION} {}\n", recorded() + 1)));
    if let Err(error) = written {
        log::warn!("Cannot count the file chooser's start: {error}");
    }
}

/// A window of Oxyde's showed, or Oxyde ends by itself: its starts work.
pub(super) fn cleared() {
    let Some(path) = path() else { return };
    if let Err(error) = std::fs::remove_file(path) {
        if error.kind() != std::io::ErrorKind::NotFound {
            log::warn!("Cannot clear the file chooser's failed starts: {error}");
        }
    }
}

#[cfg(test)]
#[path = "tests/guard.rs"]
mod tests;
