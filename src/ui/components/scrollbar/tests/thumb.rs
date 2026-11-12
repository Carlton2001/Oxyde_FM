use super::{drag_scrolled, Thumb};
use crate::ui::scale::set_reference;

#[test]
fn thumb_is_proportional_with_a_minimum_and_never_longer_than_the_rail() {
    set_reference(16);
    assert_eq!(Thumb::new(200.0, 800.0, 200.0, None).size, 50.0);
    assert_eq!(Thumb::new(200.0, 100_000.0, 200.0, None).size, 40.0);
    assert_eq!(Thumb::new(20.0, 100.0, 20.0, None).size, 20.0);
    assert_eq!(Thumb::new(200.0, 800.0, 200.0, Some(10.0)).size, 50.0);
    // A select menu's rail runs past its viewport.
    assert_eq!(Thumb::new(200.0, 800.0, 216.0, None).size, 54.0);
}

#[test]
fn position_and_drag_are_inverse_and_reach_both_ends_at_each_scale() {
    for reference in [10, 16, 24, 32] {
        set_reference(reference);
        let factor = reference as f32 / 16.0;
        let (viewport, content, grab) = (200.0 * factor, 800.0 * factor, 10.0 * factor);
        let thumb = Thumb::new(viewport, content, viewport, None);
        assert_eq!(thumb.position(0.0), 0.0);
        assert_eq!(thumb.position(600.0 * factor), thumb.travel);
        assert_eq!(thumb.scrolled_at(thumb.position(300.0 * factor)), 300.0 * factor);
        assert_eq!(drag_scrolled(viewport, content, viewport, -100.0, grab), 0.0);
        assert_eq!(drag_scrolled(viewport, content, viewport, grab, grab), 0.0);
        assert_eq!(drag_scrolled(viewport, content, viewport, 160.0 * factor, grab), 600.0 * factor);
        assert_eq!(drag_scrolled(viewport, content, viewport, 1000.0 * factor, grab), 600.0 * factor);
    }
    set_reference(16);
}

#[test]
fn content_that_fits_never_scrolls() {
    set_reference(16);
    assert_eq!(drag_scrolled(200.0, 100.0, 200.0, 150.0, 0.0), 0.0);
    assert_eq!(Thumb::new(200.0, 0.0, 200.0, None).position(50.0), 0.0);
}
