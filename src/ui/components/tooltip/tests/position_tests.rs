use super::{anchored_origin, cursor_origin};
use gpui::{point, px, size, Bounds};

#[test]
fn ordinary_and_preview_metadata_follow_cursor_with_tauri_offset() {
    let content = size(px(200.0), px(100.0));
    let viewport = size(px(1200.0), px(900.0));
    for cursor in [point(px(100.0), px(120.0)), point(px(160.0), px(180.0))] {
        assert_eq!(
            cursor_origin(cursor, content, viewport, px(24.0), px(50.0)),
            cursor + point(px(24.0), px(24.0))
        );
    }
}

#[test]
fn right_and_bottom_edges_match_tauri_clamping() {
    assert_eq!(
        cursor_origin(
            point(px(780.0), px(580.0)),
            size(px(200.0), px(100.0)),
            size(px(800.0), px(600.0)),
            px(24.0),
            px(50.0)
        ),
        point(px(550.0), px(450.0))
    );
}

#[test]
fn small_viewport_relaxes_margin_to_keep_content_visible() {
    assert_eq!(
        cursor_origin(
            point(px(200.0), px(110.0)),
            size(px(200.0), px(100.0)),
            size(px(220.0), px(120.0)),
            px(24.0),
            px(50.0)
        ),
        point(px(20.0), px(20.0))
    );
}

#[test]
fn offset_scales_with_interface_dimensions() {
    let cursor = point(px(100.0), px(100.0));
    assert_eq!(
        cursor_origin(
            cursor,
            size(px(300.0), px(150.0)),
            size(px(1600.0), px(1200.0)),
            px(36.0),
            px(75.0)
        ),
        point(px(136.0), px(136.0))
    );
}

#[test]
fn anchored_tooltip_follows_target_bounds_without_mouse_coordinates() {
    let content = size(px(200.0), px(40.0));
    let viewport = size(px(800.0), px(600.0));
    let bounds = Bounds::new(point(px(100.0), px(10.0)), size(px(150.0), px(32.0)));
    assert_eq!(
        anchored_origin(bounds, content, viewport, px(4.0)),
        point(px(104.0), px(46.0))
    );
    let moved = Bounds::new(point(px(200.0), px(10.0)), bounds.size);
    assert_eq!(
        anchored_origin(moved, content, viewport, px(4.0)),
        point(px(204.0), px(46.0))
    );
}
