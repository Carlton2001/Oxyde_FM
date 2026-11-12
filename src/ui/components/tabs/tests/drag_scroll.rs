use super::drag_depth;
use crate::ui::scale::logical;

#[test]
fn only_the_bands_at_the_ends_scroll_and_their_speed_is_capped() {
    let (left, right) = (100.0, 600.0);
    assert_eq!(drag_depth(300.0, left, right), 0.0);
    assert!(drag_depth(left + 1.0, left, right) < 0.0);
    assert!(drag_depth(right - 1.0, left, right) > 0.0);
    assert_eq!(drag_depth(left - 500.0, left, right), -logical(super::DRAG_MAX_DEPTH));
    assert_eq!(drag_depth(right + 500.0, left, right), logical(super::DRAG_MAX_DEPTH));
}
