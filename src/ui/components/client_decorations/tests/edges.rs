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
    // The content starts right after the margin.
    assert_eq!(at(10.0, 10.0), None);
    assert_eq!(at(789.0, 589.0), None);
    assert_eq!(at(400.0, 300.0), None);
}

#[test]
fn adjacent_zones_never_share_a_pixel() {
    let zones: Vec<_> = resize_zones(WINDOW, Tiling::default()).collect();
    for x in 0..800 {
        for y in [0, 9, 10, 300, 589, 590, 599] {
            let position = point(px(x as f32), px(y as f32));
            let hits = zones.iter().filter(|(_, zone)| {
                let end = zone.bottom_right();
                (zone.origin.x..end.x).contains(&position.x) && (zone.origin.y..end.y).contains(&position.y)
            }).count();
            assert!(hits <= 1, "{x}, {y}");
        }
    }
}

#[test]
fn tiled_sides_have_no_margin_or_zone() {
    let tiling = Tiling { top: true, left: true, right: false, bottom: false };
    let edges = margin(tiling);
    assert_eq!((edges.top, edges.left), (px(0.0), px(0.0)));
    assert_eq!((edges.right, edges.bottom), (px(10.0), px(10.0)));
    let at = |x: f32, y: f32| resize_edge(point(px(x), px(y)), WINDOW, tiling);
    assert_eq!(at(400.0, 2.0), None);
    assert_eq!(at(2.0, 300.0), None);
    assert_eq!(at(2.0, 595.0), Some(ResizeEdge::Bottom));
    assert_eq!(at(795.0, 2.0), Some(ResizeEdge::Right));
    assert_eq!(resize_zones(WINDOW, Tiling::tiled()).count(), 0);
}
