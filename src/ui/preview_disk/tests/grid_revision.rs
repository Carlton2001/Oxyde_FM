use super::*;

fn pixels(color: [u8; 4]) -> RgbaImage {
    RgbaImage::from_pixel(2, 1, image::Rgba(color))
}

#[test]
fn stale_listing_is_rejected_before_decoding_and_a_new_revision_has_new_pixels() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("image.png");
    std::fs::write(&path, b"original").unwrap();
    let old = read_stamp(&path).unwrap();
    let before = checked_grid_image(&path, Some(&old), || Ok(pixels([1, 2, 3, 255]))).unwrap();
    std::fs::write(&path, b"replacement with a different size").unwrap();
    assert!(
        checked_grid_image(&path, Some(&old), || panic!("stale request decoded pixels")).is_err()
    );
    let fresh = read_stamp(&path).unwrap();
    let after = checked_grid_image(&path, Some(&fresh), || Ok(pixels([4, 5, 6, 255]))).unwrap();
    assert_ne!(before.as_bytes(0), after.as_bytes(0));
    assert_eq!(after.as_bytes(0).unwrap(), &[4, 5, 6, 255, 4, 5, 6, 255]);
}

#[test]
fn a_late_decode_cannot_publish_after_modification_or_deletion() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("image.png");
    for remove in [false, true] {
        std::fs::write(&path, b"original").unwrap();
        let stamp = read_stamp(&path).unwrap();
        assert!(checked_grid_image(&path, Some(&stamp), || {
            if remove {
                std::fs::remove_file(&path).unwrap();
            } else {
                std::fs::write(&path, b"changed during worker decode").unwrap();
            }
            Ok(pixels([9, 8, 7, 255]))
        })
        .is_err());
    }
}

#[test]
fn a_failed_or_deleted_source_can_be_recreated_and_grid_pixels_remain_bounded() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("image.png");
    assert!(checked_grid_image(&path, None, || panic!("missing source decoded")).is_err());
    std::fs::write(&path, b"corrupt").unwrap();
    assert!(checked_grid_image(&path, None, || Err("corrupt decoder input".into())).is_err());
    std::fs::remove_file(&path).unwrap();
    std::fs::write(&path, b"valid replacement").unwrap();
    let stamp = read_stamp(&path).unwrap();
    assert!(checked_grid_image(&path, Some(&stamp), || Ok(RgbaImage::new(129, 1))).is_err());
    let image = checked_grid_image(&path, Some(&stamp), || Ok(RgbaImage::new(128, 128))).unwrap();
    assert_eq!(image.frame_count(), 1);
    assert_eq!(image.as_bytes(0).unwrap().len(), 128 * 128 * 4);
}

#[test]
fn readable_sources_without_modification_times_keep_the_previous_decode_fallback() {
    let image =
        checked_grid_image_with_metadata(None, || Ok((12, None)), || Ok(pixels([1, 2, 3, 255])))
            .unwrap();
    assert_eq!(image.as_bytes(0).unwrap().len(), 8);
    let mut reads = 0;
    assert!(checked_grid_image_with_metadata(
        None,
        || {
            reads += 1;
            Ok((if reads == 1 { 12 } else { 13 }, None))
        },
        || Ok(pixels([1, 2, 3, 255]))
    )
    .is_err());
}
