//! Type of an item from the shared MIME database (shared-mime-info), as Nautilus
//! shows it: by file name, then by content for a file whose name says nothing.
use crate::models::MimeType;
use std::{io::Read, path::{Path, PathBuf}, sync::OnceLock};

const DIRECTORY: &str = "inode/directory";
const TEXT: &str = "text/plain";
const UNKNOWN: &str = "application/octet-stream";

struct Glob {
    weight: u32,
    mime: String,
    pattern: glob::Pattern,
    case_sensitive: bool,
    literal: bool,
    length: usize,
}

fn data_dirs() -> Vec<PathBuf> {
    let home = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| dirs::home_dir().map(|home| home.join(".local/share")));
    let system = std::env::var("XDG_DATA_DIRS")
        .ok()
        .filter(|dirs| !dirs.is_empty())
        .unwrap_or_else(|| "/usr/local/share:/usr/share".into());
    home.into_iter().chain(system.split(':').map(PathBuf::from)).collect()
}

fn globs() -> &'static [Glob] {
    static GLOBS: OnceLock<Vec<Glob>> = OnceLock::new();
    GLOBS.get_or_init(|| {
        let mut globs = Vec::new();
        for dir in data_dirs() {
            let Ok(text) = std::fs::read_to_string(dir.join("mime/globs2")) else { continue };
            globs.extend(parse(&text));
        }
        globs
    })
}

/// `weight:type:glob[:flags]` lines of a `globs2` file.
fn parse(text: &str) -> Vec<Glob> {
    text.lines()
        .filter(|line| !line.starts_with('#'))
        .filter_map(|line| {
            let mut fields = line.splitn(4, ':');
            let weight = fields.next()?.parse().ok()?;
            let mime = fields.next()?.to_owned();
            let pattern = fields.next()?;
            let case_sensitive = fields.next().is_some_and(|flags| flags.split(',').any(|flag| flag == "cs"));
            Some(Glob {
                weight,
                mime,
                literal: !pattern.contains(['*', '?', '[']),
                length: pattern.len(),
                pattern: glob::Pattern::new(pattern).ok()?,
                case_sensitive,
            })
        })
        .collect()
}

/// Literal names first, then the heaviest glob, then the longest one, as the
/// shared-mime-info specification orders them.
fn by_name<'a>(globs: &'a [Glob], name: &str) -> Option<&'a str> {
    globs
        .iter()
        .filter(|glob| {
            glob.pattern.matches_with(name, glob::MatchOptions {
                case_sensitive: glob.case_sensitive,
                ..Default::default()
            })
        })
        .max_by_key(|glob| (glob.literal, glob.weight, glob.length))
        .map(|glob| glob.mime.as_str())
}

/// Text without NUL in its first block is plain text; anything else is unknown.
fn by_content(path: &Path) -> &'static str {
    let mut block = [0_u8; 4096];
    let read = std::fs::File::open(path).and_then(|mut file| file.read(&mut block));
    match read {
        Ok(length) if !block[..length].contains(&0) && utf8_prefix(&block[..length]) => TEXT,
        _ => UNKNOWN,
    }
}

/// UTF-8 text, allowing a character cut at the end of the block.
fn utf8_prefix(bytes: &[u8]) -> bool {
    match std::str::from_utf8(bytes) {
        Ok(_) => true,
        Err(error) => error.error_len().is_none(),
    }
}

/// The `<comment>` of the type in this language, or the untranslated one.
fn description(mime: &str, language: &str) -> Option<String> {
    let text = data_dirs()
        .into_iter()
        .find_map(|dir| std::fs::read_to_string(dir.join("mime").join(format!("{mime}.xml"))).ok())?;
    let comment = |open: &str| {
        let start = text.find(open)? + open.len();
        let end = text[start..].find("</comment>")? + start;
        Some(unescape(&text[start..end]))
    };
    comment(&format!("<comment xml:lang=\"{language}\">")).or_else(|| comment("<comment>"))
}

fn unescape(text: &str) -> String {
    text.replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&apos;", "'").replace("&amp;", "&")
}

/// Devices, pipes and sockets are named by their kind: opening a pipe would wait.
fn special(path: &Path) -> Option<&'static str> {
    use std::os::unix::fs::FileTypeExt;
    let kind = std::fs::metadata(path).ok()?.file_type();
    [
        (kind.is_char_device(), "inode/chardevice"),
        (kind.is_block_device(), "inode/blockdevice"),
        (kind.is_fifo(), "inode/fifo"),
        (kind.is_socket(), "inode/socket"),
    ]
    .into_iter()
    .find_map(|(matches, mime)| matches.then_some(mime))
}

pub(super) fn mime_type(path: &Path, is_dir: bool, language: &str) -> MimeType {
    let name = if is_dir {
        DIRECTORY
    } else if let Some(mime) = special(path) {
        mime
    } else {
        path.file_name()
            .and_then(|name| by_name(globs(), &name.to_string_lossy()))
            .unwrap_or_else(|| by_content(path))
    };
    MimeType { description: description(name, language), name: name.to_owned() }
}

#[cfg(test)]
#[path = "linux_mime/tests/mime.rs"]
mod tests;
