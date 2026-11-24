use super::{parse_day, size_in_bytes, DateFormat};

#[test]
fn a_typed_size_takes_digits_only() {
    assert!(size_in_bytes("+1", 1).is_err());
    assert!(size_in_bytes("1.+5", 1).is_err());
    assert_eq!(size_in_bytes("2", 1), Ok(Some(2048)));
}

#[test]
fn a_typed_day_needs_its_whole_year() {
    assert!(parse_day("01/02/26", DateFormat::European).is_none());
    assert!(parse_day("26-02-01", DateFormat::Iso).is_none());
    assert_eq!(
        parse_day("01/02/2026", DateFormat::European),
        chrono::NaiveDate::from_ymd_opt(2026, 2, 1)
    );
}
