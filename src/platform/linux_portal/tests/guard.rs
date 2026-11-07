use super::*;

#[test]
fn failed_starts_count_for_their_version_only() {
    assert_eq!(failed_starts("2.1.0 2\n", "2.1.0"), 2);
    assert_eq!(failed_starts("2.1.0 2\n", "2.2.0"), 0);
    assert_eq!(failed_starts("", "2.1.0"), 0);
    assert_eq!(failed_starts("2.1.0 many", "2.1.0"), 0);
}

#[test]
fn two_failed_starts_trip_the_guard() {
    assert!(failed_starts("2.1.0 1", "2.1.0") < LIMIT);
    assert!(failed_starts("2.1.0 2", "2.1.0") >= LIMIT);
}
