// Frozen benchmark baseline: fcf724874a05fc4c58ab4f6f3093b9bafe13a645:src/platform/windows_icon_cache.rs
use image::RgbaImage;
use std::collections::{HashMap, VecDeque};
use std::io::{Cursor, Read, Write};
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
    entries: HashMap<String, Vec<u8>>,
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

    fn get(&self, key: &str) -> Option<Vec<u8>> {
        self.entries.get(key).cloned()
    }

    // The caller validates the PNG before admitting it. Do not make admission
    // failure an icon failure: a useful icon may simply remain uncached.
    fn insert(&mut self, key: String, data: Vec<u8>) {
        if self.entry_limit == 0 || data.len() > self.byte_limit || data.len() > MAX_PNG_BYTES {
            return;
        }
        if let Some(previous) = self.entries.remove(&key) {
            self.bytes -= previous.len();
            self.insertion_order.retain(|entry| entry != &key);
        }
        while self.entries.len() >= self.entry_limit || self.bytes > self.byte_limit - data.len() {
            let Some(oldest) = self.insertion_order.pop_front() else {
                return;
            };
            if let Some(previous) = self.entries.remove(&oldest) {
                self.bytes -= previous.len();
            }
        }
        self.bytes += data.len();
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

fn read_png(input: impl Read) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    input
        .take(MAX_PNG_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    decode_icon_png(&bytes)?;
    Ok(bytes)
}

pub(super) fn read_cached_png(path: &Path) -> Result<Vec<u8>, String> {
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
    generate: impl FnOnce() -> Result<Vec<u8>, String>,
) -> Result<Vec<u8>, String> {
    if let Some(bytes) = memory.lock().unwrap().get(key) {
        return Ok(bytes);
    }
    let path = directory.map(|directory| directory.join(format!("{key}.png")));
    if let Some(path) = &path {
        if let Ok(bytes) = read_cached_png(path) {
            return Ok(bytes);
        }
    }
    // File I/O, decoding and Shell generation run outside the memory-cache lock.
    let bytes = generate()?;
    decode_icon_png(&bytes)?;
    if let (Some(directory), Some(path)) = (directory, &path) {
        let temp_id = CACHE_TEMP_ID.fetch_add(1, Ordering::Relaxed);
        let temporary = directory.join(format!("{key}.{}.{}.tmp", std::process::id(), temp_id));
        let _ = super::platform()
            .write_preview_cache(path, &temporary, &mut |file| file.write_all(&bytes));
    }
    memory
        .lock()
        .unwrap()
        .insert(key.to_string(), bytes.clone());
    Ok(bytes)
}
