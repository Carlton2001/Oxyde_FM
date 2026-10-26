use super::{format_percent, parse_days, parse_percent};

#[test]
fn retention_days_are_positive_and_bounded() {
    assert_eq!(parse_days(" 30 "), Some(30));
    assert_eq!(parse_days("0"), None);
    assert_eq!(parse_days("-3"), None);
    assert_eq!(parse_days("36501"), None);
    assert_eq!(parse_days("7 jours"), None);
}

#[test]
fn size_limits_accept_decimal_commas_and_stay_a_share_of_the_disk() {
    assert_eq!(parse_percent("10"), Some(10.0));
    assert_eq!(parse_percent("5,5"), Some(5.5));
    assert_eq!(parse_percent("0"), None);
    assert_eq!(parse_percent("101"), None);
    assert_eq!(format_percent(10.0), "10");
    assert_eq!(format_percent(5.5), "5.5");
}
