use super::*;

#[test]
fn fitting_an_rgba_preview_reuses_its_allocation_and_converts_to_bgra() {
    let pixels = RgbaImage::from_pixel(512, 256, image::Rgba([10, 20, 30, 128]));
    let allocation = pixels.as_raw().as_ptr();
    let fitted = fitted_bgra(DynamicImage::ImageRgba8(pixels), POLICY);
    assert_eq!(fitted.as_raw().as_ptr(), allocation);
    assert_eq!(fitted.dimensions(), (512, 256));
    assert_eq!(fitted.get_pixel(0, 0).0, [30, 20, 10, 128]);
}

#[test]
fn reducing_a_large_source_keeps_only_a_new_small_pixel_buffer() {
    let pixels = RgbaImage::from_pixel(2048, 1024, image::Rgba([10, 20, 30, 128]));
    let allocation = pixels.as_raw().as_ptr();
    let fitted = fitted_bgra(DynamicImage::ImageRgba8(pixels), POLICY);
    assert_ne!(fitted.as_raw().as_ptr(), allocation);
    assert_eq!(fitted.dimensions(), (1024, 512));
    assert_eq!(fitted.as_raw().len(), 2 * 1024 * 1024);
    assert_eq!(fitted.get_pixel(512, 256).0, [30, 20, 10, 128]);
}
