//! Files other applications put on the Wayland clipboard, read through the `wl-paste`
//! process: in the GNOME format file managers share (`x-special/gnome-copied-files`:
//! `copy` or `cut`, then one `file://` URI per line), otherwise as a URI list, which
//! Dolphin marks cut with `application/x-kde-cutselection`, as Nautilus reads them.
use percent_encoding::percent_decode_str;
use std::process::Command;
use std::time::{Duration, Instant};

pub(super) const FORMAT: &str = "x-special/gnome-copied-files";
pub(super) const URI_LIST: &str = "text/uri-list";
const KDE_CUT: &str = "application/x-kde-cutselection";
/// The types in which other applications offer files, the preferred one first.
pub(super) const TYPES: [&str; 2] = [FORMAT, URI_LIST];

/// A clipboard owner that stops answering must not hold the paste.
const TIMEOUT: Duration = Duration::from_secs(3);
/// Paths of tens of thousands of files.
const LIMIT: usize = 16 * 1024 * 1024;

/// Whether the session is Wayland, from the variable GPUI chooses its backend with. Under
/// X11, `wl-paste` has no compositor to reach.
pub(super) fn session() -> bool {
    std::env::var_os("WAYLAND_DISPLAY").is_some_and(|display| !display.is_empty())
}

/// Files another application offers. A clipboard owner that stops answering holds the
/// whole read for `TIMEOUT` at most.
pub(super) fn files() -> Option<(Vec<String>, bool)> {
    if !session() {
        return None;
    }
    let deadline = Instant::now() + TIMEOUT;
    // The compositor lists the types; only reading one asks the owner.
    let types = paste(&["--list-types"], deadline)?;
    let types: Vec<&str> = types.lines().map(str::trim).collect();
    if types.contains(&FORMAT) {
        return decode(&paste(&["--no-newline", "--type", FORMAT], deadline)?);
    }
    if !types.contains(&URI_LIST) {
        return None;
    }
    let paths = uris(paste(&["--no-newline", "--type", URI_LIST], deadline)?.lines())?;
    let cut = types.contains(&KDE_CUT)
        && paste(&["--no-newline", "--type", KDE_CUT], deadline).is_some_and(|text| text.trim() == "1");
    Some((paths, cut))
}

fn paste(args: &[&str], deadline: Instant) -> Option<String> {
    let mut command = Command::new("wl-paste");
    command.args(args);
    let timeout = deadline.saturating_duration_since(Instant::now());
    let output = super::super::linux_process::output_within(command, timeout, LIMIT).ok()?;
    output.status.success().then(|| String::from_utf8_lossy(&output.stdout).into_owned())
}

pub(super) fn decode(text: &str) -> Option<(Vec<String>, bool)> {
    let mut lines = text.lines();
    let cut = match lines.next()?.trim_end_matches('\r') {
        "copy" => false,
        "cut" => true,
        _ => return None,
    };
    Some((uris(lines)?, cut))
}

/// Local paths of `file://` URIs, one per line; other lines and URIs are skipped, as
/// comments in a URI list.
pub(super) fn uris<'a>(lines: impl Iterator<Item = &'a str>) -> Option<Vec<String>> {
    let paths: Vec<String> = lines
        .map(|line| line.trim_end_matches('\r'))
        .filter_map(|uri| uri.strip_prefix("file://"))
        .filter(|path| path.starts_with('/'))
        .filter_map(|path| percent_decode_str(path).decode_utf8().ok().map(String::from))
        .collect();
    (!paths.is_empty()).then_some(paths)
}
