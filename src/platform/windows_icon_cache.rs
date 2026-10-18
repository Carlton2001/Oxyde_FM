use image::RgbaImage;
use std::collections::{HashMap, VecDeque};
use std::io::{Cursor, Read};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

static CACHE_TEMP_ID: AtomicU64 = AtomicU64::new(0);

const MAX_PNG_BYTES: usize = 1024 * 1024;
const MAX_ICON_SIDE: u32 = 256;
const MAX_DECODE_ALLOC: u64 = 4 * 1024 * 1024;
const MAX_MEMORY_BYTES: usize = 32 * 1024 * 1024;
const MAX_MEMORY_ENTRIES: usize = 1000;

pub(super) struct IconCache {
    entries: HashMap<String, RgbaImage>,
    insertion_order: VecDeque<String>,
    bytes: usize,
    byte_limit: usize,
    entry_limit: usize,
}

impl Default for IconCache {
    fn default() -> Self {
        Self::new(MAX_MEMORY_ENTRIES, MAX_MEMORY_BYTES)
    }
}

impl IconCache {
    fn new(entry_limit: usize, byte_limit: usize) -> Self {
        Self {
            entries: HashMap::new(),
            insertion_order: VecDeque::new(),
            bytes: 0,
            byte_limit,
            entry_limit,
        }
    }

    fn get(&self, key: &str) -> Option<RgbaImage> {
        self.entries.get(key).cloned()
    }

    // The caller validates the pixels before admitting them. Do not make admission
    // failure an icon failure: a useful icon may simply remain uncached.
    fn insert(&mut self, key: String, data: RgbaImage) {
        let length = data.as_raw().len();
        if self.entry_limit == 0 || length > self.byte_limit {
            return;
        }
        if let Some(previous) = self.entries.remove(&key) {
            self.bytes -= previous.as_raw().len();
            self.insertion_order.retain(|entry| entry != &key);
        }
        while self.entries.len() >= self.entry_limit || self.bytes > self.byte_limit - length {
            let Some(oldest) = self.insertion_order.pop_front() else {
                return;
            };
            if let Some(previous) = self.entries.remove(&oldest) {
                self.bytes -= previous.as_raw().len();
            }
        }
        self.bytes += length;
        self.insertion_order.push_back(key.clone());
        self.entries.insert(key, data);
    }

    pub(super) fn clear(&mut self) {
        self.entries.clear();
        self.entries.shrink_to_fit();
        self.insertion_order.clear();
        self.insertion_order.shrink_to_fit();
        self.bytes = 0;
    }
}

fn encoded_size_error() -> String {
    format!("Icon PNG exceeds the encoded size limit ({MAX_PNG_BYTES} bytes)")
}

fn read_png(input: impl Read) -> Result<RgbaImage, String> {
    let mut bytes = Vec::new();
    input
        .take(MAX_PNG_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    decode_icon_png(&bytes)
}

pub(super) fn read_cached_png(path: &Path) -> Result<RgbaImage, String> {
    let file = super::platform()
        .read_cache_file(path)
        .map_err(|error| error.to_string())?;
    if file.metadata().map_err(|error| error.to_string())?.len() > MAX_PNG_BYTES as u64 {
        return Err(encoded_size_error());
    }
    // Bound actual reads too, including growth after the handle's length check.
    read_png(file)
}

pub(super) fn decode_icon_png(bytes: &[u8]) -> Result<RgbaImage, String> {
    if bytes.len() > MAX_PNG_BYTES {
        return Err(encoded_size_error());
    }
    // Our icon cache contains PNGs only: do not dispatch another decoder by content.
    let mut reader = image::ImageReader::with_format(Cursor::new(bytes), image::ImageFormat::Png);
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(MAX_ICON_SIDE);
    limits.max_image_height = Some(MAX_ICON_SIDE);
    // Internal decoder allocations are best effort, not a process memory guarantee.
    limits.max_alloc = Some(MAX_DECODE_ALLOC);
    reader.limits(limits);
    reader
        .decode()
        .map(|image| image.to_rgba8())
        .map_err(|error| error.to_string())
}

pub(super) fn cached_or_generate(
    memory: &Mutex<IconCache>,
    directory: Option<&Path>,
    key: &str,
    generate: impl FnOnce() -> Result<RgbaImage, String>,
) -> Result<RgbaImage, String> {
    if let Some(image) = memory.lock().unwrap().get(key) {
        return Ok(image);
    }
    let path = directory.map(|directory| directory.join(format!("{key}.png")));
    if let Some(path) = &path {
        if let Ok(image) = read_cached_png(path) {
            memory
                .lock()
                .unwrap()
                .insert(key.to_string(), image.clone());
            return Ok(image);
        }
    }
    // File I/O, decoding and Shell generation run outside the memory-cache lock.
    let image = generate()?;
    if image.width() == 0
        || image.height() == 0
        || image.width() > MAX_ICON_SIDE
        || image.height() > MAX_ICON_SIDE
    {
        return Err("Generated icon dimensions exceed their limit".into());
    }
    if let Some(directory) = directory {
        let _ = write_cached_png(directory, key, &image);
    }
    memory
        .lock()
        .unwrap()
        .insert(key.to_string(), image.clone());
    Ok(image)
}

pub(super) fn write_cached_png(
    directory: &Path,
    key: &str,
    image: &RgbaImage,
) -> std::io::Result<()> {
    let path = directory.join(format!("{key}.png"));
    let temp_id = CACHE_TEMP_ID.fetch_add(1, Ordering::Relaxed);
    let temporary = directory.join(format!("{key}.{}.{}.tmp", std::process::id(), temp_id));
    super::platform().write_preview_cache(&path, &temporary, &mut |file| {
        image
            .write_to(file, image::ImageFormat::Png)
            .map_err(std::io::Error::other)
    })
}

#[cfg(test)]
#[path = "windows_icon_cache/tests/cache.rs"]
mod tests;
