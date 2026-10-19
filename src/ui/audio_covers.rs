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

pub(crate) fn load(path: &str) -> Result<Arc<RenderImage>, String> {
    load_size(path, false)
}

pub(crate) fn load_thumbnail(path: &str) -> Result<Arc<RenderImage>, String> {
    load_size(path, true)
}

fn load_size(path: &str, thumbnail: bool) -> Result<Arc<RenderImage>, String> {
    let stamp = || {
        std::fs::metadata(path)
            .map_err(|error| error.to_string())
            .map(|metadata| FileStamp::from_metadata(&metadata))
    };
    let before = stamp()?;
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
                return Ok(image);
            }
        }
        cache.revision
    };
    let pixels = oxyde_lib::platform::platform()
        .audio_cover(Path::new(path), thumbnail)
        .map_err(|error| error.to_string())?
        .ok_or("No embedded audio cover")?;
    if stamp()? != before {
        return Err("Audio cover source changed during loading".into());
    }
    let image = share(Arc::new(RenderImage::new(vec![image::Frame::new(pixels)])));
    if !thumbnail { crate::ui::preview_budget::admit(&image)?; }
    let mut cache = cache.lock().unwrap();
    if cache.revision == revision {
        if let Some(stamp) = before {
            cache.entries.retain(|(source, _, _, small)| source != path || *small != thumbnail);
            while cache.entries.len() >= 32 {
                cache.entries.pop_front();
            }
            cache
                .entries
                .push_back((path.to_owned(), stamp, image.clone(), thumbnail));
        }
    }
    Ok(image)
}

#[cfg(test)]
#[path = "audio_covers/tests.rs"]
mod tests;
