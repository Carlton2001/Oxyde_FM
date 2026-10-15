use super::menu_position;
use gpui::{point, px};

#[test]
fn pinned_menu_aligns_with_outer_rail_edge_at_every_scale() {
    for rem in [12.0, 16.0, 24.0] {
        let origin = point(px(8.0), px(73.0));
        let position = menu_position(origin, px(rem));
        assert_eq!(position.y, origin.y);
        assert_eq!(position.x, px(8.0 + 2.5 * rem));
        assert_eq!(position.x - (origin.x + px(2.0 * rem)), px(0.5 * rem));
    }
}
