use super::*;

const INSET: Pixels = px(32.0);

fn anchor(height: f32) -> PopupAnchor {
    PopupAnchor::element(Bounds::new(point(px(10.0), px(20.0)), size(px(30.0), px(height))))
}

/// Where the surface's near edge lands on each side, as Mutter computes it: the flip mirrors the
/// anchor and gravity, not the offset.
fn sides(axis: Axis) -> (Pixels, Pixels) {
    let (opened, flipped) = if axis.at_end { (axis.end, axis.start) } else { (axis.start, axis.end) };
    (opened + axis.offset, flipped + axis.offset)
}

#[::core::prelude::v1::test]
fn a_flipped_frame_keeps_its_gap_around_a_point() {
    let axis = flip_axis(px(100.0), px(100.0), px(12.0), INSET, true);
    assert_eq!(sides(axis), (px(80.0), px(120.0)));
}

#[::core::prelude::v1::test]
fn a_flipped_frame_keeps_its_gap_around_an_element() {
    for height in [10.0, 200.0] {
        let end = px(100.0 + height);
        assert_eq!(sides(flip_axis(px(100.0), end, px(4.0), INSET, true)), (end - px(28.0), px(128.0)));
        assert_eq!(sides(flip_axis(px(100.0), end, px(4.0), INSET, false)), (px(128.0), end - px(28.0)));
    }
}

#[::core::prelude::v1::test]
fn a_change_of_side_or_form_needs_another_surface() {
    assert!(same_placement(&anchor(10.0), &anchor(40.0), INSET));
    assert!(!same_placement(&anchor(10.0), &anchor(200.0), INSET));
    assert!(!same_placement(&anchor(10.0), &anchor(10.0).above(true), INSET));
    assert!(!same_placement(&PopupAnchor::beside(anchor(10.0).bounds, false), &anchor(10.0), INSET));
}
