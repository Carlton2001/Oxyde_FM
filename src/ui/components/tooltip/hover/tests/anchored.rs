use super::*;

#[::core::prelude::v1::test]
fn a_following_surface_overlaps_the_pointer_by_its_margins() {
    let pointer = point(px(499.0), px(399.0));
    assert_eq!(rect(None, pointer), Bounds::new(pointer, size(px(1.0), px(1.0))));
}

#[::core::prelude::v1::test]
fn a_fixed_surface_keeps_its_element_as_anchor() {
    let element = Bounds::new(point(px(10.0), px(20.0)), size(px(30.0), px(40.0)));
    assert_eq!(rect(Some(element), point(px(1.0), px(1.0))), element);
}
