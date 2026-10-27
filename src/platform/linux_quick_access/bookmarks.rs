//! GTK bookmarks: one `URI [name]` per line. Oxyde lists local folders and rewrites the
//! file atomically, keeping the lines it does not show (network locations, names).
use crate::models::QuickAccessItem;
use percent_encoding::{AsciiSet, NON_ALPHANUMERIC, percent_decode_str, utf8_percent_encode};
use std::io;
use std::path::Path;

/// Characters GLib's `g_filename_to_uri` leaves unescaped in a path.
const PATH: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'/').remove(b'-').remove(b'.').remove(b'_').remove(b'~')
    .remove(b'!').remove(b'$').remove(b'&').remove(b'\'').remove(b'(').remove(b')')
    .remove(b'*').remove(b'+').remove(b',').remove(b';').remove(b'=').remove(b':').remove(b'@');

/// The local folder a line points to, and its optional name.
fn parse(line: &str) -> Option<(String, Option<&str>)> {
    let (uri, name) = match line.trim().split_once(' ') {
        Some((uri, name)) => (uri, Some(name).filter(|name| !name.is_empty())),
        None => (line.trim(), None),
    };
    let path = uri.strip_prefix("file://")?;
    path.starts_with('/')
        .then(|| percent_decode_str(path).decode_utf8().ok())
        .flatten()
        .map(|path| (path.into_owned(), name))
}

fn read(file: &Path) -> io::Result<String> {
    match std::fs::read_to_string(file) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(String::new()),
        result => result,
    }
}

/// Replaces the file as GTK does, so that a reader never sees half of it.
fn write(file: &Path, lines: &[&str]) -> io::Result<()> {
    let folder = file.parent().ok_or_else(|| io::Error::other("Bookmarks have no folder"))?;
    std::fs::create_dir_all(folder)?;
    let mut temporary = tempfile::NamedTempFile::new_in(folder)?;
    io::Write::write_all(&mut temporary, lines.iter().map(|line| format!("{line}\n")).collect::<String>().as_bytes())?;
    temporary.as_file().sync_all()?;
    temporary.persist(file).map(|_| ()).map_err(|error| error.error)
}

pub(super) fn items(file: &Path) -> Vec<QuickAccessItem> {
    let content = read(file).unwrap_or_else(|error| {
        log::warn!("Cannot read GTK bookmarks {}: {error}", file.display());
        String::new()
    });
    content
        .lines()
        .filter_map(parse)
        .map(|(path, name)| QuickAccessItem {
            name: name.map(str::to_string).unwrap_or_else(|| {
                Path::new(&path)
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_else(|| path.clone())
            }),
            path,
        })
        .collect()
}

pub(super) fn pin(file: &Path, path: &str) -> io::Result<()> {
    let content = read(file)?;
    if content.lines().filter_map(parse).any(|(pinned, _)| pinned == path) {
        return Ok(());
    }
    let line = format!("file://{}", utf8_percent_encode(path, PATH));
    let mut lines: Vec<&str> = content.lines().collect();
    lines.push(&line);
    write(file, &lines)
}

pub(super) fn unpin(file: &Path, path: &str) -> io::Result<()> {
    let content = read(file)?;
    let lines: Vec<&str> = content
        .lines()
        .filter(|line| parse(line).is_none_or(|(pinned, _)| pinned != path))
        .collect();
    if lines.len() == content.lines().count() {
        return Ok(());
    }
    write(file, &lines)
}
