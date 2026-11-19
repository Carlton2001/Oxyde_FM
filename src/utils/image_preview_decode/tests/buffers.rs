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

#[test]
fn a_damaged_animation_keeps_the_frames_read_before_the_damage() {
    let frame = || Ok(Frame::new(RgbaImage::from_pixel(4, 4, image::Rgba([1, 2, 3, 255]))));
    let damage = || Err(image::ImageError::IoError(std::io::Error::from(std::io::ErrorKind::UnexpectedEof)));
    let (frames, _) = collect_frames(vec![frame(), frame(), damage()].into_iter(), POLICY).unwrap();
    assert_eq!(frames.len(), 2);
    assert!(collect_frames(vec![damage()].into_iter(), POLICY).is_err());
}
