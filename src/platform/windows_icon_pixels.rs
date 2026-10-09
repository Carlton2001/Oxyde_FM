//! Parent cache contains already validated pixels, never encoded source images.
use image::RgbaImage;
use std::collections::{HashMap, VecDeque};
use std::sync::{Mutex, OnceLock};

struct PixelCache {
    images: HashMap<String, RgbaImage>,
    order: VecDeque<String>,
    bytes: usize,
    byte_limit: usize,
    entry_limit: usize,
}
impl PixelCache {
    fn new(byte_limit: usize, entry_limit: usize) -> Self {
        Self {
            images: HashMap::new(),
            order: VecDeque::new(),
            bytes: 0,
            byte_limit,
            entry_limit,
        }
    }
    fn insert(&mut self, key: String, image: RgbaImage) {
        let length = image.as_raw().len();
        if self.entry_limit == 0 || length > self.byte_limit {
            return;
        }
        if let Some(old) = self.images.remove(&key) {
            self.bytes -= old.as_raw().len();
            self.order.retain(|entry| entry != &key);
        }
        while self.images.len() >= self.entry_limit || self.bytes > self.byte_limit - length {
            let Some(oldest) = self.order.pop_front() else {
                return;
            };
            if let Some(old) = self.images.remove(&oldest) {
                self.bytes -= old.as_raw().len();
            }
        }
        self.bytes += length;
        self.order.push_back(key.clone());
        self.images.insert(key, image);
    }
}
fn cache() -> &'static Mutex<PixelCache> {
    static CACHE: OnceLock<Mutex<PixelCache>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(PixelCache::new(32 * 1024 * 1024, 1000)))
}
pub(super) fn clear() {
    *cache().lock().unwrap_or_else(|error| error.into_inner()) =
        PixelCache::new(32 * 1024 * 1024, 1000);
}
pub(super) fn get(path: String, large: bool) -> Result<RgbaImage, String> {
    let path = path.replace('/', "\\");
    let mode = if large { "icon-large" } else { "icon-small" };
    let key = format!("{mode}|{path}");
    if let Some(image) = cache()
        .lock()
        .map_err(|error| error.to_string())?
        .images
        .get(&key)
        .cloned()
    {
        return Ok(image);
    }
    // No cache lock across file I/O, IPC or Shell handlers.
    let bytes = super::platform()
        .run_preview_worker(mode, std::path::Path::new(&path))
        .map_err(|error| error.to_string())?;
    let image = match crate::utils::preview_protocol::decode(&bytes)
        .map_err(|error| error.to_string())?
    {
        crate::utils::preview_protocol::Reply::Images { mut frames, .. } if frames.len() == 1 => {
            frames.remove(0).into_buffer()
        }
        crate::utils::preview_protocol::Reply::Error { detail, .. } => return Err(detail),
        _ => return Err("Unexpected icon worker reply".into()),
    };
    if image.width() > 256 || image.height() > 256 {
        return Err("Icon worker dimensions exceed their limit".into());
    }
    cache()
        .lock()
        .map_err(|error| error.to_string())?
        .insert(key, image.clone());
    Ok(image)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn image() -> RgbaImage {
        RgbaImage::from_pixel(1, 1, image::Rgba([1, 2, 3, 4]))
    }
    #[test]
    fn pixel_cache_evicts_on_bytes_count_and_replacement_without_rejecting_the_output() {
        let mut cache = PixelCache::new(8, 2);
        for name in ["a", "b", "c"] {
            cache.insert(name.into(), image());
        }
        assert_eq!(cache.bytes, 8);
        assert_eq!(cache.images.len(), 2);
        assert!(!cache.images.contains_key("a"));
        cache.insert("b".into(), RgbaImage::new(2, 1));
        assert_eq!(cache.bytes, 8);
        assert_eq!(cache.images.len(), 1);
        assert!(cache.images.contains_key("b"));
        cache.insert("large".into(), RgbaImage::new(3, 1));
        assert_eq!(cache.bytes, 8);
        assert_eq!(cache.images.len(), 1);
        let mut cache = PixelCache::new(100, 1);
        cache.insert("a".into(), image());
        cache.insert("b".into(), image());
        assert_eq!(cache.images.len(), 1);
        assert_eq!(cache.bytes, 4);
    }
}
