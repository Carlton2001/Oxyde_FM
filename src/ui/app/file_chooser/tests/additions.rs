use super::one_line;

#[test]
fn a_field_s_text_goes_back_on_one_line() {
    assert_eq!(one_line("a\tb\r\nc"), "a b  c");
    assert_eq!(one_line("plain"), "plain");
}
