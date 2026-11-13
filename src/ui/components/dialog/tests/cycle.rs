use super::next_stop;

#[test]
fn tab_wraps_both_ways() {
    assert_eq!(next_stop(Some(2), 3, false), Some(0));
    assert_eq!(next_stop(Some(0), 3, true), Some(2));
    assert_eq!(next_stop(Some(1), 3, false), Some(2));
}

#[test]
fn from_outside_the_order_tab_starts_at_an_end() {
    assert_eq!(next_stop(None, 3, false), Some(0));
    assert_eq!(next_stop(None, 3, true), Some(2));
    assert_eq!(next_stop(None, 0, false), None);
}
