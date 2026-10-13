use super::menu_position;
use gpui::{point, px, size};

#[test]
fn text_menu_stays_inside_a_small_properties_window() {
    let viewport = size(px(448.0), px(300.0));
    let menu = size(px(208.0), px(120.0));
    assert_eq!(
        menu_position(point(px(430.0), px(280.0)), menu, viewport),
        point(px(240.0), px(180.0))
    );
    assert_eq!(
        menu_position(point(px(30.0), px(40.0)), menu, viewport),
        point(px(30.0), px(40.0))
    );
}

#[test]
fn oversized_menu_and_negative_click_position_do_not_create_negative_bounds() {
    assert_eq!(
        menu_position(
            point(px(-5.0), px(100.0)),
            size(px(500.0), px(400.0)),
            size(px(200.0), px(150.0))
        ),
        point(px(0.0), px(0.0))
    );
}
