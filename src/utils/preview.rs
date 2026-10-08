use std::io::{self, Read};
use std::path::Path;

const MAX_TEXT_BYTES: u64 = 64 * 1024;
const MAX_TEXT_CHARACTERS: usize = 4096;
const MAX_TEXT_LINES: usize = 15;

pub fn read_text_preview(path: &Path) -> io::Result<String> {
    text_preview(std::fs::File::open(path)?)
}

fn text_preview(reader: impl Read) -> io::Result<String> {
    let mut bytes = Vec::new();
    reader.take(MAX_TEXT_BYTES).read_to_end(&mut bytes)?;
    let text = String::from_utf8_lossy(&bytes);
    // The intermediate snippet is bounded by the read budget. Keep the existing
    // lines() behavior for CRLF and a final newline, then bound display length.
    let snippet = text
        .lines()
        .take(MAX_TEXT_LINES)
        .collect::<Vec<_>>()
        .join("\n");
    Ok(snippet.chars().take(MAX_TEXT_CHARACTERS).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_long_single_line_and_an_unbounded_reader_remain_bounded() {
        let preview = text_preview(io::repeat(b'a')).unwrap();
        assert_eq!(preview.len(), MAX_TEXT_CHARACTERS);
        let mut input = io::Cursor::new(vec![b'a'; MAX_TEXT_BYTES as usize + 1]);
        text_preview(&mut input).unwrap();
        assert_eq!(input.position(), MAX_TEXT_BYTES);
    }

    #[test]
    fn lines_unicode_and_short_files_are_preserved() {
        assert_eq!(text_preview("école\n🦀".as_bytes()).unwrap(), "école\n🦀");
        assert_eq!(
            text_preview(b"first\r\nsecond\r\n".as_slice()).unwrap(),
            "first\nsecond"
        );
        let input = "line\n".repeat(30);
        assert_eq!(
            text_preview(input.as_bytes()).unwrap().lines().count(),
            MAX_TEXT_LINES
        );
        let mut input = vec![b'a'; MAX_TEXT_BYTES as usize - 1];
        input.extend_from_slice("🦀".as_bytes());
        assert!(text_preview(input.as_slice()).is_ok());
    }
}
