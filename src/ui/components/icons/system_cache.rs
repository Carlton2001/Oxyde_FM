//! Bounds what GPUI's asset cache keeps of system icons: icons proper to one file
//! (programs, shortcuts, drives) leave once many others were shown since, and a
//! failed load is tried again after a while instead of for the whole session.

use super::SystemIconAsset;
use gpui::*;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Icons proper to one file kept at once, a few MB of small images.
const MAX_UNIQUE_ICONS: usize = 1024;
/// A failed icon is asked again when shown after this delay.
const RETRY_AFTER: Duration = Duration::from_secs(30);

type Source = (SharedString, bool);
type Output = Result<Arc<RenderImage>, ImageCacheError>;

#[derive(Default)]
struct SystemIconCache {
    /// Icons proper to one file, with the order in which they were last shown.
    unique: HashMap<Source, (u64, Arc<RenderImage>)>,
    shown: u64,
    failed: HashMap<Source, Instant>,
}

impl Global for SystemIconCache {}

/// The icon for `source`, loading it as needed. `unique` marks an icon proper to one file.
pub(super) fn use_icon(source: &Source, unique: bool, window: &mut Window, cx: &mut App) -> Option<Output> {
    let result = window.use_asset::<SystemIconAsset>(source, cx);
    match &result {
        Some(Err(_)) => {
            let cache = cx.default_global::<SystemIconCache>();
            let since = *cache.failed.entry(source.clone()).or_insert_with(Instant::now);
            if since.elapsed() >= RETRY_AFTER {
                cache.failed.remove(source);
                cx.remove_asset::<SystemIconAsset>(source);
                return window.use_asset::<SystemIconAsset>(source, cx);
            }
        }
        Some(Ok(image)) if unique => {
            let cache = cx.default_global::<SystemIconCache>();
            cache.shown += 1;
            let shown = cache.shown;
            cache.unique.insert(source.clone(), (shown, image.clone()));
            if cache.unique.len() > MAX_UNIQUE_ICONS {
                let oldest = cache.unique.iter().min_by_key(|(_, (shown, _))| *shown).map(|(key, _)| key.clone());
                if let Some((key, (_, image))) = oldest.and_then(|key| cache.unique.remove_entry(&key)) {
                    cx.remove_asset::<SystemIconAsset>(&key);
                    // The window being drawn is out of App.windows until it is done.
                    cx.defer(move |cx| cx.drop_image(image, None));
                }
            }
        }
        _ => {}
    }
    result
}
