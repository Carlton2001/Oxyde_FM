//! Worker-only cover cache: file revisions stay separate, identical pixels share storage.
use gpui::RenderImage;
use oxyde_lib::models::FileStamp;
use std::{
    collections::{HashMap, VecDeque},
    path::Path,
    sync::{Arc, Mutex, OnceLock, Weak},
};

#[derive(Default)]
struct Pool(HashMap<blake3::Hash, Weak<RenderImage>>);

impl Pool {
    fn share(&mut self, image: Arc<RenderImage>) -> Arc<RenderImage> {
        let mut hash = blake3::Hasher::new();
        let size = image.size(0);
        hash.update(&(size.width.0 as u32).to_le_bytes());
        hash.update(&(size.height.0 as u32).to_le_bytes());
        hash.update(image.as_bytes(0).unwrap_or_default());
        let key = hash.finalize();
        if let Some(shared) = self.0.get(&key).and_then(Weak::upgrade) {
            return shared;
        }
        self.0.retain(|_, image| image.strong_count() > 0);
        if self.0.len() >= 4096 {
            if let Some(key) = self.0.keys().next().copied() {
                self.0.remove(&key);
            }
        }
        self.0.insert(key, Arc::downgrade(&image));
        image
    }
}

#[derive(Default)]
struct Cache {
    entries: VecDeque<(String, FileStamp, Arc<RenderImage>, bool)>,
    revision: u64,
}
static CACHE: OnceLock<Mutex<Cache>> = OnceLock::new();
static POOL: OnceLock<Mutex<Pool>> = OnceLock::new();

pub(crate) fn diagnostic_snapshot() -> Option<(usize, usize)> {
    let Some(cache) = CACHE.get() else { return Some((0, 0)) };
    cache.try_lock().ok().map(|cache| {
        let mut unique = std::collections::HashSet::new();
        cache.entries.iter().filter(|(_, _, image, _)| unique.insert(image.id))
            .fold((0, 0), |(count, bytes), (_, _, image, _)| {
                (count + 1, bytes + image.as_bytes(0).map_or(0, <[u8]>::len))
            })
    })
}

pub(crate) fn share(image: Arc<RenderImage>) -> Arc<RenderImage> {
    POOL.get_or_init(Default::default)
        .lock()
        .unwrap()
        .share(image)
}

pub(crate) fn invalidate(path: &str) {
    if let Some(cache) = CACHE.get() {
        let mut cache = cache.lock().unwrap();
        cache.revision = cache.revision.wrapping_add(1);
        cache.entries.retain(|(source, _, _, _)| source != path);
    }
}

pub(crate) fn clear() {
    if let Some(cache) = CACHE.get() {
        let mut cache = cache.lock().unwrap();
        cache.revision = cache.revision.wrapping_add(1);
        cache.entries.clear();
    }
}

pub(crate) fn load(path: &str) -> Result<(Arc<RenderImage>, bool), String> {
    load_size(path, false)
}

pub(crate) fn load_thumbnail(path: &str) -> Result<Arc<RenderImage>, String> {
    load_size(path, true).map(|(image, _)| image)
}

fn load_size(path: &str, thumbnail: bool) -> Result<(Arc<RenderImage>, bool), String> {
    with_cache(path, thumbnail, None, || {
        let pixels = oxyde_lib::platform::platform()
            .audio_cover(Path::new(path), thumbnail)
            .map_err(|error| error.to_string())?
            .ok_or("No embedded audio cover")?;
        Ok(Arc::new(RenderImage::new(vec![image::Frame::new(pixels)])))
    })
}

/// Keep decoded grid pixels across card leases, including disk-cache hits.
pub(crate) fn load_grid(
    path: &str,
    expected: Option<&FileStamp>,
    load: impl FnOnce() -> Result<Arc<RenderImage>, String>,
) -> Result<Arc<RenderImage>, String> {
    with_cache(path, true, expected, load).map(|(image, _)| image)
}

/// UI hint only: source validation stays on the loading worker.
pub(crate) fn may_have_cached(path: &str) -> bool {
    CACHE.get().is_some_and(|cache| match cache.try_lock() {
        Ok(cache) => cache.entries.iter().any(|entry| entry.0 == path && !entry.3),
        Err(std::sync::TryLockError::WouldBlock) => true,
        Err(_) => false,
    })
}

fn with_cache(
    path: &str,
    thumbnail: bool,
    expected: Option<&FileStamp>,
    load: impl FnOnce() -> Result<Arc<RenderImage>, String>,
) -> Result<(Arc<RenderImage>, bool), String> {
    let stamp = || {
        std::fs::metadata(path)
            .map_err(|error| error.to_string())
            .map(|metadata| FileStamp::from_metadata(&metadata))
    };
    let before = stamp()?;
    if expected.is_some_and(|expected| Some(expected) != before.as_ref()) {
        return Err("Thumbnail source changed since listing".into());
    }
    let cache = CACHE.get_or_init(Default::default);
    let revision = {
        let mut cache = cache.lock().unwrap();
        if let Some(index) = cache
            .entries
            .iter()
            .position(|(source, _, _, small)| source == path && *small == thumbnail)
        {
            let entry = cache.entries.remove(index).unwrap();
            if Some(&entry.1) == before.as_ref() {
                let image = entry.2.clone();
                cache.entries.push_back(entry);
                return Ok((image, true));
            }
        }
        cache.revision
    };
    let image = load()?;
    if stamp()? != before {
        return Err("Audio cover source changed during loading".into());
    }
    let image = share(image);
    if !thumbnail { crate::ui::preview_budget::admit(&image)?; }
    let mut cache = cache.lock().unwrap();
    if cache.revision == revision {
        if let Some(stamp) = before {
            cache.entries.retain(|(source, _, _, small)| source != path || *small != thumbnail);
            let limit = if thumbnail { 256 } else { 32 };
            while cache.entries.iter().filter(|entry| entry.3 == thumbnail).count() >= limit {
                let oldest = cache.entries.iter().position(|entry| entry.3 == thumbnail).unwrap();
                cache.entries.remove(oldest);
            }
            cache
                .entries
                .push_back((path.to_owned(), stamp, image.clone(), thumbnail));
        }
    }
    Ok((image, false))
}

#[cfg(test)]
#[path = "audio_covers/tests.rs"]
mod tests;
