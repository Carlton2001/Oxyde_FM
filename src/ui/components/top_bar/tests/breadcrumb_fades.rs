use super::edge_fades;
use gpui::{point, px, size, Bounds};

fn fades(viewport_width: f32, path_width: f32, offset: f32, max: f32) -> (bool, bool) {
    // Use a nonzero origin, as the breadcrumb follows the navigation buttons.
    let viewport = Bounds::new(
        point(px(200.0), px(50.0)),
        size(px(viewport_width), px(40.0)),
    );
    let last = Bounds::new(
        point(px(200.0 + path_width - 60.0), px(50.0)),
        size(px(60.0), px(40.0)),
    );
    edge_fades(viewport, last, px(offset), px(max), px(2.0))
}

#[test]
fn fitting_path_has_no_fades_even_with_unclamped_wheel_offsets() {
    for offset in [-120.0, 0.0, 120.0] {
        assert_eq!(fades(800.0, 700.0, offset, 0.0), (false, false));
    }
}

#[test]
fn hidden_editable_space_does_not_fade_the_last_segment() {
    // Only the trailing 24px blank area overflows.
    assert_eq!(fades(700.0, 700.0, 0.0, 24.0), (false, false));
    // Resizing reveals the final segment without scrolling the blank into view.
    assert_eq!(fades(700.0, 900.0, -200.0, 224.0), (true, false));
    assert_eq!(fades(700.0, 900.0, -224.0, 224.0), (true, false));
}

#[test]
fn scrolling_away_from_the_end_fades_only_clipped_edges() {
    assert_eq!(fades(700.0, 900.0, 0.0, 224.0), (false, true));
    assert_eq!(fades(700.0, 900.0, -100.0, 224.0), (true, true));
    assert_eq!(fades(700.0, 900.0, -199.0, 224.0), (true, false));
}
