//! Decoded image LRU survives tooltip release; filesystem checks stay on workers.
use crate::ui::image_preview::{PreviewError, PreviewImage};
use gpui::SvgRenderer;
use std::{
    collections::VecDeque,
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
    time::SystemTime,
};

const MAX_ENTRIES: usize = 32;
const MAX_BYTES: usize = 64 * 1024 * 1024;
static CACHE: OnceLock<Mutex<Cache>> = OnceLock::new();

#[derive(Clone, PartialEq, Eq)]
struct Signature {
    path: PathBuf,
    length: u64,
    modified: SystemTime,
}
impl Signature {
    fn read(path: &Path) -> Option<Self> {
        let metadata = std::fs::metadata(path).ok()?;
        Some(Self {
            path: path.into(),
            length: metadata.len(),
            modified: metadata.modified().ok()?,
        })
    }
}

struct Entry {
    signature: Signature,
    preview: PreviewImage,
    bytes: usize,
}
struct Cache {
    revision: u64,
    entries: VecDeque<Entry>,
    bytes: usize,
    max_entries: usize,
    max_bytes: usize,
}
impl Cache {
    fn new(max_entries: usize, max_bytes: usize) -> Self {
        Self {
            revision: 0,
            entries: VecDeque::new(),
            bytes: 0,
            max_entries,
            max_bytes,
        }
    }
    fn remove(&mut self, index: usize) -> Entry {
        let entry = self.entries.remove(index).unwrap();
        self.bytes -= entry.bytes;
        entry
    }
    fn invalidate(&mut self, path: &Path) {
        self.revision += 1;
        if let Some(index) = self.entries.iter().position(|entry| entry.signature.path == path) {
            self.remove(index);
        }
    }
    fn get(&mut self, path: &Path, signature: Option<&Signature>) -> Option<PreviewImage> {
        let index = self
            .entries
            .iter()
            .position(|entry| entry.signature.path == path)?;
        let entry = self.remove(index);
        if signature != Some(&entry.signature) {
            return None;
        }
        let preview = entry.preview.clone();
        self.bytes += entry.bytes;
        self.entries.push_back(entry);
        Some(preview)
    }
    fn insert(&mut self, signature: Signature, preview: PreviewImage) {
        // Count every animation frame, not just the first frame or the file size.
        let bytes = (0..preview.image.frame_count()).try_fold(0usize, |sum, frame| {
            sum.checked_add(preview.image.as_bytes(frame)?.len())
        });
        let Some(bytes) = bytes.filter(|bytes| *bytes <= self.max_bytes) else {
            return;
        };
        if self.max_entries == 0 {
            return;
        }
        if let Some(index) = self
            .entries
            .iter()
            .position(|entry| entry.signature.path == signature.path)
        {
            self.remove(index);
        }
        while self.entries.len() >= self.max_entries
            || bytes > self.max_bytes.saturating_sub(self.bytes)
        {
            self.remove(0);
        }
        self.bytes += bytes;
        self.entries.push_back(Entry {
            signature,
            preview,
            bytes,
        });
    }
}

fn cached_load_with_status(
    cache: &Mutex<Cache>,
    path: &Path,
    decode: impl FnOnce() -> Result<PreviewImage, PreviewError>,
) -> Result<(PreviewImage, bool), PreviewError> {
    let revision = cache.lock().unwrap().revision;
    let signature = Signature::read(path);
    if let Some(preview) = cache.lock().unwrap().get(path, signature.as_ref()) {
        log::debug!("Tooltip image decoded cache hit: {}", path.display());
        return Ok((preview, true));
    }
    log::debug!("Tooltip image decoded cache miss: {}", path.display());
    // Do not hold the cache lock during decoding, and never retain failures.
    let preview = decode()?;
    if let Some(signature) = signature {
        if Signature::read(path).as_ref() == Some(&signature) {
            let mut cache = cache.lock().unwrap();
            if cache.revision == revision { cache.insert(signature, preview.clone()); }
        }
    }
    Ok((preview, false))
}

pub(super) fn load(path: &Path, renderer: &SvgRenderer) -> Result<(PreviewImage, bool), PreviewError> {
    let cache = CACHE.get_or_init(|| Mutex::new(Cache::new(MAX_ENTRIES, MAX_BYTES)));
    let disk_hit = std::cell::Cell::new(false);
    cached_load_with_status(cache, path, || {
        let (preview, cached) = crate::ui::preview_disk::load_image(path, renderer)?;
        disk_hit.set(cached);
        Ok(preview)
    }).map(|(preview, cached)| (preview, cached || disk_hit.get()))
}

pub(super) fn invalidate(path: &Path) {
    if let Some(cache) = CACHE.get() { cache.lock().unwrap().invalidate(path); }
}

#[cfg(test)]
fn cached_load(cache: &Mutex<Cache>, path: &Path, decode: impl FnOnce() -> Result<PreviewImage, PreviewError>) -> Result<PreviewImage, PreviewError> {
    cached_load_with_status(cache, path, decode).map(|(preview, _)| preview)
}

#[cfg(test)]
#[path = "tests/image_cache_tests.rs"]
mod tests;
