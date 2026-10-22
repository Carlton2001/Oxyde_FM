//! Atlas ownership follows displayed previews, independently of the bounded CPU caches.
use super::PreviewTask;
use gpui::{App, Context, EntityId, Global, RenderImage};
use std::{collections::HashMap, sync::Arc};

#[derive(Default)]
struct DisplayedImages(HashMap<EntityId, Arc<RenderImage>>);

impl Global for DisplayedImages {}

impl DisplayedImages {
    fn contains(&self, image: &RenderImage) -> bool {
        self.0.values().any(|displayed| displayed.id == image.id)
    }

    fn replace(
        &mut self,
        owner: EntityId,
        image: Option<Arc<RenderImage>>,
    ) -> Option<Arc<RenderImage>> {
        if self.0.get(&owner).map(|image| image.id) == image.as_ref().map(|image| image.id) {
            return None;
        }
        let previous = self.0.remove(&owner);
        if let Some(image) = image {
            self.0.insert(owner, image);
        }
        previous.filter(|image| !self.contains(image))
    }
}

pub(super) fn observe(cx: &mut Context<PreviewTask>) {
    let owner = cx.entity_id();
    cx.on_release(move |_, cx| display(owner, None, cx))
        .detach();
}

pub(super) fn display(owner: EntityId, image: Option<Arc<RenderImage>>, cx: &mut App) {
    let retired = cx.default_global::<DisplayedImages>().replace(owner, image);
    if let Some(image) = retired {
        // GPUI temporarily removes the current window from App.windows during
        // callbacks. Wait for its return, and for any handover sharing these pixels.
        cx.defer(move |cx| {
            if !cx.default_global::<DisplayedImages>().contains(&image) {
                cx.drop_image(image, None);
            }
        });
    }
}

#[cfg(test)]
#[path = "tests/pixel_lifetimes.rs"]
mod tests;
