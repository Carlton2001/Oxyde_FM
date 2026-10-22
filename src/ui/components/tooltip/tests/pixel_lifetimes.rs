use super::*;

fn image() -> Arc<RenderImage> {
    Arc::new(RenderImage::new(vec![image::Frame::new(
        image::RgbaImage::new(2, 1),
    )]))
}

#[test]
fn shared_pixels_retire_only_after_the_final_display_and_preserve_the_cpu_cache() {
    let mut displayed = DisplayedImages::default();
    let first = EntityId::from(1);
    let second = EntityId::from(2);
    let cached = image();
    assert!(displayed.replace(first, Some(cached.clone())).is_none());
    assert!(displayed.replace(second, Some(cached.clone())).is_none());
    assert!(displayed.replace(first, None).is_none());
    let retired = displayed.replace(second, None).unwrap();
    assert!(Arc::ptr_eq(&retired, &cached));
    assert!(!displayed.contains(&cached));
    assert_eq!(cached.as_bytes(0).unwrap().len(), 8);
    // A new hover can reclaim these pixels before the deferred eviction runs.
    displayed.replace(first, Some(cached.clone()));
    assert!(displayed.contains(&retired));
    assert!(displayed.replace(first, Some(cached.clone())).is_none());
}

#[test]
fn successive_video_frames_and_transition_images_do_not_accumulate() {
    let mut displayed = DisplayedImages::default();
    let owner = EntityId::from(1);
    let mut previous = None;
    for _ in 0..2000 {
        let frame = image();
        let probe = Arc::downgrade(&frame);
        let retired = displayed.replace(owner, Some(frame));
        if let Some(previous) = previous {
            assert!(Arc::ptr_eq(retired.as_ref().unwrap(), &previous));
        } else {
            assert!(retired.is_none());
        }
        let retired_probe = retired.as_ref().map(Arc::downgrade);
        drop(retired);
        assert!(retired_probe.is_none_or(|probe| probe.upgrade().is_none()));
        assert_eq!(displayed.0.len(), 1);
        previous = probe.upgrade();
    }
    drop(previous);
    let retired = displayed.replace(owner, None).unwrap();
    let probe = Arc::downgrade(&retired);
    drop(retired);
    assert!(probe.upgrade().is_none());
    assert!(displayed.0.is_empty());
}
