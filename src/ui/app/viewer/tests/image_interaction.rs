use super::anchored_offset;
use gpui::{point, px};

#[test]
fn zoom_keeps_the_pixel_under_the_pointer_fixed() {
    let base_center = point(px(300.0), px(200.0));
    let offset = point(px(40.0), px(-20.0));
    let cursor = point(px(180.0), px(120.0));
    let center = base_center + offset;
    for ratio in [0.5, 1.0, 2.0, 4.0] {
        let next = anchored_offset(offset, cursor, center, ratio);
        let pixel_after = base_center + next + (cursor - center) * ratio;
        assert_eq!(pixel_after, cursor);
    }
}
