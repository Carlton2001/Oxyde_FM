//! Assets belong to visible cards, not to every path visited during the session.
use super::GridThumbnailAsset;
use gpui::{App, Context, Entity, Global, RenderImage, SharedString, Window};
use oxyde_lib::models::{FileEntry, FileStamp};
use std::collections::HashMap;
use std::sync::{Arc, Weak};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(super) struct Source {
    pub(super) path: SharedString,
    pub(super) stamp: Option<FileStamp>,
    pub(super) audio: bool,
    size: u64,
    modified: u64,
    generation: u64,
}

impl Source {
    pub(super) fn for_file(file: &FileEntry) -> Self {
        Self {
            path: file.path.clone().into(),
            stamp: file.content_stamp.clone(),
            audio: super::is_audio(file),
            size: file.size,
            modified: file.modified,
            generation: 0,
        }
    }
}

#[derive(Default)]
struct Entry {
    cards: usize,
    image: Option<Weak<RenderImage>>,
}

#[derive(Default)]
struct Owners {
    sources: HashMap<Source, Entry>,
    generation: u64,
}
impl Global for Owners {}

impl Owners {
    fn shares_image(&self, image: &Arc<RenderImage>) -> bool {
        let image = Arc::downgrade(image);
        self.sources.values().any(|entry| entry.image.as_ref().is_some_and(|other| Weak::ptr_eq(other, &image)))
    }
    fn retain(&mut self, source: Source) {
        self.sources.entry(source).or_default().cards += 1;
    }

    fn release(&mut self, source: &Source) -> Option<Entry> {
        let entry = self.sources.get_mut(source)?;
        if entry.cards > 1 {
            entry.cards -= 1;
            return None;
        }
        self.sources.remove(source)
    }

    fn loaded(&mut self, source: &Source, image: &Arc<RenderImage>) {
        if let Some(entry) = self.sources.get_mut(source) {
            entry.image = Some(Arc::downgrade(image));
        }
    }

    fn take_image(&mut self, source: &Source) -> Option<Arc<RenderImage>> {
        self.sources.get_mut(source)?.image.take()?.upgrade()
    }

    fn reset(&mut self) -> Vec<Source> {
        // Live leases still own the old keys until rebinding or release. Never
        // let their release remove a newly loaded generation of the same path.
        self.generation = self.generation.wrapping_add(1);
        self.sources.keys().cloned().collect()
    }
}

fn release(source: &Source, window: Option<&mut Window>, cx: &mut App) {
    if let Some(entry) = cx.default_global::<Owners>().release(source) {
        // Upgrade before removing the cache's last strong image reference.
        let image = entry.image.and_then(|image| image.upgrade());
        cx.remove_asset::<GridThumbnailAsset>(source);
        if let Some(image) = image {
            if !cx.default_global::<Owners>().shares_image(&image) {
                cx.drop_image(image, window);
            }
        }
    }
}

struct CardLease {
    source: Source,
}

impl CardLease {
    fn new(source: Source, cx: &mut Context<Self>) -> Self {
        cx.default_global::<Owners>().retain(source.clone());
        cx.on_release(|lease, cx| release(&lease.source, None, cx))
            .detach();
        Self { source }
    }

    fn bind(&mut self, source: Source, window: &mut Window, cx: &mut Context<Self>) {
        if self.source == source {
            return;
        }
        cx.default_global::<Owners>().retain(source.clone());
        let old = std::mem::replace(&mut self.source, source);
        release(&old, Some(window), cx);
    }
}

pub(super) fn use_source(
    source: &Source,
    owner: &SharedString,
    window: &mut Window,
    cx: &mut App,
) -> Source {
    let mut source = source.clone();
    source.generation = cx.default_global::<Owners>().generation;
    // GPUI drops this entity when its element disappears from successive frames,
    // including scroll, tab/pane closure, details view and disabled thumbnails.
    let lease: Entity<CardLease> = window.use_keyed_state(owner.clone(), cx, |_, cx| {
        CardLease::new(source.clone(), cx)
    });
    lease.update(cx, |lease, cx| lease.bind(source.clone(), window, cx));
    source
}

pub(super) fn loaded(source: &Source, image: &Arc<RenderImage>, cx: &mut App) {
    cx.default_global::<Owners>().loaded(source, image);
}

fn evict(source: &Source, cx: &mut App) {
    let image = cx.default_global::<Owners>().take_image(source);
    cx.remove_asset::<GridThumbnailAsset>(source);
    if let Some(image) = image {
        if !cx.default_global::<Owners>().shares_image(&image) {
            cx.drop_image(image, None);
        }
    }
}

pub(super) fn invalidate(path: &str, cx: &mut App) {
    let sources: Vec<_> = cx
        .default_global::<Owners>()
        .sources
        .keys()
        .filter(|source| source.path.as_ref() == path)
        .cloned()
        .collect();
    for source in sources {
        evict(&source, cx);
    }
}

pub(crate) fn clear_grid_thumbnails(cx: &mut App) {
    crate::ui::audio_covers::clear();
    let sources = cx.default_global::<Owners>().reset();
    for source in sources {
        crate::ui::components::tooltip::invalidate_media_preview(&source.path);
        evict(&source, cx);
    }
}

#[cfg(test)]
#[path = "tests/lifetimes.rs"]
mod tests;
