use super::positions;

/// What `ShapedLine::x_for_index` does: the first glyph, in shaping order, at or past the index.
fn walk(glyphs: &[(usize, f32)], width: f32, index: usize) -> f32 {
    glyphs.iter().find(|(glyph, _)| *glyph >= index).map_or(width, |(_, x)| *x)
}

#[test]
fn the_table_answers_as_the_glyph_walk() {
    let lines: [&[(usize, f32)]; 4] = [
        &[],
        &[(0, 0.0), (1, 5.0), (3, 9.0), (4, 15.0)],
        // Right-to-left text shaped in visual order.
        &[(6, 0.0), (4, 4.0), (2, 8.0), (0, 12.0), (8, 16.0), (9, 20.0)],
        &[(0, 0.0), (2, 6.0), (1, 3.0), (5, 11.0)],
    ];
    for glyphs in lines {
        let x = positions(glyphs.to_vec(), 30.0);
        for index in 0..12 {
            assert_eq!(x(index), walk(glyphs, 30.0, index), "{glyphs:?} at {index}");
        }
    }
}
