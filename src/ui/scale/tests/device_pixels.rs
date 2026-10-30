use super::device_snapped;

/// GPUI's `round_to_device_pixel`, which lays out each authored length.
fn laid_out(value: f32, scale: f32) -> f32 {
    (value * scale - 0.5).ceil()
}

#[test]
fn menu_viewport_contains_its_rows_at_every_zoom_and_display_scale() {
    for scale in [1.0, 1.25, 1.5, 1.75, 2.0, 2.25] {
        for rem in 10..=32 {
            let unit = rem as f32 / 16.0;
            let row = device_snapped(26.0 * unit, scale);
            let separator = device_snapped(9.0 * unit, scale);
            for (rows, separators) in [(1.0, 0.0), (8.0, 4.0), (40.0, 12.0)] {
                let viewport = rows * row + separators * separator;
                let content = rows * laid_out(row, scale) + separators * laid_out(separator, scale);
                assert_eq!(laid_out(viewport, scale), content, "rem {rem} at scale {scale}");
            }
        }
    }
}

#[test]
fn the_pane_menu_overflowed_without_snapping_at_125_percent() {
    let (row, separator, scale) = (32.5, 11.25, 1.25);
    let content = 8.0 * laid_out(row, scale) + 4.0 * laid_out(separator, scale);
    assert_eq!(content, 384.0);
    assert_eq!(laid_out(8.0 * row + 4.0 * separator, scale), 381.0);
}
