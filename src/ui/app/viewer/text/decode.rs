//! Text files in the encodings Windows tools write: UTF-8 with or without a byte order mark,
//! UTF-16 with one, and legacy single-byte text, read as Windows-1252.

/// The characters of Windows-1252 bytes 0x80 to 0x9F; the five it leaves undefined stay controls.
const CP1252_HIGH: [char; 32] = [
    '€', '\u{81}', '‚', 'ƒ', '„', '…', '†', '‡', 'ˆ', '‰', 'Š', '‹', 'Œ', '\u{8D}', 'Ž', '\u{8F}',
    '\u{90}', '‘', '’', '“', '”', '•', '–', '—', '˜', '™', 'š', '›', 'œ', '\u{9D}', 'ž', 'Ÿ',
];

/// The text of `bytes`. Valid UTF-8 keeps its buffer; a byte order mark is dropped.
pub(in super::super) fn decode(mut bytes: Vec<u8>) -> String {
    match bytes.as_slice() {
        [0xFF, 0xFE, rest @ ..] => return utf16(rest, u16::from_le_bytes),
        [0xFE, 0xFF, rest @ ..] => return utf16(rest, u16::from_be_bytes),
        [0xEF, 0xBB, 0xBF, ..] => {
            bytes.drain(..3);
        }
        _ => {}
    }
    match String::from_utf8(bytes) {
        Ok(text) => text,
        Err(error) => {
            let utf8 = error.utf8_error();
            // A cut inside the last character (a file read up to a limit) is not another encoding.
            let truncated = utf8.error_len().is_none() && error.as_bytes().len() - utf8.valid_up_to() < 4;
            if truncated {
                String::from_utf8_lossy(error.as_bytes()).into_owned()
            } else {
                error.as_bytes().iter().map(|&byte| cp1252(byte)).collect()
            }
        }
    }
}

fn utf16(bytes: &[u8], unit: fn([u8; 2]) -> u16) -> String {
    let units = bytes.chunks_exact(2).map(|pair| unit([pair[0], pair[1]]));
    char::decode_utf16(units).map(|decoded| decoded.unwrap_or(char::REPLACEMENT_CHARACTER)).collect()
}

fn cp1252(byte: u8) -> char {
    match byte {
        0x80..=0x9F => CP1252_HIGH[usize::from(byte - 0x80)],
        _ => char::from(byte),
    }
}

#[cfg(test)]
#[path = "tests/decode.rs"]
mod tests;
