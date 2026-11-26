//! Path text that keeps a name which is not valid Unicode without confusing it with
//! another file. On Unix, each byte that is not UTF-8 is written as NUL followed by two
//! hexadecimal digits; on Windows, each lone surrogate of an NTFS name as NUL followed
//! by four. No path holds NUL, so the text is reversible, and any code that opens it
//! without `decode` fails instead of reaching the file whose name it resembles. A
//! Windows API reads its text up to the first NUL, a shorter path: `wide` and
//! `hstring` decode it, and refuse a NUL left over. Valid paths keep their usual text.
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
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        match path.to_str() {
            Some(text) => text.to_owned(),
            None => encode_wide(path.as_os_str().encode_wide()),
        }
    }
}

/// `encode` of the UTF-16 of a Windows name.
#[cfg(any(windows, test))]
fn encode_wide(units: impl IntoIterator<Item = u16>) -> String {
    let mut text = String::new();
    for character in char::decode_utf16(units) {
        match character {
            Ok(character) => text.push(character),
            Err(error) => {
                text.push(ESCAPE);
                text.push_str(&format!("{:04X}", error.unpaired_surrogate()));
            }
        }
    }
    text
}

/// The UTF-16 `encode_wide` wrote. Only a surrogate is unescaped: a malformed escape
/// keeps its NUL.
#[cfg(any(windows, test))]
fn decode_wide(text: &str) -> Vec<u16> {
    let mut units = Vec::with_capacity(text.len());
    let mut rest = text;
    while let Some(character) = rest.chars().next() {
        let escaped = (character == ESCAPE)
            .then(|| rest.get(1..5))
            .flatten()
            .filter(|digits| digits.bytes().all(|byte| byte.is_ascii_hexdigit()))
            .and_then(|digits| u16::from_str_radix(digits, 16).ok())
            .filter(|unit| (0xD800..=0xDFFF).contains(unit));
        match escaped {
            Some(unit) => {
                units.push(unit);
                rest = &rest[5..];
            }
            None => {
                units.extend_from_slice(character.encode_utf16(&mut [0; 2]));
                rest = &rest[character.len_utf8()..];
            }
        }
    }
    units
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
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStringExt;
        if !is_encoded(text) {
            return PathBuf::from(text);
        }
        PathBuf::from(std::ffi::OsString::from_wide(&decode_wide(text)))
    }
}

/// The path for a Windows API, NUL-terminated: decoded when it is `encode`'s text,
/// refused when a NUL remains, since the API would stop there at another path.
#[cfg(windows)]
pub fn wide(path: impl AsRef<Path>) -> std::io::Result<Vec<u16>> {
    use std::os::windows::ffi::OsStrExt;
    let path = path.as_ref();
    let mut units: Vec<u16> = decoded(path).as_os_str().encode_wide().collect();
    if units.contains(&0) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("Unrepresentable path [{}]", display(&path.to_string_lossy())),
        ));
    }
    units.push(0);
    Ok(units)
}

/// `wide` as an `HSTRING`.
#[cfg(windows)]
pub fn hstring(path: impl AsRef<Path>) -> std::io::Result<windows::core::HSTRING> {
    let units = wide(path)?;
    Ok(windows::core::HSTRING::from_wide(&units[..units.len() - 1]))
}

/// A path built from `encode`'s text without decoding it, decoded; any other path as it is.
pub fn decoded(path: &Path) -> std::borrow::Cow<'_, Path> {
    match path.to_str() {
        Some(text) if is_encoded(text) => std::borrow::Cow::Owned(decode(text)),
        _ => std::borrow::Cow::Borrowed(path),
    }
}

/// Whether the text holds a name that is not valid Unicode.
pub fn is_encoded(text: &str) -> bool {
    text.contains(ESCAPE)
}

/// The text shown to the user: each invalid byte or lone surrogate becomes U+FFFD, as
/// GLib shows an invalid byte.
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
