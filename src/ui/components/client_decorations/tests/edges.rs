use super::{input_region, margin, resize_edge, resize_zones};
use gpui::{point, px, size, Bounds, ResizeEdge, Tiling};

const WINDOW: gpui::Size<gpui::Pixels> = size(px(800.0), px(600.0));

#[test]
fn band_zones_resolve_edges_and_corners() {
    let free = Tiling::default();
    // The 12 px band runs from 14 to 26 px; the shadow outside it takes no resize.
    let at = |x: f32, y: f32| resize_edge(point(px(x), px(y)), WINDOW, free);
    assert_eq!(at(400.0, 13.0), None);
    assert_eq!(at(400.0, 16.0), Some(ResizeEdge::Top));
    assert_eq!(at(400.0, 581.0), Some(ResizeEdge::Bottom));
    assert_eq!(at(400.0, 586.0), None);
    assert_eq!(at(16.0, 300.0), Some(ResizeEdge::Left));
    assert_eq!(at(781.0, 300.0), Some(ResizeEdge::Right));
    assert_eq!(at(19.0, 19.0), Some(ResizeEdge::TopLeft));
    assert_eq!(at(781.0, 19.0), Some(ResizeEdge::TopRight));
    assert_eq!(at(19.0, 581.0), Some(ResizeEdge::BottomLeft));
    assert_eq!(at(781.0, 581.0), Some(ResizeEdge::BottomRight));
    // Corners run 24 px along both edges past their square.
    assert_eq!(at(49.0, 16.0), Some(ResizeEdge::TopLeft));
    assert_eq!(at(50.0, 16.0), Some(ResizeEdge::Top));
    assert_eq!(at(16.0, 49.0), Some(ResizeEdge::TopLeft));
    assert_eq!(at(16.0, 50.0), Some(ResizeEdge::Left));
    assert_eq!(at(750.0, 581.0), Some(ResizeEdge::BottomRight));
    assert_eq!(at(749.0, 581.0), Some(ResizeEdge::Bottom));
    // The content starts right after the margin.
    assert_eq!(at(25.0, 25.0), Some(ResizeEdge::TopLeft));
    assert_eq!(at(26.0, 26.0), None);
    assert_eq!(at(773.0, 573.0), None);
    assert_eq!(at(774.0, 300.0), Some(ResizeEdge::Right));
    assert_eq!(at(400.0, 300.0), None);
}

#[test]
fn adjacent_zones_never_share_a_pixel() {
    let zones: Vec<_> = resize_zones(WINDOW, Tiling::default()).collect();
    for x in 0..800 {
        for y in [0, 13, 14, 25, 26, 300, 573, 574, 585, 586, 599] {
            let position = point(px(x as f32), px(y as f32));
            let hits = zones.iter().filter(|(_, zone)| {
                let end = zone.bottom_right();
                (zone.origin.x..end.x).contains(&position.x) && (zone.origin.y..end.y).contains(&position.y)
            }).count();
            assert!(hits <= 1, "{x}, {y}");
            let in_band = (14..786).contains(&x) && (14..586).contains(&y);
            let inside = (26..774).contains(&x) && (26..574).contains(&y);
            assert_eq!(hits == 1, in_band && !inside, "{x}, {y}");
        }
    }
}

#[test]
fn tiled_sides_have_no_margin_or_zone() {
    let tiling = Tiling { top: true, left: true, right: false, bottom: false };
    let edges = margin(tiling);
    assert_eq!((edges.top, edges.left), (px(0.0), px(0.0)));
    assert_eq!((edges.right, edges.bottom), (px(26.0), px(26.0)));
    let at = |x: f32, y: f32| resize_edge(point(px(x), px(y)), WINDOW, tiling);
    assert_eq!(at(400.0, 2.0), None);
    assert_eq!(at(2.0, 300.0), None);
    assert_eq!(at(2.0, 581.0), Some(ResizeEdge::Bottom));
    assert_eq!(at(781.0, 2.0), Some(ResizeEdge::Right));
    // A corner beside a tiled side leaves its edges to the free side.
    assert_eq!(at(2.0, 20.0), None);
    assert_eq!(at(20.0, 581.0), Some(ResizeEdge::Bottom));
    assert_eq!(at(781.0, 20.0), Some(ResizeEdge::Right));
    assert_eq!(resize_zones(WINDOW, Tiling::tiled()).count(), 0);
}

#[test]
fn corners_share_a_small_window_without_overlap() {
    let small = size(px(68.0), px(58.0));
    let zones: Vec<_> = resize_zones(small, Tiling::default()).collect();
    for x in 14..54 {
        for y in 14..44 {
            let position = point(px(x as f32), px(y as f32));
            let hits = zones.iter().filter(|(_, zone)| {
                let end = zone.bottom_right();
                (zone.origin.x..end.x).contains(&position.x) && (zone.origin.y..end.y).contains(&position.y)
            }).count();
            let inside = (26..42).contains(&x) && (26..32).contains(&y);
            assert_eq!(hits, usize::from(!inside), "{x}, {y}");
        }
    }
}

#[test]
fn input_region_covers_frame_and_band_only() {
    let content = Bounds::new(point(px(26.0), px(26.0)), size(px(748.0), px(548.0)));
    let free = Tiling::default();
    assert_eq!(input_region(content, free, false), content);
    assert_eq!(input_region(content, free, true), Bounds::new(point(px(14.0), px(14.0)), size(px(772.0), px(572.0))));
    let tiled = Tiling { top: true, left: true, right: false, bottom: false };
    let flush = Bounds::new(point(px(0.0), px(0.0)), size(px(774.0), px(574.0)));
    assert_eq!(input_region(flush, tiled, true), Bounds::new(point(px(0.0), px(0.0)), size(px(786.0), px(586.0))));
}
