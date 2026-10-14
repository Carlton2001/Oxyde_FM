use super::{constrain_offset, modal_viewport};
use gpui::{point, px, size};

#[test]
fn dragging_stops_before_the_status_bar_at_each_scale() {
    for scale in [0.75, 1.0, 1.5, 2.0] {
        let window = size(px(1000.0), px(800.0));
        let status_and_margin = px(48.0 * scale);
        let available = modal_viewport(window, status_and_margin);
        let modal = size(px(480.0), px(360.0));
        let offset = constrain_offset(point(px(0.0), px(1000.0)), modal, available);
        let bottom = (available.height + modal.height) / 2.0 + offset.y;
        assert_eq!(bottom, window.height - status_and_margin);
    }
}

#[test]
fn small_windows_never_produce_a_negative_available_height() {
    assert_eq!(
        modal_viewport(size(px(200.0), px(24.0)), px(48.0)),
        size(px(200.0), px(0.0)),
    );
}

#[test]
fn dragging_keeps_every_edge_inside_the_viewport() {
    let modal = size(px(480.0), px(360.0));
    let viewport = size(px(1000.0), px(800.0));
    assert_eq!(
        constrain_offset(point(px(900.0), px(-900.0)), modal, viewport),
        point(px(260.0), px(-220.0))
    );
    assert_eq!(
        constrain_offset(point(px(-900.0), px(900.0)), modal, viewport),
        point(px(-260.0), px(220.0))
    );
    assert_eq!(
        constrain_offset(point(px(50.0), px(60.0)), modal, viewport),
        point(px(50.0), px(60.0))
    );
}

#[test]
fn resizing_or_growing_a_modal_reclamps_its_position() {
    assert_eq!(
        constrain_offset(
            point(px(200.0), px(200.0)),
            size(px(480.0), px(500.0)),
            size(px(600.0), px(600.0))
        ),
        point(px(60.0), px(50.0))
    );
    assert_eq!(
        constrain_offset(
            point(px(200.0), px(-200.0)),
            size(px(700.0), px(700.0)),
            size(px(600.0), px(600.0))
        ),
        point(px(0.0), px(0.0))
    );
}
