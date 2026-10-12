#[test]
fn capacity_conversion_rejects_invalid_input_and_percent_overflow() {
    assert_eq!(super::capacity_mb("25", true, 400 * 1024 * 1024), Some(100));
    assert_eq!(super::capacity_mb("101", true, 400 * 1024 * 1024), None);
    assert_eq!(super::capacity_mb("-1", false, 0), None);
    assert_eq!(super::capacity_mb("4294967296", false, 0), None);
    assert_eq!(super::capacity_mb("123", false, 0), Some(123));
}
