//! Type of an item from the shared MIME database (shared-mime-info), as Nautilus
//! shows it through GIO: by file name when it names one type, else by content.
mod magic;

use crate::models::MimeType;
use std::{collections::HashMap, io::Read, path::{Path, PathBuf}, sync::OnceLock};

const DIRECTORY: &str = "inode/directory";
const TEXT: &str = "text/plain";
const UNKNOWN: &str = "application/octet-stream";
const EXECUTABLE: &str = "application/x-executable";
const ZERO_SIZE: &str = "application/x-zerosize";

struct Glob {
    weight: u32,
    mime: String,
    pattern: glob::Pattern,
    case_sensitive: bool,
    kind: GlobKind,
}

/// xdgmime looks names up in this order.
#[derive(Clone, Copy, PartialEq, Eq)]
enum GlobKind {
    Literal,
    /// `*` and a suffix: the longest matching suffix wins.
    Suffix(usize),
    Full,
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

/// The globs of a database, indexed: a name is looked up by itself and by each of its
/// suffixes, instead of being matched against every glob.
struct Globs {
    globs: Vec<Glob>,
    /// Literal names and `*suffix` globs, by their ASCII lowercase text (the case
    /// `glob` ignores), each list in file order.
    literals: HashMap<String, Vec<usize>>,
    suffixes: HashMap<String, Vec<usize>>,
    full: Vec<usize>,
}

impl Globs {
    fn new(globs: Vec<Glob>) -> Self {
        let mut index = Self { globs: Vec::new(), literals: HashMap::new(), suffixes: HashMap::new(), full: Vec::new() };
        for (position, glob) in globs.iter().enumerate() {
            let pattern = glob.pattern.as_str().to_ascii_lowercase();
            match glob.kind {
                GlobKind::Literal => index.literals.entry(pattern).or_default().push(position),
                GlobKind::Suffix(_) => index.suffixes.entry(pattern[1..].to_owned()).or_default().push(position),
                GlobKind::Full => index.full.push(position),
            }
        }
        index.globs = globs;
        index
    }
}

fn globs() -> &'static Globs {
    static GLOBS: OnceLock<Globs> = OnceLock::new();
    GLOBS.get_or_init(|| {
        let mut globs = Vec::new();
        for dir in data_dirs() {
            let Ok(text) = std::fs::read_to_string(dir.join("mime/globs2")) else { continue };
            globs.extend(parse(&text));
        }
        Globs::new(globs)
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
                kind: match pattern.strip_prefix('*') {
                    _ if !pattern.contains(['*', '?', '[']) => GlobKind::Literal,
                    Some(suffix) if !suffix.contains(['*', '?', '[']) => GlobKind::Suffix(suffix.len()),
                    _ => GlobKind::Full,
                },
                pattern: glob::Pattern::new(pattern).ok()?,
                case_sensitive,
            })
        })
        .collect()
}

/// Content rules of every data directory, highest priority first, and how many bytes
/// of a file they read.
fn magic_sections() -> &'static (Vec<magic::Section>, usize) {
    static SECTIONS: OnceLock<(Vec<magic::Section>, usize)> = OnceLock::new();
    SECTIONS.get_or_init(|| {
        let mut sections: Vec<_> = data_dirs()
            .into_iter()
            .filter_map(|dir| std::fs::read(dir.join("mime/magic")).ok())
            .flat_map(|bytes| magic::parse(&bytes))
            .collect();
        sections.sort_by_key(|section| std::cmp::Reverse(section.priority));
        let extent = magic::extent(&sections);
        (sections, extent)
    })
}

fn subclasses() -> &'static HashMap<String, Vec<String>> {
    static SUBCLASSES: OnceLock<HashMap<String, Vec<String>>> = OnceLock::new();
    SUBCLASSES.get_or_init(|| {
        let mut parents = HashMap::new();
        for dir in data_dirs() {
            let Ok(text) = std::fs::read_to_string(dir.join("mime/subclasses")) else { continue };
            parse_subclasses(&text, &mut parents);
        }
        parents
    })
}

/// Other names of types, by alias, from the `aliases` files.
fn aliases() -> &'static HashMap<String, String> {
    static ALIASES: OnceLock<HashMap<String, String>> = OnceLock::new();
    ALIASES.get_or_init(|| {
        let mut aliases = HashMap::new();
        for dir in data_dirs() {
            let Ok(text) = std::fs::read_to_string(dir.join("mime/aliases")) else { continue };
            parse_aliases(&text, &mut aliases);
        }
        aliases
    })
}

/// `alias type` lines of an `aliases` file; the first directory wins.
fn parse_aliases(text: &str, aliases: &mut HashMap<String, String>) {
    for line in text.lines() {
        if let Some((alias, canonical)) = line.split_once(' ') {
            aliases.entry(alias.to_ascii_lowercase()).or_insert_with(|| canonical.to_ascii_lowercase());
        }
    }
}

/// Whether `mime` answers a file chooser's `pattern`, as GTK's filters read it: `*` or
/// `*/*` any type, `image/*` a type of that media, else the type, one of its aliases or
/// one of its subclasses. Types ignore case.
fn matches(parents: &HashMap<String, Vec<String>>, aliases: &HashMap<String, String>, mime: &str, pattern: &str) -> bool {
    let canonical = |name: &str| {
        let name = name.to_ascii_lowercase();
        aliases.get(&name).cloned().unwrap_or(name)
    };
    let pattern = pattern.trim().to_ascii_lowercase();
    if pattern == "*" || pattern == "*/*" {
        return true;
    }
    let mime = canonical(mime);
    match pattern.strip_suffix("/*") {
        Some(media) => mime.split_once('/').is_some_and(|(major, _)| major == media),
        None => is_a(parents, &mime, &canonical(&pattern)),
    }
}

pub(super) fn type_matches(mime: &str, pattern: &str) -> bool {
    matches(subclasses(), aliases(), mime, pattern)
}

/// `child parent` lines of a `subclasses` file.
fn parse_subclasses(text: &str, parents: &mut HashMap<String, Vec<String>>) {
    for line in text.lines() {
        if let Some((child, parent)) = line.split_once(' ') {
            parents.entry(child.to_owned()).or_default().push(parent.to_owned());
        }
    }
}

/// Whether `mime` is `ancestor` or one of its subclasses; every `text/*` type is a
/// kind of `text/plain`, as the specification states.
fn is_a(parents: &HashMap<String, Vec<String>>, mime: &str, ancestor: &str) -> bool {
    let mut pending = vec![mime.to_owned()];
    let mut seen = std::collections::HashSet::new();
    while let Some(mime) = pending.pop() {
        if mime == ancestor || (ancestor == TEXT && mime.starts_with("text/")) {
            return true;
        }
        if seen.insert(mime.clone()) {
            pending.extend(parents.get(&mime).into_iter().flatten().cloned());
        }
    }
    false
}

/// GIO's rule, which Nautilus follows to offer execution.
fn can_be_executable(parents: &HashMap<String, Vec<String>>, mime: &str) -> bool {
    is_a(parents, mime, EXECUTABLE) || is_a(parents, mime, TEXT)
}

/// Types the name allows, heaviest first, as xdgmime finds them: an exact name alone;
/// else the longest matching `*suffix`, then the other globs if that left fewer than two.
/// Exact names ignore case unless marked otherwise. Globs try the exact case first,
/// then ignore case for those that allow it, when nothing matched.
fn by_name<'a>(globs: &'a Globs, name: &str) -> Vec<&'a str> {
    let lowercase = name.to_ascii_lowercase();
    let literal = globs.literals.get(&lowercase).into_iter().flatten().map(|&index| &globs.globs[index]).find(|glob| {
        glob.pattern.matches_with(name, glob::MatchOptions { case_sensitive: glob.case_sensitive, ..Default::default() })
    });
    if let Some(glob) = literal {
        return vec![glob.mime.as_str()];
    }
    // The `*suffix` globs that may match: those of each ending of the name, in file order.
    let mut candidates: Vec<usize> = lowercase.char_indices()
        .flat_map(|(start, _)| globs.suffixes.get(&lowercase[start..]).into_iter().flatten().copied())
        .chain(globs.suffixes.get("").into_iter().flatten().copied())
        .collect();
    candidates.sort_unstable();
    let exact = by_glob(globs, &candidates, name, true);
    if exact.is_empty() { by_glob(globs, &candidates, name, false) } else { exact }
}

fn by_glob<'a>(globs: &'a Globs, candidates: &[usize], name: &str, exact: bool) -> Vec<&'a str> {
    let matches = |glob: &&Glob| {
        let case_sensitive = exact || glob.case_sensitive;
        glob.pattern.matches_with(name, glob::MatchOptions { case_sensitive, ..Default::default() })
    };
    let suffixes: Vec<_> = candidates.iter().map(|&index| &globs.globs[index]).filter(matches).collect();
    let longest = suffixes.iter().filter_map(|glob| match glob.kind { GlobKind::Suffix(length) => Some(length), _ => None }).max();
    let mut found: Vec<_> = suffixes.into_iter().filter(|glob| glob.kind == GlobKind::Suffix(longest.unwrap_or(0))).collect();
    if found.len() < 2 {
        found.extend(globs.full.iter().map(|&index| &globs.globs[index]).filter(matches));
    }
    found.sort_by_key(|glob| std::cmp::Reverse(glob.weight));
    let mut types: Vec<&str> = Vec::new();
    for glob in found {
        if !types.contains(&glob.mime.as_str()) {
            types.push(&glob.mime);
        }
    }
    types
}

/// GIO's test, measured against `gio info`: no control character other than tab,
/// line feed, form feed, carriage return and backspace in the first 128 bytes.
fn looks_like_text(data: &[u8]) -> bool {
    data.iter().take(128).all(|byte| *byte >= 0x20 || matches!(byte, b'\t' | b'\n' | 0x0c | b'\r' | 0x08))
}

/// As `g_content_type_guess` given the start of the file: a content rule of priority
/// 80 or more wins; otherwise it decides between the types the name allows, else the
/// name's heaviest type; without a name, the content alone, then text or unknown.
fn by_content<'a>(sections: &'a [magic::Section], extent: usize, path: &Path, named: &[&'a str]) -> &'a str {
    let mut data = Vec::new();
    let limit = extent.clamp(4096, 1 << 16) as u64;
    if std::fs::File::open(path).and_then(|file| file.take(limit).read_to_end(&mut data)).is_err() {
        return named.first().copied().unwrap_or(UNKNOWN);
    }
    let sniffed = magic::lookup(sections, &data);
    let sniffed_type = sniffed.map(|(mime, _)| mime).or_else(|| looks_like_text(&data).then_some(TEXT));
    let Some(first) = named.first().copied() else {
        return sniffed_type.unwrap_or(UNKNOWN);
    };
    match (sniffed, sniffed_type) {
        (Some((mime, priority)), _) if priority >= 80 => mime,
        (_, Some(sniffed)) => named
            .iter()
            .copied()
            .find(|name| is_a(subclasses(), name, sniffed))
            .unwrap_or(first),
        _ => first,
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

/// The type's name alone: by file name when it names one type, else by content.
pub(super) fn type_name(path: &Path, is_dir: bool) -> &'static str {
    if is_dir {
        DIRECTORY
    } else if let Some(mime) = special(path) {
        mime
    } else {
        let named = path.file_name().map(|name| by_name(globs(), &name.to_string_lossy())).unwrap_or_default();
        // GLib does not read an empty file: files of /proc and /sys look empty.
        let empty = std::fs::metadata(path).is_ok_and(|metadata| metadata.is_file() && metadata.len() == 0);
        match named[..] {
            _ if empty => ZERO_SIZE,
            [only] => only,
            _ => {
                let (sections, extent) = magic_sections();
                by_content(sections, *extent, path, &named)
            }
        }
    }
}

pub(super) fn mime_type(path: &Path, is_dir: bool, language: &str) -> MimeType {
    let name = type_name(path, is_dir);
    MimeType {
        description: description(name, language),
        can_be_executable: can_be_executable(subclasses(), name),
        name: name.to_owned(),
    }
}

#[cfg(test)]
#[path = "linux_mime/tests/mime.rs"]
mod tests;
