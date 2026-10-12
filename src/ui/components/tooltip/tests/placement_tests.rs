use super::preview_frame_origin;
use gpui::{point, px};

#[test]
fn preview_always_grows_above_fixed_metadata_origin() {
    let metadata_origin = point(px(107.0), px(510.0));
    for extra in [px(0.0), px(20.0), px(120.0), px(400.0)] {
        let frame_origin = preview_frame_origin(metadata_origin, extra);
        assert_eq!(frame_origin.x, metadata_origin.x);
        assert_eq!(frame_origin.y + extra, metadata_origin.y);
        assert!(frame_origin.y >= px(0.0));
    }
}
