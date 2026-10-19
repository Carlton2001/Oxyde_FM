use super::Pool;
use gpui::RenderImage;
use std::sync::Arc;

fn image(width: u32, height: u32, color: [u8; 4]) -> Arc<RenderImage> {
    Arc::new(RenderImage::new(vec![image::Frame::new(
        image::RgbaImage::from_pixel(width, height, image::Rgba(color)),
    )]))
}

#[test]
fn identical_covers_share_pixels_but_different_colors_or_dimensions_do_not() {
    let mut pool = Pool::default();
    let first = pool.share(image(2, 1, [10, 20, 30, 255]));
    let duplicate = pool.share(image(2, 1, [10, 20, 30, 255]));
    assert!(Arc::ptr_eq(&first, &duplicate));
    assert!(!Arc::ptr_eq(
        &first,
        &pool.share(image(2, 1, [30, 20, 10, 255]))
    ));
    assert!(!Arc::ptr_eq(
        &first,
        &pool.share(image(1, 2, [10, 20, 30, 255]))
    ));
    let weak = Arc::downgrade(&first);
    drop(first);
    drop(duplicate);
    assert!(
        weak.upgrade().is_none(),
        "The content pool must not own images"
    );
}
