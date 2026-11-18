//! Files other applications put on the Wayland clipboard, in the GNOME format file
//! managers share (`x-special/gnome-copied-files`: `copy` or `cut`, then one `file://` URI
//! per line), read through the `wl-paste` process.
use percent_encoding::percent_decode_str;
use std::process::{Command, Stdio};

const FORMAT: &str = "x-special/gnome-copied-files";

/// Whether the session is Wayland, from the variable GPUI chooses its backend with. Under
/// X11, `wl-paste` has no compositor to reach.
pub(super) fn session() -> bool {
    std::env::var_os("WAYLAND_DISPLAY").is_some_and(|display| !display.is_empty())
}

/// Files another application offers.
pub(super) fn files() -> Option<(Vec<String>, bool)> {
    if !session() {
        return None;
    }
    let output = Command::new("wl-paste")
        .args(["--no-newline", "--type", FORMAT])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    output.status.success().then(|| decode(&String::from_utf8_lossy(&output.stdout)))?
}

pub(super) fn decode(text: &str) -> Option<(Vec<String>, bool)> {
    let mut lines = text.lines().map(|line| line.trim_end_matches('\r'));
    let cut = match lines.next()? {
        "copy" => false,
        "cut" => true,
        _ => return None,
    };
    let paths: Vec<String> = lines
        .filter_map(|uri| uri.strip_prefix("file://"))
        .filter(|path| path.starts_with('/'))
        .filter_map(|path| percent_decode_str(path).decode_utf8().ok().map(String::from))
        .collect();
    (!paths.is_empty()).then_some((paths, cut))
}
