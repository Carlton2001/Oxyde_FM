#[test]
fn capacity_conversion_rejects_invalid_input_and_percent_overflow() {
    assert_eq!(super::capacity_mb("25", true, 400 * 1024 * 1024), Some(100));
    assert_eq!(super::capacity_mb("101", true, 400 * 1024 * 1024), None);
    assert_eq!(super::capacity_mb("-1", false, 0), None);
    assert_eq!(super::capacity_mb("4294967296", false, 0), None);
    assert_eq!(super::capacity_mb("123", false, 0), Some(123));
}

#[test]
fn displayed_percent_preserves_entered_percent_after_megabyte_storage() {
    let total_bytes = 499_107_856_384;
    for percent in 0..=100 {
        let stored_mb = super::capacity_mb(&percent.to_string(), true, total_bytes).unwrap();
        assert_eq!(super::capacity_percent(stored_mb, total_bytes), percent);
    }
}

#[test]
fn displayed_percent_rounds_to_nearest_with_halves_up() {
    let total_bytes = 200 * 1024 * 1024;
    assert_eq!(super::capacity_percent(0, total_bytes), 0);
    assert_eq!(super::capacity_percent(1, total_bytes), 1);
    assert_eq!(super::capacity_percent(2, total_bytes), 1);
    assert_eq!(super::capacity_percent(3, total_bytes), 2);
    assert_eq!(super::capacity_percent(200, total_bytes), 100);
    assert_eq!(super::capacity_percent(123, 0), 0);
    assert_eq!(super::capacity_percent(u32::MAX as u64, u64::MAX), 0);
}
