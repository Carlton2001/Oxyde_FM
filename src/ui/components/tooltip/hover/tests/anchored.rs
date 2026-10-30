use super::*;

const INSET: Pixels = px(32.0);

#[::core::prelude::v1::test]
fn a_following_surface_surrounds_the_pointer_by_margins_less_the_gap() {
    let (rect, ..) = placement(None, point(px(500.0), px(400.0)), INSET);
    assert_eq!(rect, Bounds::from_corners(point(px(480.0), px(380.0)), point(px(520.0), px(420.0))));
}

#[::core::prelude::v1::test]
fn a_fixed_surface_starts_at_its_element() {
    let element = Bounds::new(point(px(10.0), px(20.0)), size(px(30.0), px(40.0)));
    let (rect, _, _, offset) = placement(Some(element), point(px(1.0), px(1.0)), INSET);
    assert_eq!((rect.left(), rect.right(), offset.x), (px(10.0), px(40.0), -INSET));
}
