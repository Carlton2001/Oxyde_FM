use super::menu_position;
use gpui::{point, px};

#[test]
fn pinned_menu_keeps_measured_button_origin_and_scaled_rail_gap() {
    for rem in [12.0, 16.0, 24.0] {
        let origin = point(px(8.0), px(73.0));
        let position = menu_position(origin, px(rem));
        assert_eq!(position.y, origin.y);
        assert_eq!(position.x, px(8.0 + 2.5 * rem));
        // The 2 rem button is followed by the original half-rem gap.
        assert_eq!(position.x - (origin.x + px(2.0 * rem)), px(0.5 * rem));
    }
}
