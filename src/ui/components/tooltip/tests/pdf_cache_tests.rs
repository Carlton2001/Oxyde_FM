use super::{Cache, Signature, MAX_BYTES, MAX_ENTRIES};
use gpui::RenderImage;
use std::{
    sync::Arc,
    time::{Duration, SystemTime},
};

fn key(path: &str, length: u64) -> Signature {
    Signature {
        path: path.into(),
        length,
        modified: SystemTime::UNIX_EPOCH,
    }
}
fn image() -> Arc<RenderImage> {
    Arc::new(RenderImage::new(vec![image::Frame::new(
        image::RgbaImage::new(2, 1),
    )]))
}

#[test]
fn revisit_reuses_pixels_and_changed_source_invalidates() {
    let mut cache = Cache::default();
    let pixels = image();
    cache.insert(key("a", 10), pixels.clone(), 8);
    assert!(Arc::ptr_eq(&pixels, &cache.get(&key("a", 10)).unwrap()));
    let mut changed = key("a", 10);
    changed.modified += Duration::from_secs(1);
    assert!(cache.get(&changed).is_none());
    assert_eq!(cache.bytes, 0);
    cache.insert(key("a", 10), pixels, 8);
    assert!(cache.get(&key("a", 11)).is_none());
}

#[test]
fn least_recently_used_entry_is_evicted_at_count_limit() {
    let mut cache = Cache::default();
    for index in 0..MAX_ENTRIES {
        cache.insert(key(&index.to_string(), 10), image(), 8);
    }
    assert!(cache.get(&key("0", 10)).is_some());
    cache.insert(key("new", 10), image(), 8);
    assert_eq!(cache.entries.len(), MAX_ENTRIES);
    assert!(cache.get(&key("1", 10)).is_none());
    assert!(cache.get(&key("0", 10)).is_some());
}

#[test]
fn byte_limit_evicts_old_entries_and_refuses_oversized_pixels() {
    let mut cache = Cache::default();
    cache.insert(key("a", 10), image(), MAX_BYTES);
    cache.insert(key("b", 10), image(), 8);
    assert!(cache.get(&key("a", 10)).is_none());
    assert_eq!(cache.bytes, 8);
    cache.insert(key("huge", 10), image(), MAX_BYTES + 1);
    assert_eq!(cache.entries.len(), 1);
}

#[test]
fn invalidation_removes_pixels_and_rejects_old_inflight_load() {
    let mut cache = Cache::default();
    cache.insert(key("a", 10), image(), 8);
    let revision = cache.revision;
    cache.invalidate("a");
    assert_eq!(cache.bytes, 0);
    cache.insert_if_current(revision, key("a", 10), image(), 8);
    assert!(cache.get(&key("a", 10)).is_none());
    cache.insert_if_current(cache.revision, key("a", 10), image(), 8);
    assert!(cache.get(&key("a", 10)).is_some());
}
