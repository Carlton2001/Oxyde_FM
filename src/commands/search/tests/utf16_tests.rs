use super::*;

fn utf16(text: &str, big_endian: bool) -> Vec<u8> {
    let mut bytes = if big_endian { vec![0xFE, 0xFF] } else { vec![0xFF, 0xFE] };
    for unit in text.encode_utf16() {
        bytes.extend(if big_endian { unit.to_be_bytes() } else { unit.to_le_bytes() });
    }
    bytes
}

#[test]
fn utf16_text_files_are_searched_by_their_text() {
    let root = tempfile::tempdir().unwrap();
    let cancellation = AtomicBool::new(false);
    for big_endian in [false, true] {
        let path = root.path().join(format!("export-{big_endian}.reg"));
        std::fs::write(&path, utf16("Windows Registry Editor\r\n\"Clé\"=\"Pomme 🍎\"\r\n", big_endian)).unwrap();
        let search = |pattern: &ContentPattern| {
            let (file, start) = open_text(&path).expect("text, not binary");
            text_contains_content(file, start, pattern, false, &cancellation)
        };
        for needle in ["Clé", "Pomme 🍎"] {
            assert!(search(&ContentPattern::new(needle, true).unwrap()), "{needle} {big_endian}");
        }
        assert!(!search(&ContentPattern::new("Poire", true).unwrap()));
    }
}

#[test]
fn a_reader_split_inside_characters_still_decodes_them() {
    struct Trickle(std::io::Cursor<Vec<u8>>);
    impl Read for Trickle {
        fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
            let length = buffer.len().min(3);
            self.0.read(&mut buffer[..length])
        }
    }
    let mut text = String::new();
    utf16_text::Utf16Reader::new(Trickle(std::io::Cursor::new(utf16("aé🍎b", false))), false)
        .read_to_string(&mut text)
        .unwrap();
    assert_eq!(text, "aé🍎b");
}
