//! Bounded RAM-only LRU of decoded PDF pixels. Files are checked on the worker,
//! never during UI rendering. GPUI's per-tooltip asset still releases on close.
use gpui::RenderImage;
use std::{
    collections::VecDeque,
    path::Path,
    sync::{Arc, Mutex, OnceLock},
    time::SystemTime,
};

const MAX_ENTRIES: usize = 32;
const MAX_BYTES: usize = 64 * 1024 * 1024;

#[derive(Clone, PartialEq, Eq)]
struct Signature {
    path: String,
    length: u64,
    modified: SystemTime,
}

impl Signature {
    fn read(path: &str) -> Option<Self> {
        let metadata = std::fs::metadata(Path::new(path)).ok()?;
        Some(Self {
            path: path.into(),
            length: metadata.len(),
            modified: metadata.modified().ok()?,
        })
    }
}

struct Entry {
    key: Signature,
    image: Arc<RenderImage>,
    bytes: usize,
}
#[derive(Default)]
struct Cache {
    entries: VecDeque<Entry>,
    bytes: usize,
    revision: u64,
}

impl Cache {
    fn invalidate(&mut self, path: &str) {
        self.revision += 1;
        if let Some(index) = self.entries.iter().position(|entry| entry.key.path == path) {
            self.remove(index);
        }
    }

    fn insert_if_current(
        &mut self,
        revision: u64,
        key: Signature,
        image: Arc<RenderImage>,
        bytes: usize,
    ) {
        if self.revision == revision {
            self.insert(key, image, bytes);
        }
    }
    fn remove(&mut self, index: usize) -> Entry {
        let entry = self.entries.remove(index).unwrap();
        self.bytes -= entry.bytes;
        entry
    }
    fn get(&mut self, key: &Signature) -> Option<Arc<RenderImage>> {
        let index = self
            .entries
            .iter()
            .position(|entry| entry.key.path == key.path)?;
        let entry = self.remove(index);
        if entry.key != *key {
            return None;
        }
        let image = entry.image.clone();
        self.bytes += entry.bytes;
        self.entries.push_back(entry);
        Some(image)
    }
    fn insert(&mut self, key: Signature, image: Arc<RenderImage>, bytes: usize) {
        if let Some(index) = self
            .entries
            .iter()
            .position(|entry| entry.key.path == key.path)
        {
            self.remove(index);
        }
        if bytes > MAX_BYTES {
            return;
        }
        while self.entries.len() >= MAX_ENTRIES || self.bytes + bytes > MAX_BYTES {
            self.remove(0);
        }
        self.bytes += bytes;
        self.entries.push_back(Entry { key, image, bytes });
    }
}

static CACHE: OnceLock<Mutex<Cache>> = OnceLock::new();

pub(super) fn invalidate(path: &str) {
    if let Some(cache) = CACHE.get() {
        let mut cache = cache.lock().unwrap();
        cache.invalidate(path);
    }
}

pub(super) fn load(path: String) -> Result<(Arc<RenderImage>, bool), String> {
    let cache = CACHE.get_or_init(|| Mutex::new(Cache::default()));
    let revision = cache.lock().unwrap().revision;
    let signature = Signature::read(&path);
    if let Some(key) = &signature {
        if let Some(image) = cache.lock().unwrap().get(key) {
            log::debug!("Tooltip PDF decoded cache hit: {path}");
            return Ok((image, true));
        }
    }
    log::debug!("Tooltip PDF decoded cache miss: {path}");
    let (pixels, disk_hit) = crate::ui::preview_disk::load_pdf(&path)?;
    let bytes = pixels.as_raw().len();
    let image = Arc::new(RenderImage::new(vec![image::Frame::new(pixels)]));
    crate::ui::preview_budget::admit(&image)?;
    // A changed source must not publish old pixels under its new metadata.
    if let Some(key) = signature {
        if Signature::read(&path).as_ref() == Some(&key) {
            let mut cache = cache.lock().unwrap();
            cache.insert_if_current(revision, key, image.clone(), bytes);
        }
    }
    Ok((image, disk_hit))
}

#[cfg(test)]
#[path = "tests/pdf_cache_tests.rs"]
mod tests;
