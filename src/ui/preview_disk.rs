//! Compact disk thumbnails for the grid; tooltip previews remain RAM-only.
use gpui::{RenderImage, SvgRenderer};
use image::{Frame, RgbaImage};
use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex, OnceLock, Weak,
    },
};
use oxyde_lib::models::FileStamp as Stamp;

const SIDE: u32 = oxyde_lib::utils::image_preview_decode::POLICY.output_side;
const MAX_BYTES: usize = 24 * 1024;
const GRID_SIDE: u32 = 128;
const MARKER: &[u8; 8] = b"OXYDEpv2";
mod audio_refs;
static TEMP_ID: AtomicU64 = AtomicU64::new(0);

fn generation_lock(key: Option<&Key>) -> Option<Arc<Mutex<()>>> {
    static LOCKS: OnceLock<Mutex<std::collections::HashMap<PathBuf, Weak<Mutex<()>>>>> = OnceLock::new();
    let key = key?;
    let mut locks = LOCKS.get_or_init(Default::default).lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    locks.retain(|_, lock| lock.strong_count() > 0);
    let lock = locks.get(&key.file).and_then(Weak::upgrade).unwrap_or_else(|| Arc::new(Mutex::new(())));
    locks.insert(key.file.clone(), Arc::downgrade(&lock));
    Some(lock)
}

fn read_stamp(path: &Path) -> Option<Stamp> {
    Stamp::from_metadata(&std::fs::metadata(path).ok()?)
}
struct Key {
    source: PathBuf,
    stamp: Stamp,
    file: PathBuf,
    side: u32,
    max_bytes: usize,
    shared_audio: bool,
}
struct Pixels {
    bgra: RgbaImage,
    animated: bool,
    limited: bool,
}
impl Key {
    fn new(source: &Path, kind: &str) -> Option<Self> {
        Self::in_root(source, kind, &oxyde_lib::app_paths::cache_dir()?.join("thumbnails"))
    }
    fn in_root(source: &Path, kind: &str, root: &Path) -> Option<Self> {
        let stamp = read_stamp(source)?;
        let source = std::path::absolute(source).ok()?;
        let side = GRID_SIDE;
        let max_bytes = MAX_BYTES;
        let key = blake3::hash(
            format!(
                "compact-preview-v2|{side}|{kind}|{source:?}|{}|{:?}",
                stamp.length, stamp.modified
            )
            .as_bytes(),
        );
        Some(Self {
            source,
            stamp,
            file: root.join(format!("{}.jpg", key.to_hex())),
            side,
            max_bytes,
            shared_audio: kind.starts_with("grid-audio-cover"),
        })
    }
    fn valid(&self) -> bool {
        read_stamp(&self.source).as_ref() == Some(&self.stamp)
    }
    fn read(&self) -> Option<Pixels> {
        let file = oxyde_lib::platform::platform()
            .read_cache_file(&self.file)
            .or_else(|_| oxyde_lib::platform::platform().read_cache_file(&self.png_file()))
            .ok()?;
        let mut bytes = Vec::new();
        file.take(self.max_bytes as u64 + 1)
            .read_to_end(&mut bytes)
            .ok()?;
        if self.shared_audio {
            bytes = audio_refs::resolve(&self.file, bytes, self.max_bytes)?;
        }
        if bytes.len() > self.max_bytes || bytes.len() < 10 {
            return None;
        }
        let trailer = &bytes[bytes.len() - 10..];
        if &trailer[..8] != MARKER || trailer[8] > 1 || trailer[9] > 1 {
            return None;
        }
        let (animated, limited) = (trailer[8] != 0, trailer[9] != 0);
        let mut reader = image::ImageReader::new(
            std::io::Cursor::new(&bytes[..bytes.len() - 10]),
        ).with_guessed_format().ok()?;
        if !matches!(reader.format(), Some(image::ImageFormat::Jpeg | image::ImageFormat::Png)) { return None; }
        let mut limits = image::Limits::default();
        limits.max_image_width = Some(self.side);
        limits.max_image_height = Some(self.side);
        limits.max_alloc = Some(8 * 1024 * 1024);
        reader.limits(limits);
        let mut bgra = reader.decode().ok()?.to_rgba8();
        for pixel in bgra.pixels_mut() {
            pixel.0.swap(0, 2);
        }
        if !self.valid() {
            return None;
        }
        Some(Pixels {
            bgra,
            animated,
            limited,
        })
    }
    fn png_file(&self) -> PathBuf {
        self.file.with_file_name(format!("{}_large.png", self.file.file_stem().unwrap().to_string_lossy()))
    }
    fn write(&self, pixels: &Pixels) {
        if !self.valid() {
            return;
        }
        let result = (|| -> std::io::Result<()> {
            let mut rgba = pixels.bgra.clone();
            if rgba.width() > SIDE || rgba.height() > SIDE {
                return Err(std::io::Error::other("Oversized shared preview"));
            }
            for pixel in rgba.pixels_mut() {
                pixel.0.swap(0, 2);
            }
            let rgba = if rgba.width() > self.side || rgba.height() > self.side {
                image::DynamicImage::ImageRgba8(rgba)
                    .resize(self.side, self.side, image::imageops::FilterType::Triangle).to_rgba8()
            } else { rgba };
            let transparent = rgba.pixels().any(|pixel| pixel.0[3] != 255);
            let destination = if transparent { self.png_file() } else { self.file.clone() };
            let mut bytes = Vec::new();
            if transparent {
                image::DynamicImage::ImageRgba8(rgba)
                    .write_to(&mut std::io::Cursor::new(&mut bytes), image::ImageFormat::Png)
                    .map_err(std::io::Error::other)?;
            } else {
                let rgb = image::DynamicImage::ImageRgba8(rgba).to_rgb8();
                image::codecs::jpeg::JpegEncoder::new_with_quality(&mut bytes, 75)
                    .encode_image(&image::DynamicImage::ImageRgb8(rgb))
                    .map_err(std::io::Error::other)?;
            }
            bytes.extend_from_slice(MARKER);
            bytes.push(u8::from(pixels.animated));
            bytes.push(u8::from(pixels.limited));
            if bytes.len() > self.max_bytes {
                return Err(std::io::Error::other(
                    "Shared preview exceeds cache entry budget",
                ));
            }
            if !self.valid() {
                return Ok(());
            }
            if self.shared_audio {
                audio_refs::publish(&self.file, &bytes, || self.valid())
            } else {
                publish_cache_bytes(&destination, &bytes)
            }
        })();
        if let Err(error) = result {
            log::warn!("Cannot persist shared preview: {error}");
        }
    }
}

fn publish_cache_bytes(destination: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let temporary = destination.with_file_name(format!("{}.{}.{}.tmp",
        destination.file_stem().unwrap().to_string_lossy(), std::process::id(),
        TEMP_ID.fetch_add(1, Ordering::Relaxed)));
    oxyde_lib::platform::platform().write_preview_cache(destination, &temporary,
        &mut |file| file.write_all(bytes))
}

fn first_frame(image: &RenderImage) -> Option<RgbaImage> {
    let size = image.size(0);
    RgbaImage::from_raw(
        size.width.0 as u32,
        size.height.0 as u32,
        image.as_bytes(0)?.to_vec(),
    )
}

/// All metadata checks and decoding in this function run on a preview worker.
pub(crate) fn load_grid(path: &str, expected: Option<&Stamp>, audio: bool, renderer: &SvgRenderer) -> Result<Arc<RenderImage>, String> {
    let image = checked_grid_image(Path::new(path), expected, || load_grid_pixels(path, audio, renderer))?;
    Ok(if audio { crate::ui::audio_covers::share(image) } else { image })
}

fn checked_grid_image(path: &Path, expected: Option<&Stamp>, load: impl FnOnce() -> Result<RgbaImage, String>) -> Result<Arc<RenderImage>, String> {
    checked_grid_image_with_metadata(expected, || {
        let metadata = std::fs::metadata(path).map_err(|error| error.to_string())?;
        Ok((metadata.len(), Stamp::from_metadata(&metadata)))
    }, load)
}

fn checked_grid_image_with_metadata(
    expected: Option<&Stamp>,
    mut metadata: impl FnMut() -> Result<(u64, Option<Stamp>), String>,
    load: impl FnOnce() -> Result<RgbaImage, String>,
) -> Result<Arc<RenderImage>, String> {
    let before = metadata()?;
    if expected.is_some_and(|expected| Some(expected) != before.1.as_ref()) {
        return Err("Thumbnail source changed since listing".into());
    }
    let pixels = load()?;
    if metadata()? != before {
        return Err("Thumbnail source changed during loading".into());
    }
    if pixels.width() > GRID_SIDE || pixels.height() > GRID_SIDE {
        return Err("Thumbnail exceeds grid pixel envelope".into());
    }
    // Independent of the tooltip budget. The card lease owns this one-frame,
    // at most 128 x 128 image until its last visible use is released.
    Ok(Arc::new(RenderImage::new(vec![Frame::new(pixels)])))
}

fn load_grid_pixels(path: &str, audio: bool, renderer: &SvgRenderer) -> Result<RgbaImage, String> {
    let pdf = Path::new(path)
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("pdf"));
    let key = Key::new(Path::new(path), if audio { "grid-audio-cover-v2" } else if pdf { "grid-pdf" } else { "grid-image" });
    let lease = generation_lock(key.as_ref());
    let _guard = lease.as_ref().map(|lock| lock.lock().unwrap_or_else(std::sync::PoisonError::into_inner));
    let pixels = if let Some(cached) = key.as_ref().and_then(Key::read) {
        cached.bgra
    } else {
        // Reuse only a valid RAM preview; browsing never creates a disk preview.
        let source = if audio {
            let cover = crate::ui::audio_covers::load_thumbnail(path)?;
            first_frame(&cover).ok_or("Missing audio cover frame")?
        } else if let Some(image) = crate::ui::components::tooltip::cached_media_pixels(path, pdf) {
            first_frame(&image).ok_or("Missing cached preview frame")?
        } else if pdf {
            oxyde_lib::platform::platform().pdf_thumbnail(path.into()).map_err(|error| error.to_string())?
        } else {
            first_frame(&crate::ui::image_preview::load_thumbnail(Path::new(path), renderer).map_err(|error| error.to_string())?.image)
                .ok_or("Missing grid preview frame")?
        };
        let pixels = if source.width() > GRID_SIDE || source.height() > GRID_SIDE {
            image::DynamicImage::ImageRgba8(source)
                .resize(GRID_SIDE, GRID_SIDE, image::imageops::FilterType::Triangle).to_rgba8()
        } else { source };
        if let Some(key) = key { key.write(&Pixels { bgra: pixels.clone(), animated: false, limited: false }); }
        pixels
    };
    Ok(pixels)
}

#[cfg(test)]
#[path = "preview_disk/tests/persistence.rs"]
mod tests;

#[cfg(test)]
#[path = "preview_disk/tests/grid_revision.rs"]
mod grid_revision_tests;

#[cfg(test)]
#[path = "preview_disk/tests/audio.rs"]
mod audio_tests;
