use gpui::{size, DevicePixels, SvgRenderer, SvgSize};
use std::sync::Arc;

#[test]
fn svg_grid_has_transparent_cells_and_fades_downwards() {
    let renderer = SvgRenderer::new(Arc::new(()));
    let svg = renderer
        .parse_svg(include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/assets/visuals/operation-grid.svg"
        )))
        .unwrap();
    let image = renderer
        .render_parsed(
            &svg,
            SvgSize::ExactSize(size(DevicePixels(800), DevicePixels(80))),
        )
        .unwrap();
    let bytes = image.as_bytes(0).unwrap();
    let alpha = |x: usize, y: usize| bytes[(y * 800 + x) * 4 + 3];
    assert_eq!(
        alpha(20, 8),
        0,
        "grid cells must keep a transparent background"
    );
    assert!(alpha(40, 8) > 0, "vertical grid lines must render");
    assert!(
        alpha(40, 8) > alpha(40, 72) * 3,
        "the SVG gradient must fade toward the bottom"
    );
}
