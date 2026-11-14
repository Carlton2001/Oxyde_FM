use super::{size_text, size_unit};
use crate::ui::i18n::Language;
use crate::ui::workspace::SizeRange;

#[test]
fn size_bounds_reopen_in_the_largest_unit_reading_at_least_one() {
    assert_eq!(size_unit(&SizeRange::default()), 2);
    assert_eq!(size_unit(&SizeRange { min: Some(500), max: None }), 1);
    assert_eq!(size_unit(&SizeRange { min: Some(10 * 1024), max: None }), 1);
    assert_eq!(size_unit(&SizeRange { min: None, max: Some(5 * 1024 * 1024) }), 2);
    assert_eq!(size_unit(&SizeRange { min: Some(1024), max: Some(3 * 1024 * 1024 * 1024) }), 3);
}

#[test]
fn size_bounds_read_back_with_the_language_separator() {
    assert_eq!(size_text(None, 2, Language::French), "");
    assert_eq!(size_text(Some(10 * 1024 * 1024), 2, Language::French), "10");
    assert_eq!(size_text(Some(1536 * 1024), 2, Language::French), "1,5");
    assert_eq!(size_text(Some(1536 * 1024), 2, Language::English), "1.5");
    assert_eq!(
        crate::ui::format::size_in_bytes(&size_text(Some(1536 * 1024), 2, Language::French), 2),
        Ok(Some(1536 * 1024))
    );
}
