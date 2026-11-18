use super::decode;

#[test]
fn utf8_is_kept_and_its_byte_order_mark_dropped() {
    assert_eq!(decode("héllo".as_bytes().to_vec()), "héllo");
    assert_eq!(decode(b"\xEF\xBB\xBF# Title".to_vec()), "# Title");
}

#[test]
fn utf16_with_a_byte_order_mark_is_decoded() {
    let little: Vec<u8> = [0xFF, 0xFE].into_iter().chain("a€\r\n".encode_utf16().flat_map(u16::to_le_bytes)).collect();
    let big: Vec<u8> = [0xFE, 0xFF].into_iter().chain("a€\r\n".encode_utf16().flat_map(u16::to_be_bytes)).collect();
    assert_eq!(decode(little), "a€\r\n");
    assert_eq!(decode(big), "a€\r\n");
}

#[test]
fn legacy_text_reads_as_windows_1252() {
    assert_eq!(decode(b"caf\xE9 \x80 \x93x\x94".to_vec()), "café € “x”");
}

#[test]
fn a_character_cut_at_the_end_is_not_another_encoding() {
    let mut bytes = "é".as_bytes().to_vec();
    bytes.insert(0, b'a');
    bytes.pop();
    assert_eq!(decode(bytes), "a\u{FFFD}");
}
