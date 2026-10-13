//! Compact disk renditions: small grid thumbnails and on-demand tooltip previews.
use crate::ui::image_preview::{PreviewError, PreviewImage};
use gpui::{RenderImage, SvgRenderer};
use image::{Frame, RgbaImage};
use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex, OnceLock, Weak,
    },
    time::SystemTime,
};

const SIDE: u32 = oxyde_lib::utils::image_preview_decode::POLICY.output_side;
const MAX_BYTES: usize = 160 * 1024;
const GRID_SIDE: u32 = 128;
const GRID_BYTES: usize = 24 * 1024;
const MARKER: &[u8; 8] = b"OXYDEpv2";
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

#[derive(Clone, PartialEq, Eq)]
struct Stamp {
    length: u64,
    modified: SystemTime,
}
impl Stamp {
    fn read(path: &Path) -> Option<Self> {
        let metadata = std::fs::metadata(path).ok()?;
        Some(Self {
            length: metadata.len(),
            modified: metadata.modified().ok()?,
        })
    }
}
struct Key {
    source: PathBuf,
    stamp: Stamp,
    file: PathBuf,
    side: u32,
    max_bytes: usize,
}
struct Pixels {
    bgra: RgbaImage,
    animated: bool,
    limited: bool,
}
impl Key {
    fn new(source: &Path, kind: &str) -> Option<Self> {
        Self::in_root(source, kind, &dirs::cache_dir()?.join("oxyde/thumbnails"))
    }
    fn in_root(source: &Path, kind: &str, root: &Path) -> Option<Self> {
        let stamp = Stamp::read(source)?;
        let source = std::path::absolute(source).ok()?;
        let grid = kind.starts_with("grid-");
        let side = if grid { GRID_SIDE } else { 768 };
        let max_bytes = if grid { GRID_BYTES } else { MAX_BYTES };
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
        })
    }
    fn valid(&self) -> bool {
        Stamp::read(&self.source).as_ref() == Some(&self.stamp)
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
            let temporary = destination.with_file_name(format!(
                "{}.{}.{}.tmp",
                destination.file_stem().unwrap().to_string_lossy(),
                std::process::id(),
                TEMP_ID.fetch_add(1, Ordering::Relaxed)
            ));
            oxyde_lib::platform::platform().write_preview_cache(
                &destination,
                &temporary,
                &mut |file| file.write_all(&bytes),
            )
        })();
        if let Err(error) = result {
            log::warn!("Cannot persist shared preview: {error}");
        }
    }
}

fn render(pixels: RgbaImage) -> Result<Arc<RenderImage>, String> {
    let image = Arc::new(RenderImage::new(vec![Frame::new(pixels)]));
    crate::ui::preview_budget::admit(&image)?;
    Ok(image)
}
fn first_frame(image: &RenderImage) -> Option<RgbaImage> {
    let size = image.size(0);
    RgbaImage::from_raw(
        size.width.0 as u32,
        size.height.0 as u32,
        image.as_bytes(0)?.to_vec(),
    )
}

pub(crate) fn load_image(
    path: &Path,
    renderer: &SvgRenderer,
) -> Result<(PreviewImage, bool), PreviewError> {
    let key = Key::new(path, "image");
    let lease = generation_lock(key.as_ref());
    let _guard = lease.as_ref().map(|lock| lock.lock().unwrap_or_else(std::sync::PoisonError::into_inner));
    if let Some(pixels) = key
        .as_ref()
        .and_then(Key::read)
        .filter(|pixels| !pixels.animated || pixels.limited)
    {
        log::debug!("Shared image preview disk cache hit: {}", path.display());
        return Ok((
            PreviewImage {
                image: render(pixels.bgra).map_err(PreviewError::Memory)?,
                notice: pixels
                    .limited
                    .then_some(crate::ui::i18n::Message::PreviewAnimationStatic),
            },
            true,
        ));
    }
    let preview = crate::ui::image_preview::load_path(path, renderer)?;
    if let (Some(key), Some(bgra)) = (key, first_frame(&preview.image)) {
        key.write(&Pixels {
            bgra,
            animated: preview.image.frame_count() > 1 || preview.notice.is_some(),
            limited: preview.notice.is_some(),
        });
    }
    Ok((preview, false))
}

pub(crate) fn load_pdf(path: &str) -> Result<(RgbaImage, bool), String> {
    let key = Key::new(Path::new(path), "pdf");
    let lease = generation_lock(key.as_ref());
    let _guard = lease.as_ref().map(|lock| lock.lock().unwrap_or_else(std::sync::PoisonError::into_inner));
    if let Some(pixels) = key.as_ref().and_then(Key::read) {
        log::debug!("Shared PDF preview disk cache hit: {path}");
        return Ok((pixels.bgra, true));
    }
    let bgra = oxyde_lib::platform::platform()
        .pdf_thumbnail(path.into())
        .map_err(|error| error.to_string())?;
    if let Some(key) = key {
        key.write(&Pixels {
            bgra: bgra.clone(),
            animated: false,
            limited: false,
        });
    }
    Ok((bgra, false))
}

pub(crate) fn load_grid(path: &str, renderer: &SvgRenderer) -> Result<Arc<RenderImage>, String> {
    let pdf = Path::new(path)
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("pdf"));
    let key = Key::new(Path::new(path), if pdf { "grid-pdf" } else { "grid-image" });
    let lease = generation_lock(key.as_ref());
    let _guard = lease.as_ref().map(|lock| lock.lock().unwrap_or_else(std::sync::PoisonError::into_inner));
    let pixels = if let Some(cached) = key.as_ref().and_then(Key::read) {
        cached.bgra
    } else {
        // Reuse a large cache if present, but never generate/persist it for the grid.
        let source = if let Some(cached) = Key::new(Path::new(path), if pdf { "pdf" } else { "image" }).and_then(|key| key.read()) {
            cached.bgra
        } else if pdf {
            oxyde_lib::platform::platform().pdf_thumbnail(path.into()).map_err(|error| error.to_string())?
        } else {
            first_frame(&crate::ui::image_preview::load_path(Path::new(path), renderer).map_err(|error| error.to_string())?.image)
                .ok_or("Missing grid preview frame")?
        };
        let pixels = if source.width() > GRID_SIDE || source.height() > GRID_SIDE {
            image::DynamicImage::ImageRgba8(source)
                .resize(GRID_SIDE, GRID_SIDE, image::imageops::FilterType::Triangle).to_rgba8()
        } else { source };
        if let Some(key) = key { key.write(&Pixels { bgra: pixels.clone(), animated: false, limited: false }); }
        pixels
    };
    // Keep the existing grid asset lifetime separate from the bounded tooltip
    // budget: retained grid thumbnails must not consume its 256 preview slots.
    Ok(Arc::new(RenderImage::new(vec![Frame::new(pixels)])))
}

#[cfg(test)]
#[path = "preview_disk/tests/persistence.rs"]
mod tests;
