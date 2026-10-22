use super::{can_initialize_full_texture, has_full_texture_bytes};

#[test]
fn full_texture_initialization_matches_actual_atlas_extent() {
    assert!(can_initialize_full_texture(4000, 4000));
    assert!(can_initialize_full_texture(1024, 1024));
    assert!(can_initialize_full_texture(16384, 16384));
    // These requests are padded up to 1024 or clamped, so source bytes cannot
    // initialize their full texture. They must keep the subregion upload.
    for dimensions in [
        (128, 128),
        (4000, 512),
        (512, 4000),
        (16385, 4000),
        (0, 0),
        (-1, 4000),
    ] {
        assert!(!can_initialize_full_texture(dimensions.0, dimensions.1));
    }
}

#[test]
fn full_texture_initialization_rejects_truncated_pixel_buffers() {
    assert!(has_full_texture_bytes(4000, 4000, 4, 64_000_000));
    assert!(!has_full_texture_bytes(4000, 4000, 4, 63_999_999));
    assert!(has_full_texture_bytes(1024, 1024, 1, 1024 * 1024));
    assert!(!has_full_texture_bytes(-1, 4000, 4, usize::MAX));
}
