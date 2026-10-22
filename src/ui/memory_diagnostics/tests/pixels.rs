use super::*;

fn image() -> Arc<RenderImage> {
    Arc::new(RenderImage::new(vec![image::Frame::new(
        image::RgbaImage::new(4, 3),
    )]))
}

#[test]
fn tracks_unique_pixels_without_keeping_them_alive() {
    let mut registry = Registry::default();
    let image = image();
    registry.track(&image, State::Decoded);
    registry.track(&image.clone(), State::Shown);
    assert_eq!(registry.snapshot().live_images, 1);
    assert_eq!(registry.snapshot().live_bytes, 48);
    registry.track(&image, State::Retired);
    let frame = image.clone();
    drop(image);
    assert_eq!(registry.snapshot().retired_bytes, 48);
    drop(frame);
    assert_eq!(registry.snapshot(), Pixels::default());
}

#[test]
fn counts_animation_frames_and_dropped_unpublished_results() {
    let mut registry = Registry::default();
    let image = Arc::new(RenderImage::new(vec![
        image::Frame::new(image::RgbaImage::new(4, 3)),
        image::Frame::new(image::RgbaImage::new(2, 2)),
    ]));
    registry.track(&image, State::Decoded);
    assert_eq!(registry.snapshot().pending_bytes, 64);
    drop(image); // Obsolete asynchronous decode, never shown.
    assert_eq!(registry.snapshot(), Pixels::default());
}

#[test]
fn reports_tracking_saturation_instead_of_claiming_complete_coverage() {
    let mut registry = Registry::default();
    let images: Vec<_> = (0..MAX_TRACKED + 1).map(|_| image()).collect();
    for image in &images { registry.track(image, State::Shown); }
    assert_eq!(registry.snapshot().live_images, MAX_TRACKED);
    assert_eq!(registry.snapshot().untracked_registrations, 1);
    drop(images);
    assert_eq!(registry.snapshot().live_bytes, 0);
}
