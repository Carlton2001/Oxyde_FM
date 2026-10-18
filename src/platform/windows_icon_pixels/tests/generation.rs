use super::*;

#[test]
fn requests_started_before_purge_cannot_refill_memory_or_schedule_persistence() {
    let mut cache = PixelCache::new(100, 4);
    let old = cache.generation;
    cache.clear();
    let image = RgbaImage::new(1, 1);
    assert!(!cache.insert_if_current(old, "obsolete".into(), image.clone()));
    assert!(cache.images.is_empty());
    assert!(cache.insert_if_current(cache.generation, "fresh".into(), image));
    assert_eq!(cache.images.len(), 1);
}
