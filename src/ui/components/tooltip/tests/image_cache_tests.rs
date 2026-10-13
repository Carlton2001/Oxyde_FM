use super::*;
use gpui::RenderImage;
use std::{cell::Cell, sync::Arc};

#[test]
fn invalidation_during_decode_does_not_retain_late_pixels() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("image.png");
    std::fs::write(&path, b"source").unwrap();
    let cache = Mutex::new(Cache::new(32, 1024));
    cached_load(&cache, &path, || {
        cache.lock().unwrap().invalidate(&path);
        Ok(preview(1))
    }).unwrap();
    assert!(cache.lock().unwrap().entries.is_empty());
}

#[test]
fn handover_cache_status_distinguishes_hits_from_changed_sources() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("image.png");
    std::fs::write(&path, b"first").unwrap();
    let cache = Mutex::new(Cache::new(32, 1024));
    let (_, cached) = cached_load_with_status(&cache, &path, || Ok(preview(1))).unwrap();
    assert!(!cached);
    let (_, cached) = cached_load_with_status(&cache, &path, || panic!("cache hit must not decode")).unwrap();
    assert!(cached);
    std::fs::write(&path, b"modified source").unwrap();
    let (_, cached) = cached_load_with_status(&cache, &path, || Ok(preview(1))).unwrap();
    assert!(!cached, "stale cache entries must not bypass the fresh-preview delay");
}

fn preview(frames: usize) -> PreviewImage {
    PreviewImage {
        image: Arc::new(RenderImage::new(
            (0..frames)
                .map(|_| image::Frame::new(image::RgbaImage::new(2, 1)))
                .collect::<Vec<_>>(),
        )),
        notice: Some(crate::ui::i18n::Message::PreviewAnimationStatic),
    }
}

#[test]
fn reopening_reuses_same_decoded_image_without_calling_decoder() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("image.png");
    std::fs::write(&path, b"first").unwrap();
    let cache = Mutex::new(Cache::new(32, 1024));
    let calls = Cell::new(0);
    let first = cached_load(&cache, &path, || {
        calls.set(calls.get() + 1);
        Ok(preview(2))
    })
    .unwrap();
    let weak = Arc::downgrade(&first.image);
    drop(first); // The previous tooltip and its GPUI asset have been released.
    let second = cached_load(&cache, &path, || {
        calls.set(calls.get() + 1);
        Ok(preview(2))
    })
    .unwrap();
    assert_eq!(calls.get(), 1);
    assert!(Arc::ptr_eq(&weak.upgrade().unwrap(), &second.image));
    assert_eq!(
        second.notice,
        Some(crate::ui::i18n::Message::PreviewAnimationStatic)
    );
    std::fs::write(&path, b"changed length").unwrap();
    let changed = cached_load(&cache, &path, || {
        calls.set(calls.get() + 1);
        Ok(preview(1))
    })
    .unwrap();
    assert_eq!(calls.get(), 2);
    assert!(!Arc::ptr_eq(&second.image, &changed.image));
}

#[test]
fn failed_or_changed_during_decode_results_are_not_cached() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("image.png");
    std::fs::write(&path, b"first").unwrap();
    let cache = Mutex::new(Cache::new(32, 1024));
    assert!(cached_load(&cache, &path, || Err(PreviewError::Decode(
        "invalid".into()
    )))
    .is_err());
    assert!(cache.lock().unwrap().entries.is_empty());
    cached_load(&cache, &path, || {
        std::fs::write(&path, b"changed during decode").unwrap();
        Ok(preview(1))
    })
    .unwrap();
    assert!(cache.lock().unwrap().entries.is_empty());
    cached_load(&cache, &path, || Ok(preview(1))).unwrap();
    std::fs::remove_file(&path).unwrap();
    assert!(cached_load(&cache, &path, || Err(PreviewError::Read("missing".into()))).is_err());
    assert!(cache.lock().unwrap().entries.is_empty());
}

#[test]
fn lru_counts_all_animation_frames_and_bounds_retained_pixels() {
    let signature = |name: &str| Signature {
        path: name.into(),
        length: 1,
        modified: SystemTime::UNIX_EPOCH,
    };
    let mut cache = Cache::new(2, 24);
    cache.insert(signature("a"), preview(1)); // 8 bytes
    cache.insert(signature("b"), preview(2)); // 16 bytes
    assert!(cache.get(Path::new("a"), Some(&signature("a"))).is_some());
    cache.insert(signature("c"), preview(1));
    assert!(cache.get(Path::new("b"), Some(&signature("b"))).is_none());
    assert_eq!(cache.bytes, 16);
    cache.insert(signature("huge"), preview(4));
    assert_eq!(cache.bytes, 16);
    assert_eq!(cache.entries.len(), 2);
    let mut modified = signature("a");
    modified.modified += std::time::Duration::from_secs(1);
    assert!(cache.get(Path::new("a"), Some(&modified)).is_none());
    assert_eq!(cache.bytes, 8);
}
