use image::RgbaImage;
use std::collections::{HashMap, VecDeque};
use std::fs;
use std::io::{Cursor, Read};
use std::path::Path;
use std::sync::Mutex;

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
    let file = fs::File::open(path).map_err(|error| error.to_string())?;
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
        let _ = fs::create_dir_all(directory);
        let _ = fs::write(path, &bytes);
    }
    memory
        .lock()
        .unwrap()
        .insert(key.to_string(), bytes.clone());
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png(width: u32, height: u32) -> Vec<u8> {
        let image = RgbaImage::from_pixel(width, height, image::Rgba([201, 43, 17, 255]));
        let mut bytes = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(image)
            .write_to(&mut bytes, image::ImageFormat::Png)
            .unwrap();
        bytes.into_inner()
    }

    #[test]
    fn cached_png_rejects_dimensions_over_the_icon_policy() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("icon.png");
        for (width, height, accepted) in [
            (256, 1, true),
            (1, 256, true),
            (257, 1, false),
            (1, 257, false),
        ] {
            fs::write(&path, png(width, height)).unwrap();
            assert_eq!(read_cached_png(&path).is_ok(), accepted, "width {width}");
            assert_eq!(
                decode_icon_png(&fs::read(&path).unwrap()).is_ok(),
                accepted,
                "width {width}"
            );
        }
    }

    #[test]
    fn cached_png_rejects_encoded_bytes_over_one_mib() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("icon.png");
        fs::write(&path, png(2, 1)).unwrap();
        fs::OpenOptions::new()
            .write(true)
            .open(&path)
            .unwrap()
            .set_len(MAX_PNG_BYTES as u64)
            .unwrap();
        assert!(read_cached_png(&path).is_ok());
        fs::OpenOptions::new()
            .write(true)
            .open(&path)
            .unwrap()
            .set_len(MAX_PNG_BYTES as u64 + 1)
            .unwrap();
        assert!(
            read_cached_png(&path).is_err(),
            "Oversized cached PNG was accepted"
        );
    }

    #[test]
    fn cache_does_not_dispatch_a_non_png_decoder() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("icon.png");
        image::DynamicImage::ImageRgba8(RgbaImage::new(2, 1))
            .save_with_format(&path, image::ImageFormat::Bmp)
            .unwrap();
        assert!(
            read_cached_png(&path).is_err(),
            "Non-PNG cached image was accepted"
        );
        assert!(decode_icon_png(&fs::read(&path).unwrap()).is_err());
    }

    #[test]
    fn normal_cached_png_preserves_dimensions_and_pixels() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("icon.png");
        let bytes = png(32, 96);
        fs::write(&path, &bytes).unwrap();
        assert_eq!(read_cached_png(&path).unwrap(), bytes);
        let image = decode_icon_png(&bytes).unwrap();
        assert_eq!((image.width(), image.height()), (32, 96));
        assert!(image.pixels().all(|pixel| pixel.0 == [201, 43, 17, 255]));
    }

    #[test]
    fn reader_stops_after_the_limit_plus_one() {
        let mut input = std::io::repeat(0).take(MAX_PNG_BYTES as u64 + 9);
        let error = read_png(&mut input).unwrap_err();
        assert!(error.contains("encoded size limit"));
        assert_eq!(input.limit(), 8);
    }

    #[test]
    fn reader_accepts_exact_budget_and_rejects_one_extra_byte() {
        let mut bytes = png(2, 1);
        bytes.resize(MAX_PNG_BYTES, 0);
        assert!(read_png(Cursor::new(&bytes)).is_ok());
        bytes.push(0);
        let mut input = Cursor::new(&bytes);
        assert!(read_png(&mut input).is_err());
        assert_eq!(input.position(), MAX_PNG_BYTES as u64 + 1);
        assert!(decode_icon_png(&bytes).is_err());
    }

    #[test]
    fn invalid_disk_cache_is_regenerated_without_returning_stale_bytes() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("entry.png");
        let mut oversized = png(2, 1);
        oversized.resize(MAX_PNG_BYTES + 1, 0);
        let replacement = png(32, 32);
        for invalid in [b"not a PNG".to_vec(), png(257, 1), oversized] {
            fs::write(&path, &invalid).unwrap();
            let memory = Mutex::new(IconCache::default());
            let calls = std::cell::Cell::new(0);
            let output = cached_or_generate(&memory, Some(directory.path()), "entry", || {
                calls.set(calls.get() + 1);
                Ok(replacement.clone())
            })
            .unwrap();
            assert_eq!(calls.get(), 1);
            assert_eq!(output, replacement);
            assert_eq!(read_cached_png(&path).unwrap(), replacement);
        }
    }

    #[test]
    fn valid_disk_cache_avoids_generation() {
        let directory = tempfile::tempdir().unwrap();
        let bytes = png(32, 32);
        fs::write(directory.path().join("entry.png"), &bytes).unwrap();
        let memory = Mutex::new(IconCache::default());
        let output = cached_or_generate(&memory, Some(directory.path()), "entry", || {
            panic!("Valid disk hit must not call the Shell generator")
        })
        .unwrap();
        assert_eq!(output, bytes);
    }

    #[test]
    fn generation_failure_does_not_return_or_publish_a_bad_cache_entry() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("entry.png");
        fs::write(&path, b"bad cache").unwrap();
        let memory = Mutex::new(IconCache::default());
        let error = cached_or_generate(&memory, Some(directory.path()), "entry", || {
            Err("injected generation failure".into())
        })
        .unwrap_err();
        assert_eq!(error, "injected generation failure");
        assert_eq!(fs::read(&path).unwrap(), b"bad cache");
        assert!(memory.lock().unwrap().entries.is_empty());
    }

    #[test]
    fn cache_write_failure_does_not_block_a_valid_icon() {
        let directory = tempfile::tempdir().unwrap();
        let blocked_directory = directory.path().join("regular_file");
        fs::write(&blocked_directory, b"independent file").unwrap();
        let memory = Mutex::new(IconCache::default());
        let bytes = png(32, 32);
        let output = cached_or_generate(&memory, Some(&blocked_directory), "entry", || {
            Ok(bytes.clone())
        })
        .unwrap();
        assert_eq!(output, bytes);
        assert_eq!(fs::read(&blocked_directory).unwrap(), b"independent file");
        assert_eq!(
            cached_or_generate(&memory, Some(&blocked_directory), "entry", || panic!(
                "In-memory hit must remain available"
            ))
            .unwrap(),
            bytes
        );
    }

    #[test]
    fn memory_evicts_oldest_entries_on_count_or_byte_pressure() {
        let bytes = png(32, 32);
        for (entry_limit, byte_limit) in [(2, usize::MAX), (10, bytes.len() * 2)] {
            let memory = Mutex::new(IconCache::new(entry_limit, byte_limit));
            let calls = std::cell::Cell::new(0);
            for key in ["one", "two", "three"] {
                assert_eq!(
                    cached_or_generate(&memory, None, key, || {
                        calls.set(calls.get() + 1);
                        Ok(bytes.clone())
                    })
                    .unwrap(),
                    bytes
                );
            }
            {
                let cache = memory.lock().unwrap();
                assert_eq!(cache.entries.len(), 2);
                assert_eq!(cache.bytes, bytes.len() * 2);
                assert!(!cache.entries.contains_key("one"));
            }
            assert_eq!(
                cached_or_generate(&memory, None, "two", || panic!(
                    "Remaining icon must be cached"
                ))
                .unwrap(),
                bytes
            );
            assert_eq!(
                cached_or_generate(&memory, None, "one", || {
                    calls.set(calls.get() + 1);
                    Ok(bytes.clone())
                })
                .unwrap(),
                bytes
            );
            assert_eq!(calls.get(), 4);
        }
    }

    #[test]
    fn replacement_accounts_bytes_once_and_purge_resets_the_cache() {
        let old = png(1, 1);
        let replacement = png(32, 32);
        let mut cache = IconCache::new(2, replacement.len() * 2);
        cache.insert("same".into(), old);
        cache.insert("same".into(), replacement.clone());
        assert_eq!(cache.bytes, replacement.len());
        assert_eq!(cache.entries.len(), 1);
        assert_eq!(cache.insertion_order.len(), 1);
        cache.insert("other".into(), replacement.clone());
        assert_eq!(cache.bytes, replacement.len() * 2);
        cache.clear();
        assert_eq!(cache.bytes, 0);
        assert!(cache.entries.is_empty() && cache.insertion_order.is_empty());
        cache.insert("next".into(), replacement.clone());
        assert_eq!(cache.get("next").unwrap(), replacement);
    }

    #[test]
    fn icon_larger_than_the_memory_budget_remains_displayable_uncached() {
        let memory = Mutex::new(IconCache::new(10, 1));
        let bytes = png(32, 32);
        let calls = std::cell::Cell::new(0);
        for _ in 0..2 {
            assert_eq!(
                cached_or_generate(&memory, None, "entry", || {
                    calls.set(calls.get() + 1);
                    Ok(bytes.clone())
                })
                .unwrap(),
                bytes
            );
        }
        assert_eq!(calls.get(), 2);
        assert!(memory.lock().unwrap().entries.is_empty());
    }

    #[test]
    fn inadmissible_generator_output_is_not_published() {
        let directory = tempfile::tempdir().unwrap();
        let memory = Mutex::new(IconCache::default());
        assert!(
            cached_or_generate(&memory, Some(directory.path()), "entry", || Ok(png(257, 1)))
                .is_err()
        );
        assert!(!directory.path().join("entry.png").exists());
        assert!(memory.lock().unwrap().entries.is_empty());
    }
}
