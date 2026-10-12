use super::PreviewTask;
use gpui::{App, Asset, Context, Global, ImageCacheError, RenderImage, SharedString};
use std::sync::Arc;

pub(super) struct TooltipPreviewAsset;

impl Asset for TooltipPreviewAsset {
    type Source = SharedString;
    type Output = Result<Arc<RenderImage>, ImageCacheError>;

    fn load(
        source: Self::Source,
        _cx: &mut App,
    ) -> impl std::future::Future<Output = Self::Output> + Send + 'static {
        let path = source.to_string();
        let task = crate::ui::preview_jobs::load(move || {
            let result = super::pdf_cache::load(path);
            result.map_err(|error| {
                log::warn!("Could not load tooltip media preview: {error}");
                ImageCacheError::Asset(error.to_string().into())
            })
        });
        async move {
            let image = task
                .await
                .map_err(|error| ImageCacheError::Asset(error.into()))?;
            Ok(image)
        }
    }
}

pub(super) struct TooltipImageAsset;

impl Asset for TooltipImageAsset {
    type Source = std::path::PathBuf;
    type Output =
        Result<crate::ui::image_preview::PreviewImage, crate::ui::image_preview::PreviewError>;

    fn load(
        source: Self::Source,
        cx: &mut App,
    ) -> impl std::future::Future<Output = Self::Output> + Send + 'static {
        let renderer = cx.svg_renderer();
        // Preserve the typed decoder error across the job runner's worker errors.
        let task = crate::ui::preview_jobs::load(move || {
            Ok::<_, String>(crate::ui::image_preview::load_path(&source, &renderer))
        });
        async move {
            let result = task
                .await
                .map_err(crate::ui::image_preview::PreviewError::Worker)?;
            if let Err(error) = &result {
                log::warn!("Could not load tooltip image preview: {error}");
            }
            result
        }
    }
}

// GPUI keeps both pending loads and completed outputs until explicit eviction.
// Count preview ENTITIES, rather than render-time snapshots or result clones.
#[derive(Clone, PartialEq, Eq, Hash)]
pub(super) enum TooltipAssetSource {
    Image(std::path::PathBuf),
    Pdf(SharedString),
}

#[derive(Default)]
struct TooltipAssetOwners {
    owners: std::collections::HashMap<TooltipAssetSource, usize>,
}

impl Global for TooltipAssetOwners {}

impl TooltipAssetOwners {
    fn retain(&mut self, source: TooltipAssetSource) {
        *self.owners.entry(source).or_default() += 1;
    }

    /// True only when the final registered preview entity releases this source.
    fn release(&mut self, source: &TooltipAssetSource) -> bool {
        let Some(count) = self.owners.get_mut(source) else {
            return false;
        };
        if *count > 1 {
            *count -= 1;
            return false;
        }
        self.owners.remove(source);
        true
    }
}

pub(super) fn retain_tooltip_asset(source: TooltipAssetSource, cx: &mut Context<PreviewTask>) {
    cx.default_global::<TooltipAssetOwners>()
        .retain(source.clone());
    cx.on_release(move |_, cx| {
        if cx.default_global::<TooltipAssetOwners>().release(&source) {
            match source {
                TooltipAssetSource::Image(path) => cx.remove_asset::<TooltipImageAsset>(&path),
                TooltipAssetSource::Pdf(path) => cx.remove_asset::<TooltipPreviewAsset>(&path),
            }
        }
    })
    .detach();
}

#[cfg(test)]
mod tooltip_cache_tests {
    use super::{TooltipAssetOwners, TooltipAssetSource};
    use gpui::RenderImage;
    use std::{collections::HashMap, sync::Arc};

    // Exercise lifetime policy with real RenderImage payloads and pending/error
    // entries. GPUI's release callback/cache integration is compiled separately;
    // these tests do not create an interactive window or test its renderer.
    enum Cached {
        Image(Arc<RenderImage>),
        Pending(futures::channel::oneshot::Receiver<()>),
        Error,
    }
    type Cache = HashMap<TooltipAssetSource, Cached>;

    fn close(owners: &mut TooltipAssetOwners, cache: &mut Cache, source: &TooltipAssetSource) {
        if owners.release(source) {
            cache.remove(source);
        }
    }

    fn image() -> Arc<RenderImage> {
        Arc::new(RenderImage::new(vec![image::Frame::new(
            image::RgbaImage::new(2, 1),
        )]))
    }

    #[test]
    fn tooltip_cache_shared_previews_keep_pixels_until_final_entity_and_render_reference() {
        let source = TooltipAssetSource::Image("same.png".into());
        let pdf = TooltipAssetSource::Pdf("same.png".into());
        let mut owners = TooltipAssetOwners::default();
        let mut cache = Cache::new();
        let pixels = image();
        let probe = Arc::downgrade(&pixels);
        let pdf_pixels = image();
        let pdf_probe = Arc::downgrade(&pdf_pixels);
        let render_reference = pixels.clone();
        cache.insert(source.clone(), Cached::Image(pixels));
        cache.insert(pdf.clone(), Cached::Image(pdf_pixels));
        owners.retain(source.clone());
        owners.retain(source.clone());
        owners.retain(pdf.clone());
        close(&mut owners, &mut cache, &source);
        assert!(cache.contains_key(&source) && probe.upgrade().is_some());
        match cache.get(&source).unwrap() {
            Cached::Image(image) => assert!(Arc::ptr_eq(image, &render_reference)),
            _ => panic!("shared image was discarded"),
        }
        close(&mut owners, &mut cache, &source);
        assert!(!cache.contains_key(&source));
        assert!(cache.contains_key(&pdf) && pdf_probe.upgrade().is_some());
        assert!(probe.upgrade().is_some());
        drop(render_reference);
        assert!(probe.upgrade().is_none());
        close(&mut owners, &mut cache, &pdf);
        assert!(pdf_probe.upgrade().is_none());
        assert!(cache.is_empty() && owners.owners.is_empty());
    }

    #[test]
    fn tooltip_cache_pending_load_is_removed_only_after_last_owner() {
        let source = TooltipAssetSource::Image("pending.png".into());
        let mut owners = TooltipAssetOwners::default();
        let mut cache = Cache::new();
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
        assert!(cache.is_empty() && owners.owners.is_empty());
        // A preview can be released before its first render, without an asset entry.
        owners.retain(source.clone());
        close(&mut owners, &mut cache, &source);
        assert!(cache.is_empty() && owners.owners.is_empty());
    }

    #[test]
    fn tooltip_cache_successive_hover_results_do_not_accumulate_or_keep_stale_errors() {
        let mut owners = TooltipAssetOwners::default();
        let mut cache = Cache::new();
        for index in 0..2000 {
            let source = TooltipAssetSource::Image(format!("image-{index}.png").into());
            owners.retain(source.clone());
            let pixels = image();
            let probe = Arc::downgrade(&pixels);
            cache.insert(
                source.clone(),
                if index % 2 == 0 {
                    Cached::Image(pixels)
                } else {
                    drop(pixels);
                    Cached::Error
                },
            );
            close(&mut owners, &mut cache, &source);
            assert!(probe.upgrade().is_none());
            assert!(cache.is_empty() && owners.owners.is_empty());
        }
        let source = TooltipAssetSource::Pdf("repaired.pdf".into());
        owners.retain(source.clone());
        cache.insert(source.clone(), Cached::Error);
        close(&mut owners, &mut cache, &source);
        // Reopening after a repair starts with no previous cached failure.
        assert!(!cache.contains_key(&source));
        owners.retain(source.clone());
        cache.insert(source.clone(), Cached::Image(image()));
        assert!(matches!(cache.get(&source), Some(Cached::Image(_))));
        close(&mut owners, &mut cache, &source);
        assert!(cache.is_empty() && owners.owners.is_empty());
    }
}
