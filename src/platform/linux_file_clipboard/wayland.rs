//! Files on the Wayland clipboard in the GNOME format file managers share
//! (`x-special/gnome-copied-files`: `copy` or `cut`, then one `file://` URI per line),
//! through the `wl-copy` and `wl-paste` processes.
use percent_encoding::{AsciiSet, NON_ALPHANUMERIC, percent_decode_str, utf8_percent_encode};
use std::io::Write;
use std::process::{Child, Command, Stdio};

const FORMAT: &str = "x-special/gnome-copied-files";
const PATH: &AsciiSet = &NON_ALPHANUMERIC.remove(b'/').remove(b'-').remove(b'.').remove(b'_').remove(b'~');

/// Offers the files until another selection replaces them.
pub(super) fn offer(paths: &[String], cut: bool) -> Option<Child> {
    if paths.is_empty() {
        return None;
    }
    let spawned = Command::new("wl-copy")
        .args(["--foreground", "--type", FORMAT])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();
    let mut server = match spawned {
        Ok(server) => server,
        Err(error) => {
            static REPORTED: std::sync::Once = std::sync::Once::new();
            REPORTED.call_once(|| log::info!("wl-copy unavailable, files are offered as text: {error}"));
            return None;
        }
    };
    // Closing stdin lets wl-copy take the selection.
    let written = server.stdin.take().map(|mut stdin| stdin.write_all(encode(paths, cut).as_bytes()));
    if !matches!(written, Some(Ok(()))) {
        let _ = server.kill();
        let _ = server.wait();
        return None;
    }
    Some(server)
}

/// Files another application offers.
pub(super) fn files() -> Option<(Vec<String>, bool)> {
    let output = Command::new("wl-paste")
        .args(["--no-newline", "--type", FORMAT])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    output.status.success().then(|| decode(&String::from_utf8_lossy(&output.stdout)))?
}

pub(super) fn encode(paths: &[String], cut: bool) -> String {
    let mut text = if cut { "cut" } else { "copy" }.to_string();
    for path in paths {
        text.push_str("\nfile://");
        text.extend(utf8_percent_encode(path, PATH));
    }
    text
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
