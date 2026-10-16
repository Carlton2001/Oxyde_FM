use super::*;
use gpui::RenderImage;
use std::sync::Arc;

enum Cached {
    Pixels(Arc<RenderImage>),
    Pending(futures::channel::oneshot::Receiver<()>),
    Error,
}
type Cache = HashMap<Source, Cached>;

fn source(index: usize) -> Source {
    Source::for_file(&FileEntry {
        path: format!("image-{index}.png"),
        size: 12,
        modified: 1,
        ..Default::default()
    })
}
fn pixels() -> Arc<RenderImage> {
    Arc::new(RenderImage::new(vec![image::Frame::new(
        image::RgbaImage::new(2, 1),
    )]))
}
fn close(owners: &mut Owners, cache: &mut Cache, source: &Source) {
    if owners.release(source).is_some() {
        cache.remove(source);
    }
}

#[test]
fn two_panes_share_pixels_until_the_last_card_and_frame_release() {
    let mut owners = Owners::default();
    let mut cache = Cache::new();
    let source = source(0);
    let image = pixels();
    let probe = Arc::downgrade(&image);
    let frame = image.clone();
    cache.insert(source.clone(), Cached::Pixels(image));
    owners.retain(source.clone());
    owners.retain(source.clone());
    close(&mut owners, &mut cache, &source);
    assert_eq!(owners.sources.len(), 1);
    match cache.get(&source).unwrap() {
        Cached::Pixels(image) => assert!(Arc::ptr_eq(image, &frame)),
        _ => panic!("shared pixels were removed"),
    }
    close(&mut owners, &mut cache, &source);
    assert!(cache.is_empty() && owners.sources.is_empty());
    assert!(
        probe.upgrade().is_some(),
        "the last frame still owns its pixels"
    );
    drop(frame);
    assert!(probe.upgrade().is_none());
}

#[test]
fn pending_and_failed_assets_leave_with_their_cards_and_can_retry() {
    let mut owners = Owners::default();
    let mut cache = Cache::new();
    let source = source(0);
    let (sender, receiver) = futures::channel::oneshot::channel();
    cache.insert(source.clone(), Cached::Pending(receiver));
    owners.retain(source.clone());
    owners.retain(source.clone());
    close(&mut owners, &mut cache, &source);
    assert!(!sender.is_canceled());
    if let Cached::Pending(receiver) = cache.get_mut(&source).unwrap() {
        assert_eq!(receiver.try_recv().unwrap(), None);
    }
    close(&mut owners, &mut cache, &source);
    assert!(sender.is_canceled());
    owners.retain(source.clone());
    cache.insert(source.clone(), Cached::Error);
    close(&mut owners, &mut cache, &source);
    assert!(!cache.contains_key(&source));
    owners.retain(source.clone());
    cache.insert(source.clone(), Cached::Pixels(pixels()));
    assert!(matches!(cache.get(&source), Some(Cached::Pixels(_))));
    close(&mut owners, &mut cache, &source);
    assert!(cache.is_empty() && owners.sources.is_empty());
}

#[test]
fn scrolling_ten_thousand_paths_retains_only_the_visible_working_set() {
    let mut owners = Owners::default();
    let mut cache = Cache::new();
    let mut visible = Vec::new();
    let mut probes = Vec::new();
    for first in (0..10_000).step_by(12) {
        let next: Vec<_> = (first..first + 24).map(source).collect();
        // Retain the next frame before releasing the old one, as GPUI does.
        for source in &next {
            owners.retain(source.clone());
            cache.entry(source.clone()).or_insert_with(|| {
                let image = pixels();
                probes.push(Arc::downgrade(&image));
                Cached::Pixels(image)
            });
        }
        assert!(cache.len() <= 36);
        for old in visible {
            close(&mut owners, &mut cache, &old);
        }
        visible = next;
        assert_eq!(cache.len(), 24);
        assert_eq!(owners.sources.len(), 24);
        assert_eq!(
            probes
                .iter()
                .filter(|probe| probe.strong_count() > 0)
                .count(),
            24
        );
    }
    for old in visible {
        close(&mut owners, &mut cache, &old);
    }
    assert!(probes.iter().all(|probe| probe.upgrade().is_none()));
    assert!(cache.is_empty() && owners.sources.is_empty());
}

#[test]
fn cache_clear_keeps_old_leases_from_evicting_replacement_assets() {
    let mut owners = Owners::default();
    let mut cache = Cache::new();
    let old = source(0);
    owners.retain(old.clone());
    owners.retain(old.clone());
    cache.insert(old.clone(), Cached::Error);
    for source in owners.reset() {
        cache.remove(&source);
    }
    let mut fresh = old.clone();
    fresh.generation = owners.generation;
    assert_ne!(old, fresh);
    owners.retain(fresh.clone());
    cache.insert(fresh.clone(), Cached::Pixels(pixels()));
    close(&mut owners, &mut cache, &old);
    close(&mut owners, &mut cache, &old);
    assert!(matches!(cache.get(&fresh), Some(Cached::Pixels(_))));
    close(&mut owners, &mut cache, &fresh);
    assert!(cache.is_empty() && owners.sources.is_empty());
}

#[test]
fn listing_revisions_distinguish_same_size_updates_and_preserve_other_panes() {
    use std::time::{Duration, SystemTime};
    let mut owners = Owners::default();
    let mut cache = Cache::new();
    let mut file = FileEntry {
        path: "same.png".into(),
        size: 12,
        modified: 1,
        content_stamp: Some(FileStamp {
            length: 12,
            modified: SystemTime::UNIX_EPOCH + Duration::from_micros(100),
        }),
        ..Default::default()
    };
    let old = Source::for_file(&file);
    file.content_stamp.as_mut().unwrap().modified += Duration::from_micros(100);
    let fresh = Source::for_file(&file);
    assert_ne!(old, fresh);
    owners.retain(old.clone());
    owners.retain(old.clone());
    owners.retain(fresh.clone());
    cache.insert(old.clone(), Cached::Error);
    cache.insert(fresh.clone(), Cached::Pixels(pixels()));
    close(&mut owners, &mut cache, &old);
    assert!(cache.contains_key(&old) && cache.contains_key(&fresh));
    close(&mut owners, &mut cache, &old);
    assert!(!cache.contains_key(&old) && cache.contains_key(&fresh));
    close(&mut owners, &mut cache, &fresh);
    assert!(owners.sources.is_empty() && cache.is_empty());
}

#[test]
fn atlas_cleanup_gets_only_the_final_owners_image_without_retaining_pixels() {
    let mut owners = Owners::default();
    let first = source(0);
    let other = source(1);
    let image = pixels();
    let probe = Arc::downgrade(&image);
    let other_image = pixels();
    owners.retain(first.clone());
    owners.retain(first.clone());
    owners.retain(other.clone());
    owners.loaded(&first, &image);
    owners.loaded(&other, &other_image);
    assert_eq!(
        Arc::strong_count(&image),
        1,
        "the registry must only hold weak pixels"
    );
    assert!(owners.release(&first).is_none());
    let released = owners.release(&first).unwrap();
    let atlas_cleanup = released.image.unwrap().upgrade().unwrap();
    assert!(Arc::ptr_eq(&atlas_cleanup, &image));
    drop(image);
    assert!(
        probe.upgrade().is_some(),
        "cleanup keeps pixels alive until drop_image"
    );
    drop(atlas_cleanup);
    assert!(probe.upgrade().is_none());
    assert!(Arc::ptr_eq(
        &owners.take_image(&other).unwrap(),
        &other_image
    ));
    assert!(
        owners.take_image(&other).is_none(),
        "an explicit eviction consumes its atlas handle once"
    );
    assert!(owners.release(&other).is_some());
    assert!(owners.sources.is_empty());
}
