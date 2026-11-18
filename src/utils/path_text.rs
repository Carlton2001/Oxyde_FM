//! Path text that keeps a Unix name which is not UTF-8 without confusing it with
//! another file. Each invalid byte is written as NUL followed by two hexadecimal
//! digits: no Unix path holds NUL, so the text is reversible, and any code that opens
//! it without `decode` fails instead of reaching the file whose name it resembles.
//! Windows paths and valid UTF-8 paths keep their usual text.
use std::path::{Path, PathBuf};

const ESCAPE: char = '\0';

pub fn encode(path: &Path) -> String {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        if let Some(text) = path.to_str() {
            return text.to_owned();
        }
        let mut text = String::new();
        for chunk in path.as_os_str().as_bytes().utf8_chunks() {
            text.push_str(chunk.valid());
            for byte in chunk.invalid() {
                text.push(ESCAPE);
                text.push_str(&format!("{byte:02X}"));
            }
        }
        text
    }
    #[cfg(not(unix))]
    {
        path.to_string_lossy().into_owned()
    }
}

/// The path written by `encode`. A malformed escape keeps its NUL, so the path
/// still cannot be opened.
pub fn decode(text: &str) -> PathBuf {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStringExt;
        if !is_encoded(text) {
            return PathBuf::from(text);
        }
        let mut bytes = Vec::with_capacity(text.len());
        let mut rest = text.as_bytes();
        while let Some((&first, tail)) = rest.split_first() {
            let escaped = (first == 0)
                .then(|| tail.get(..2))
                .flatten()
                .and_then(|digits| std::str::from_utf8(digits).ok())
                .and_then(|digits| u8::from_str_radix(digits, 16).ok());
            match escaped {
                Some(byte) => {
                    bytes.push(byte);
                    rest = &tail[2..];
                }
                None => {
                    bytes.push(first);
                    rest = tail;
                }
            }
        }
        PathBuf::from(std::ffi::OsString::from_vec(bytes))
    }
    #[cfg(not(unix))]
    {
        PathBuf::from(text)
    }
}

/// Whether the text holds a name that is not UTF-8.
pub fn is_encoded(text: &str) -> bool {
    text.contains(ESCAPE)
}

/// The text shown to the user: each invalid byte becomes U+FFFD, as GLib shows it.
pub fn display(text: &str) -> String {
    if !is_encoded(text) {
        return text.to_owned();
    }
    decode(text).to_string_lossy().into_owned()
}

/// A Windows extended-length path in its usual form: `\\?\C:\…` as `C:\…` and
/// `\\?\UNC\server\share\…` as `\\server\share\…`. Other verbatim forms
/// (`\\?\Volume{…}\…`) have no usual form and stay as they are.
pub fn without_verbatim_prefix(text: &str) -> String {
    if let Some(rest) = text.strip_prefix(r"\\?\UNC\") {
        return format!(r"\\{rest}");
    }
    match text.strip_prefix(r"\\?\") {
        Some(rest) if rest.as_bytes().get(1) == Some(&b':') && rest.as_bytes()[0].is_ascii_alphabetic() => {
            rest.to_owned()
        }
        _ => text.to_owned(),
    }
}

#[cfg(test)]
#[path = "path_text/tests/encoding.rs"]
mod tests;

#[cfg(test)]
#[path = "path_text/tests/verbatim.rs"]
mod verbatim_tests;
