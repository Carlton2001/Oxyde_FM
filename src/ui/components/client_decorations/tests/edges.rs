use super::{margin, resize_edge, resize_zones};
use gpui::{point, px, size, ResizeEdge, Tiling};

const WINDOW: gpui::Size<gpui::Pixels> = size(px(800.0), px(600.0));

#[test]
fn margin_zones_resolve_edges_and_corners() {
    let free = Tiling::default();
    let at = |x: f32, y: f32| resize_edge(point(px(x), px(y)), WINDOW, free);
    assert_eq!(at(400.0, 2.0), Some(ResizeEdge::Top));
    assert_eq!(at(400.0, 595.0), Some(ResizeEdge::Bottom));
    assert_eq!(at(2.0, 300.0), Some(ResizeEdge::Left));
    assert_eq!(at(795.0, 300.0), Some(ResizeEdge::Right));
    assert_eq!(at(5.0, 5.0), Some(ResizeEdge::TopLeft));
    assert_eq!(at(795.0, 5.0), Some(ResizeEdge::TopRight));
    assert_eq!(at(5.0, 595.0), Some(ResizeEdge::BottomLeft));
    assert_eq!(at(795.0, 595.0), Some(ResizeEdge::BottomRight));
    // Corners run 24 px along both edges past their square.
    assert_eq!(at(35.0, 2.0), Some(ResizeEdge::TopLeft));
    assert_eq!(at(36.0, 2.0), Some(ResizeEdge::Top));
    assert_eq!(at(2.0, 35.0), Some(ResizeEdge::TopLeft));
    assert_eq!(at(2.0, 36.0), Some(ResizeEdge::Left));
    assert_eq!(at(764.0, 595.0), Some(ResizeEdge::BottomRight));
    assert_eq!(at(763.0, 595.0), Some(ResizeEdge::Bottom));
    assert_eq!(at(795.0, 564.0), Some(ResizeEdge::BottomRight));
    assert_eq!(at(795.0, 563.0), Some(ResizeEdge::Right));
    // The content starts right after the margin.
    assert_eq!(at(11.0, 11.0), Some(ResizeEdge::TopLeft));
    assert_eq!(at(12.0, 12.0), None);
    assert_eq!(at(787.0, 587.0), None);
    assert_eq!(at(788.0, 300.0), Some(ResizeEdge::Right));
    assert_eq!(at(400.0, 300.0), None);
}

#[test]
fn adjacent_zones_never_share_a_pixel() {
    let zones: Vec<_> = resize_zones(WINDOW, Tiling::default()).collect();
    for x in 0..800 {
        for y in [0, 11, 12, 300, 587, 588, 599] {
            let position = point(px(x as f32), px(y as f32));
            let hits = zones.iter().filter(|(_, zone)| {
                let end = zone.bottom_right();
                (zone.origin.x..end.x).contains(&position.x) && (zone.origin.y..end.y).contains(&position.y)
            }).count();
            assert!(hits <= 1, "{x}, {y}");
            let inside = (12..788).contains(&x) && (12..588).contains(&y);
            assert_eq!(hits == 1, !inside, "{x}, {y}");
        }
    }
}

#[test]
fn tiled_sides_have_no_margin_or_zone() {
    let tiling = Tiling { top: true, left: true, right: false, bottom: false };
    let edges = margin(tiling);
    assert_eq!((edges.top, edges.left), (px(0.0), px(0.0)));
    assert_eq!((edges.right, edges.bottom), (px(12.0), px(12.0)));
    let at = |x: f32, y: f32| resize_edge(point(px(x), px(y)), WINDOW, tiling);
    assert_eq!(at(400.0, 2.0), None);
    assert_eq!(at(2.0, 300.0), None);
    assert_eq!(at(2.0, 595.0), Some(ResizeEdge::Bottom));
    assert_eq!(at(795.0, 2.0), Some(ResizeEdge::Right));
    // A corner beside a tiled side leaves its edges to the free side.
    assert_eq!(at(2.0, 20.0), None);
    assert_eq!(at(20.0, 595.0), Some(ResizeEdge::Bottom));
    assert_eq!(at(795.0, 20.0), Some(ResizeEdge::Right));
    assert_eq!(resize_zones(WINDOW, Tiling::tiled()).count(), 0);
}

#[test]
fn corners_share_a_small_window_without_overlap() {
    let small = size(px(40.0), px(30.0));
    let zones: Vec<_> = resize_zones(small, Tiling::default()).collect();
    for x in 0..40 {
        for y in 0..30 {
            let position = point(px(x as f32), px(y as f32));
            let hits = zones.iter().filter(|(_, zone)| {
                let end = zone.bottom_right();
                (zone.origin.x..end.x).contains(&position.x) && (zone.origin.y..end.y).contains(&position.y)
            }).count();
            let inside = (12..28).contains(&x) && (12..18).contains(&y);
            assert_eq!(hits, usize::from(!inside), "{x}, {y}");
        }
    }
}
