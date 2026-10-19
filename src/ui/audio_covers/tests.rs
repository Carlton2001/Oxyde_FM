use super::Pool;
use gpui::RenderImage;
use std::sync::Arc;

fn image(width: u32, height: u32, color: [u8; 4]) -> Arc<RenderImage> {
    Arc::new(RenderImage::new(vec![image::Frame::new(
        image::RgbaImage::from_pixel(width, height, image::Rgba(color)),
    )]))
}

#[test]
fn grid_cache_reuses_disk_pixels_beyond_32_cards_and_checks_revisions() {
    let root = tempfile::tempdir().unwrap();
    let mut first = None;
    for index in 0..64 {
        let path = root.path().join(format!("{index}.mp3"));
        std::fs::write(&path, b"audio revision").unwrap();
        let path = path.to_str().unwrap();
        let pixels = super::load_grid(path, None, || Ok(image(128, 64, [20, 40, 80, 255]))).unwrap();
        if index == 0 { first = Some((path.to_owned(), pixels)); }
    }
    let (path, original) = first.unwrap();
    let stamp = oxyde_lib::models::FileStamp::from_metadata(&std::fs::metadata(&path).unwrap()).unwrap();
    let cached = super::load_grid(&path, Some(&stamp), || panic!("Must not reload the disk thumbnail")).unwrap();
    assert!(Arc::ptr_eq(&original, &cached));
    std::fs::write(&path, b"different revision with new cover").unwrap();
    assert!(super::load_grid(&path, Some(&stamp), || panic!("Stale listing must be rejected")).is_err());
    let updated = super::load_grid(&path, None, || Ok(image(128, 64, [80, 40, 20, 255]))).unwrap();
    assert!(!Arc::ptr_eq(&original, &updated));
    super::invalidate(&path);
    let invalidated = super::load_grid(&path, None, || Err("Reload required".to_owned()));
    assert_eq!(invalidated.unwrap_err(), "Reload required");
}

#[test]
fn hover_cache_reports_valid_hits_and_keeps_its_quota_separate_from_grid() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("hover.mp3");
    std::fs::write(&path, b"first revision").unwrap();
    let path = path.to_str().unwrap();
    assert!(!super::may_have_cached(path));
    let (original, cached) = super::with_cache(path, false, None, || Ok(image(512, 256, [1, 2, 3, 255]))).unwrap();
    assert!(!cached);
    assert!(super::may_have_cached(path));
    for index in 0..64 {
        let grid = root.path().join(format!("grid-{index}.mp3"));
        std::fs::write(&grid, b"source").unwrap();
        super::load_grid(grid.to_str().unwrap(), None, || Ok(image(128, 64, [1, 2, 3, 255]))).unwrap();
    }
    let (hit, cached) = super::with_cache(path, false, None, || panic!("Hover must not extract the cover again")).unwrap();
    assert!(cached);
    assert!(Arc::ptr_eq(&original, &hit));
    std::fs::write(path, b"changed revision").unwrap();
    let (_, cached) = super::with_cache(path, false, None, || Ok(image(512, 256, [3, 2, 1, 255]))).unwrap();
    assert!(!cached, "Changed sources must not bypass the fresh-preview delay");
    super::invalidate(path);
    assert!(!super::may_have_cached(path));
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
