use crate::models::CommandError;
use image::RgbaImage;
use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

/// Target thumbnail size in pixels (longest side).
const THUMB_SIZE: u32 = 256;
const VIDEO_THUMB_SIZE: u32 = 640;
static CACHE_TEMP_ID: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Copy)]
struct ThumbnailDecodeLimits {
    encoded_bytes: u64,
    side: u32,
    alloc_bytes: u64,
}

// Cache entries are our small, generated thumbnails, not full source images.
const CACHE_DECODE_LIMITS: ThumbnailDecodeLimits = ThumbnailDecodeLimits {
    encoded_bytes: 4 * 1024 * 1024,
    side: 1024,
    alloc_bytes: 8 * 1024 * 1024,
};
const SOURCE_DECODE_LIMITS: ThumbnailDecodeLimits = ThumbnailDecodeLimits {
    encoded_bytes: 64 * 1024 * 1024,
    side: 32768,
    alloc_bytes: 128 * 1024 * 1024,
};
const OFFICE_DECODE_LIMITS: ThumbnailDecodeLimits = ThumbnailDecodeLimits {
    encoded_bytes: 8 * 1024 * 1024,
    side: 4096,
    alloc_bytes: 64 * 1024 * 1024,
};

fn encoded_thumbnail_limit_error(policy: ThumbnailDecodeLimits) -> CommandError {
    CommandError::Other(format!(
        "Thumbnail image exceeds the encoded size limit ({} bytes)",
        policy.encoded_bytes
    ))
}

fn decode_thumbnail_image(
    input: impl std::io::Read,
    policy: ThumbnailDecodeLimits,
    format: Option<image::ImageFormat>,
) -> Result<image::DynamicImage, CommandError> {
    use std::io::Read;
    let mut data = Vec::new();
    // Bound actual bytes read as well as metadata: the source may grow after opening,
    // and Office readers decompress ZIP entries with no filesystem length to trust.
    input
        .take(policy.encoded_bytes + 1)
        .read_to_end(&mut data)?;
    if data.len() as u64 > policy.encoded_bytes {
        return Err(encoded_thumbnail_limit_error(policy));
    }
    let mut reader = image::ImageReader::new(std::io::Cursor::new(data));
    if let Some(format) = format {
        reader.set_format(format);
    } else {
        reader = reader.with_guessed_format()?;
    }
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(policy.side);
    limits.max_image_height = Some(policy.side);
    // image enforces output size, but internal decoder allocations are best effort.
    // This is not a bound on process memory, especially with concurrent previews.
    limits.max_alloc = Some(policy.alloc_bytes);
    reader.limits(limits);
    reader
        .decode()
        .map_err(|error| CommandError::Other(error.to_string()))
}

fn load_thumbnail_image(
    path: &Path,
    policy: ThumbnailDecodeLimits,
) -> Result<image::DynamicImage, CommandError> {
    let file = fs::File::open(path)?;
    if file.metadata()?.len() > policy.encoded_bytes {
        return Err(encoded_thumbnail_limit_error(policy));
    }
    // Keep image::open's extension-based dispatch, especially JPEG-only cache files.
    let format = image::ImageFormat::from_path(path)
        .map_err(|error| CommandError::Other(error.to_string()))?;
    decode_thumbnail_image(file, policy, Some(format))
}

fn load_cached_thumbnail(path: &Path) -> Result<image::DynamicImage, CommandError> {
    let file = super::platform().read_cache_file(path)?;
    if file.metadata()?.len() > CACHE_DECODE_LIMITS.encoded_bytes {
        return Err(encoded_thumbnail_limit_error(CACHE_DECODE_LIMITS));
    }
    decode_thumbnail_image(file, CACHE_DECODE_LIMITS, Some(image::ImageFormat::Jpeg))
}

fn load_source_thumbnail(path: &Path) -> Result<image::DynamicImage, CommandError> {
    load_thumbnail_image(path, SOURCE_DECODE_LIMITS)
}

#[cfg(test)]
mod thumbnail_decode_security_tests {
    use super::*;

    fn write_image(path: &Path, width: u32, height: u32, format: image::ImageFormat) {
        image::DynamicImage::ImageRgba8(RgbaImage::new(width, height))
            .save_with_format(path, format)
            .unwrap();
    }

    #[test]
    fn cached_thumbnail_dimension_limit_accepts_boundary_and_rejects_next_pixel() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("cache.jpg");
        for (width, height, accepted) in [
            (1024, 1, true),
            (1, 1024, true),
            (1025, 1, false),
            (1, 1025, false),
        ] {
            write_image(&path, width, height, image::ImageFormat::Jpeg);
            assert_eq!(
                load_cached_thumbnail(&path).is_ok(),
                accepted,
                "width {width}"
            );
        }
    }

    #[test]
    fn source_thumbnail_dimension_limit_accepts_boundary_and_rejects_next_pixel() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("source.png");
        for (width, height, accepted) in [
            (32768, 1, true),
            (1, 32768, true),
            (32769, 1, false),
            (1, 32769, false),
        ] {
            write_image(&path, width, height, image::ImageFormat::Png);
            assert_eq!(
                load_source_thumbnail(&path).is_ok(),
                accepted,
                "width {width}"
            );
        }
    }

    #[test]
    fn cached_thumbnail_encoded_limit_accepts_boundary_and_rejects_next_byte() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("cache.jpg");
        write_image(&path, 2, 1, image::ImageFormat::Jpeg);
        fs::OpenOptions::new()
            .write(true)
            .open(&path)
            .unwrap()
            .set_len(CACHE_DECODE_LIMITS.encoded_bytes)
            .unwrap();
        assert!(load_cached_thumbnail(&path).is_ok());
        fs::OpenOptions::new()
            .write(true)
            .open(&path)
            .unwrap()
            .set_len(CACHE_DECODE_LIMITS.encoded_bytes + 1)
            .unwrap();
        let error = load_cached_thumbnail(&path).unwrap_err();
        assert!(error.to_string().contains("encoded size limit"));
    }

    #[test]
    fn source_thumbnail_rejects_oversized_open_file_before_format_detection() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("source.png");
        fs::File::create(&path)
            .unwrap()
            .set_len(SOURCE_DECODE_LIMITS.encoded_bytes + 1)
            .unwrap();
        // A file of zeros would fail format detection: this error comes from admission.
        let error = load_source_thumbnail(&path).unwrap_err();
        assert!(error.to_string().contains("encoded size limit"));
    }

    #[test]
    fn thumbnail_reader_stops_after_encoded_limit_plus_one() {
        use std::io::Read;
        let mut input = std::io::repeat(0).take(1000);
        let policy = ThumbnailDecodeLimits {
            encoded_bytes: 16,
            ..CACHE_DECODE_LIMITS
        };
        let error = decode_thumbnail_image(&mut input, policy, None).unwrap_err();
        assert!(error.to_string().contains("encoded size limit"));
        assert_eq!(input.limit(), 1000 - 17);
    }

    #[test]
    fn valid_thumbnail_reader_accepts_exact_budget_and_rejects_appended_byte() {
        let mut data = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(RgbaImage::new(2, 1))
            .write_to(&mut data, image::ImageFormat::Png)
            .unwrap();
        let mut data = data.into_inner();
        let policy = ThumbnailDecodeLimits {
            encoded_bytes: data.len() as u64,
            ..CACHE_DECODE_LIMITS
        };
        assert!(decode_thumbnail_image(std::io::Cursor::new(&data), policy, None).is_ok());
        data.push(0);
        assert!(decode_thumbnail_image(std::io::Cursor::new(&data), policy, None).is_err());
    }

    #[test]
    fn thumbnail_decode_rejects_output_over_allocation_budget() {
        let mut data = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(RgbaImage::new(128, 128))
            .write_to(&mut data, image::ImageFormat::Png)
            .unwrap();
        let data = data.into_inner();
        assert!(
            decode_thumbnail_image(std::io::Cursor::new(&data), CACHE_DECODE_LIMITS, None).is_ok()
        );
        let policy = ThumbnailDecodeLimits {
            alloc_bytes: 32 * 1024,
            ..CACHE_DECODE_LIMITS
        };
        assert!(decode_thumbnail_image(std::io::Cursor::new(&data), policy, None).is_err());
    }

    #[test]
    fn cache_and_source_do_not_decode_a_different_format_than_the_extension() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("cache.jpg");
        write_image(&path, 2, 1, image::ImageFormat::Png);
        assert!(load_cached_thumbnail(&path).is_err());
        assert!(load_source_thumbnail(&path).is_err());
    }

    #[test]
    fn normal_source_and_cache_images_preserve_dimensions_and_pixels() {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("source.png");
        let cache = directory.path().join("cache.jpg");
        let image = RgbaImage::from_pixel(512, 128, image::Rgba([201, 43, 17, 255]));
        image::DynamicImage::ImageRgba8(image.clone())
            .save(&source)
            .unwrap();
        let decoded = load_source_thumbnail(&source).unwrap().to_rgba8();
        assert_eq!(decoded, image);
        let thumbnail = image::DynamicImage::ImageRgba8(decoded).thumbnail(THUMB_SIZE, THUMB_SIZE);
        assert_eq!((thumbnail.width(), thumbnail.height()), (256, 64));
        thumbnail.save(&cache).unwrap();
        let decoded = load_cached_thumbnail(&cache).unwrap();
        assert_eq!((decoded.width(), decoded.height()), (256, 64));
        let pixel = decoded.to_rgba8().get_pixel(0, 0).0;
        assert!(pixel[0] > 190 && pixel[1] < 55 && pixel[2] < 30);
    }
}

fn cached_thumbnail(
    path: String,
    kind: &str,
    generate: impl FnOnce(String) -> Result<RgbaImage, CommandError>,
) -> Result<RgbaImage, CommandError> {
    let source = Path::new(&path);
    let metadata =
        fs::metadata(source).map_err(|error| CommandError::IoError(error.to_string()))?;
    let modified_ns = metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |duration| duration.as_nanos());
    let Some(cache_root) = dirs::cache_dir() else {
        log::warn!("Cannot resolve the user cache directory; thumbnail will not be cached");
        return generate(path);
    };
    let cache_dir = cache_root.join("oxyde").join("thumbnails");
    let key =
        blake3::hash(format!("v1|{kind}|{}|{}|{modified_ns}", path, metadata.len()).as_bytes())
            .to_hex();
    let cache_file = cache_dir.join(format!("{key}.jpg"));

    cached_thumbnail_in(&cache_file, || {
        generate(path.clone()).map_err(|error| {
            log::warn!("Could not generate {kind} thumbnail for {path}: {error}");
            error
        })
    })
}

fn cached_thumbnail_in(
    cache_file: &Path,
    generate: impl FnOnce() -> Result<RgbaImage, CommandError>,
) -> Result<RgbaImage, CommandError> {
    if let Ok(image) = load_cached_thumbnail(cache_file) {
        return Ok(image.to_rgba8());
    }
    // Persistence is optional. Keep a generated image usable even when its cache
    // directory cannot be created, encoded, synchronized or published.
    let image = generate()?;
    let write_result = (|| -> Result<(), CommandError> {
        let cache_dir = cache_file.parent().ok_or_else(|| {
            CommandError::Other("Thumbnail cache has no parent directory".to_string())
        })?;
        let temp_id = CACHE_TEMP_ID.fetch_add(1, Ordering::Relaxed);
        let name = cache_file.file_stem().unwrap_or_default().to_string_lossy();
        let temp_file = cache_dir.join(format!("{name}.{}.{}.tmp", std::process::id(), temp_id));
        write_thumbnail_cache(cache_file, &temp_file, &image)
    })();
    if let Err(error) = write_result {
        log::warn!(
            "Could not cache thumbnail at {}: {error}",
            cache_file.display()
        );
    }
    Ok(image)
}

fn write_thumbnail_cache(
    cache_file: &Path,
    temp_file: &Path,
    image: &RgbaImage,
) -> Result<(), CommandError> {
    super::platform().write_preview_cache(cache_file, temp_file, &mut |file| {
        let rgb = image::DynamicImage::ImageRgba8(image.clone()).to_rgb8();
        image::codecs::jpeg::JpegEncoder::new_with_quality(file, 75)
            .encode_image(&image::DynamicImage::ImageRgb8(rgb))
            .map_err(std::io::Error::other)
    })?;
    Ok(())
}

#[cfg(test)]
mod thumbnail_cache_write_tests {
    use super::*;

    fn generated_image() -> RgbaImage {
        RgbaImage::from_pixel(32, 16, image::Rgba([201, 43, 17, 255]))
    }

    #[test]
    fn unavailable_cache_directory_does_not_prevent_generation() {
        let root = tempfile::tempdir().unwrap();
        let blocked = root.path().join("regular_file");
        fs::write(&blocked, b"independent directory marker").unwrap();
        let calls = std::cell::Cell::new(0);
        let expected = generated_image();
        let result = cached_thumbnail_in(&blocked.join("entry.jpg"), || {
            calls.set(calls.get() + 1);
            Ok(expected.clone())
        });
        assert_eq!(calls.get(), 1, "Caching prevented thumbnail generation");
        assert_eq!(result.unwrap(), expected);
        assert_eq!(fs::read(&blocked).unwrap(), b"independent directory marker");
    }

    #[test]
    fn persistence_failure_keeps_the_generated_image() {
        let root = tempfile::tempdir().unwrap();
        let cache_dir = root.path().join("cache");
        fs::create_dir(&cache_dir).unwrap();
        let expected = generated_image();
        let result = cached_thumbnail_in(&cache_dir.join("entry.jpg"), || {
            // Change only this fixture after a valid image has been generated.
            fs::remove_dir(&cache_dir).unwrap();
            fs::write(&cache_dir, b"independent directory marker").unwrap();
            Ok(expected.clone())
        });
        assert_eq!(result.unwrap(), expected);
        assert_eq!(
            fs::read(&cache_dir).unwrap(),
            b"independent directory marker"
        );
    }

    #[test]
    fn exclusive_creation_collision_preserves_the_existing_file() {
        let root = tempfile::tempdir().unwrap();
        let temp = root.path().join("entry.tmp");
        let destination = root.path().join("entry.jpg");
        fs::write(&temp, b"independent staging marker").unwrap();
        assert!(write_thumbnail_cache(&destination, &temp, &generated_image()).is_err());
        assert_eq!(fs::read(&temp).unwrap(), b"independent staging marker");
        assert!(!destination.exists());
    }

    #[test]
    fn normal_cache_hit_avoids_generation_and_preserves_pixels() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("entry.jpg");
        let expected = generated_image();
        assert_eq!(
            cached_thumbnail_in(&path, || Ok(expected.clone())).unwrap(),
            expected
        );
        let bytes = fs::read(&path).unwrap();
        let image =
            cached_thumbnail_in(&path, || panic!("Valid cache hit generated again")).unwrap();
        assert_eq!(image.dimensions(), (32, 16));
        let pixel = image.get_pixel(0, 0).0;
        assert!(pixel[0] > 190 && pixel[1] < 55 && pixel[2] < 30);
        assert_eq!(fs::read(path).unwrap(), bytes);
    }

    #[test]
    fn generation_failure_preserves_invalid_cache_until_replacement() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("entry.jpg");
        fs::write(&path, b"invalid cache marker").unwrap();
        let error = cached_thumbnail_in(&path, || {
            Err(CommandError::Other("injected generation failure".into()))
        })
        .unwrap_err();
        assert!(error.to_string().contains("injected generation failure"));
        assert_eq!(fs::read(&path).unwrap(), b"invalid cache marker");
        assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
        let expected = generated_image();
        assert_eq!(
            cached_thumbnail_in(&path, || Ok(expected.clone())).unwrap(),
            expected
        );
        assert_eq!(
            load_cached_thumbnail(&path)
                .unwrap()
                .to_rgba8()
                .dimensions(),
            (32, 16)
        );
        assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
    }

    #[test]
    fn unavailable_cache_does_not_hide_a_generation_error() {
        let root = tempfile::tempdir().unwrap();
        let blocked = root.path().join("regular_file");
        fs::write(&blocked, b"independent directory marker").unwrap();
        let error = cached_thumbnail_in(&blocked.join("entry.jpg"), || {
            Err(CommandError::Other("injected generation failure".into()))
        })
        .unwrap_err();
        assert!(error.to_string().contains("injected generation failure"));
        assert_eq!(fs::read(&blocked).unwrap(), b"independent directory marker");
    }

    #[test]
    fn failed_publication_cleans_only_the_created_staging_file() {
        let root = tempfile::tempdir().unwrap();
        let destination = root.path().join("entry.jpg");
        fs::create_dir(&destination).unwrap();
        fs::write(
            destination.join("marker"),
            b"independent destination marker",
        )
        .unwrap();
        let unrelated = root.path().join("other.tmp");
        fs::write(&unrelated, b"independent staging marker").unwrap();
        let temp = root.path().join("entry.tmp");
        assert!(write_thumbnail_cache(&destination, &temp, &generated_image()).is_err());
        assert!(!temp.exists());
        assert_eq!(fs::read(unrelated).unwrap(), b"independent staging marker");
        assert_eq!(
            fs::read(destination.join("marker")).unwrap(),
            b"independent destination marker"
        );
    }

    #[test]
    fn failed_publication_still_returns_the_generated_thumbnail() {
        let root = tempfile::tempdir().unwrap();
        let destination = root.path().join("entry.jpg");
        fs::create_dir(&destination).unwrap();
        fs::write(
            destination.join("marker"),
            b"independent destination marker",
        )
        .unwrap();
        let expected = generated_image();
        assert_eq!(
            cached_thumbnail_in(&destination, || Ok(expected.clone())).unwrap(),
            expected
        );
        assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
        assert_eq!(
            fs::read(destination.join("marker")).unwrap(),
            b"independent destination marker"
        );
    }

    #[test]
    fn missing_source_metadata_still_reports_an_error() {
        let root = tempfile::tempdir().unwrap();
        assert!(cached_thumbnail(
            root.path()
                .join("missing.png")
                .to_string_lossy()
                .into_owned(),
            "image",
            |_| { panic!("Missing source must not generate a thumbnail or access the user cache") }
        )
        .is_err());
        assert_eq!(fs::read_dir(root.path()).unwrap().count(), 0);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn cache_junction_does_not_publish_into_its_target() {
        let root = tempfile::tempdir().unwrap();
        let outside = root.path().join("outside");
        fs::create_dir(&outside).unwrap();
        fs::write(outside.join("entry.jpg"), b"independent destination marker").unwrap();
        let cache = root.path().join("cache");
        crate::platform::windows_reparse_tests::junction(&cache, &outside);
        let result = write_thumbnail_cache(
            &cache.join("entry.jpg"),
            &cache.join("entry.tmp"),
            &generated_image(),
        );
        let bytes = fs::read(outside.join("entry.jpg")).unwrap();
        fs::remove_dir(&cache).unwrap();
        assert!(result.is_err(), "Cache publication followed a junction");
        assert_eq!(bytes, b"independent destination marker");
        assert_eq!(fs::read_dir(&outside).unwrap().count(), 1);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn cache_junction_does_not_supply_a_thumbnail() {
        let root = tempfile::tempdir().unwrap();
        let outside = root.path().join("outside");
        fs::create_dir(&outside).unwrap();
        write_thumbnail_cache(
            &outside.join("entry.jpg"),
            &outside.join("entry.tmp"),
            &generated_image(),
        )
        .unwrap();
        let cache = root.path().join("cache");
        crate::platform::windows_reparse_tests::junction(&cache, &outside);
        let result = load_cached_thumbnail(&cache.join("entry.jpg"));
        fs::remove_dir(&cache).unwrap();
        assert!(result.is_err(), "Cache read followed a junction");
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn occupied_disk_quota_lease_keeps_the_generated_thumbnail_usable() {
        let root = tempfile::tempdir().unwrap();
        let lease = root.path().join(".preview-cache-lock");
        fs::write(&lease, b"lease witness").unwrap();
        let reader = fs::File::open(&lease).unwrap();
        let destination = root.path().join(format!("{:064x}.jpg", 1));
        let expected = generated_image();
        assert_eq!(
            cached_thumbnail_in(&destination, || Ok(expected.clone())).unwrap(),
            expected
        );
        assert!(!destination.exists());
        assert_eq!(fs::read(&lease).unwrap(), b"lease witness");
        assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
        drop(reader);
    }
}

pub fn get_thumbnail_raw(path: String) -> Result<RgbaImage, CommandError> {
    #[cfg(target_os = "windows")]
    return isolated_thumbnail("cached-thumbnail", &path);
    #[cfg(not(target_os = "windows"))]
    cached_thumbnail(path, "image", get_thumbnail_uncached)
}

pub fn get_office_thumbnail_raw(path: String) -> Result<RgbaImage, CommandError> {
    #[cfg(target_os = "windows")]
    return isolated_thumbnail("cached-office", &path);
    #[cfg(not(target_os = "windows"))]
    cached_thumbnail(path, "office", get_office_thumbnail_uncached)
}

#[cfg(target_os = "windows")]
fn get_thumbnail_uncached(path: String) -> Result<RgbaImage, CommandError> {
    match shell_thumbnail_raw(&path) {
        Ok(image) => Ok(image),
        Err(shell_error) => match load_source_thumbnail(Path::new(&path)) {
            Ok(image) => {
                let mut image = image.thumbnail(THUMB_SIZE, THUMB_SIZE).to_rgba8();
                // RenderImage consumes BGRA buffers; the image decoder returns RGBA.
                for pixel in image.pixels_mut() {
                    pixel.0.swap(0, 2);
                }
                Ok(image)
            }
            Err(decode_error) => Err(CommandError::Other(format!(
                "Shell thumbnail failed ({shell_error}); image decoding failed ({decode_error})"
            ))),
        },
    }
}

#[cfg(not(target_os = "windows"))]
fn get_thumbnail_uncached(path: String) -> Result<RgbaImage, CommandError> {
    load_source_thumbnail(Path::new(&path))
        .map(|image| image.thumbnail(THUMB_SIZE, THUMB_SIZE).to_rgba8())
}

#[cfg(not(target_os = "windows"))]
fn get_office_thumbnail_uncached(path: String) -> Result<RgbaImage, CommandError> {
    let file = fs::File::open(path).map_err(|error| CommandError::IoError(error.to_string()))?;
    let mut archive =
        zip::ZipArchive::new(file).map_err(|error| CommandError::Other(error.to_string()))?;
    for name in ["docProps/thumbnail.jpeg", "Thumbnails/thumbnail.png"] {
        if let Ok(entry) = archive.by_name(name) {
            return decode_office_thumbnail(entry);
        }
    }
    Err(CommandError::Other("No office thumbnail".to_string()))
}

#[cfg(not(target_os = "windows"))]
pub fn get_pdf_thumbnail_raw(_path: String) -> Result<RgbaImage, CommandError> {
    Err(CommandError::Other(
        "PDF thumbnail is unavailable on this platform".to_string(),
    ))
}

#[cfg(not(target_os = "windows"))]
pub fn stream_video_frames_sync<F>(
    _path: String,
    _on_frame: F,
    _cancelled: impl Fn() -> bool,
) -> Result<(), CommandError>
where
    F: FnMut(RgbaImage, Duration) -> bool,
{
    Err(CommandError::Other(
        "Video preview is unavailable on this platform".to_string(),
    ))
}

#[cfg(target_os = "windows")]
struct MediaFoundationGuard;
#[cfg(target_os = "windows")]
impl MediaFoundationGuard {
    fn new() -> Result<Self, CommandError> {
        unsafe {
            windows::Win32::Media::MediaFoundation::MFStartup(
                windows::Win32::Media::MediaFoundation::MF_SDK_VERSION,
                windows::Win32::Media::MediaFoundation::MFSTARTUP_FULL,
            )
        }
        .map_err(|error| CommandError::SystemError(error.to_string()))?;
        Ok(Self)
    }
}
#[cfg(target_os = "windows")]
impl Drop for MediaFoundationGuard {
    fn drop(&mut self) {
        unsafe {
            let _ = windows::Win32::Media::MediaFoundation::MFShutdown();
        }
    }
}

#[cfg(target_os = "windows")]
enum VideoSample {
    Frame(RgbaImage, Duration),
    Gap,
    End,
}

#[cfg(target_os = "windows")]
fn play_video<R>(
    cancelled: &impl Fn() -> bool,
    mut open: impl FnMut() -> Result<R, CommandError>,
    mut read: impl FnMut(&mut R) -> Result<VideoSample, CommandError>,
    on_frame: &mut impl FnMut(RgbaImage, Duration) -> bool,
    mut pause: impl FnMut(),
) -> Result<(), CommandError> {
    while !cancelled() {
        let mut reader = open()?;
        let mut received_frame = false;
        loop {
            if cancelled() {
                return Ok(());
            }
            let sample = read(&mut reader)?;
            if cancelled() {
                return Ok(());
            }
            match sample {
                VideoSample::Frame(frame, pts) => {
                    received_frame = true;
                    if !on_frame(frame, pts) {
                        return Ok(());
                    }
                }
                VideoSample::Gap => {}
                VideoSample::End => break,
            }
        }
        if !received_frame {
            return Err(CommandError::Other(
                "Video stream ended without a frame".into(),
            ));
        }
        // Release the native reader before waiting or opening the next loop.
        drop(reader);
        for _ in 0..20 {
            if cancelled() {
                return Ok(());
            }
            pause();
        }
    }
    Ok(())
}

#[cfg(target_os = "windows")]
#[derive(Clone, Copy)]
struct VideoLayout {
    width: u32,
    height: u32,
    stride: i32,
    out_w: u32,
    out_h: u32,
}
#[cfg(target_os = "windows")]
impl VideoLayout {
    fn new(width: u32, height: u32, stride: i32) -> Result<Self, CommandError> {
        if width == 0
            || height == 0
            || width > i32::MAX as u32
            || height > i32::MAX as u32
            || stride.checked_abs().is_none()
            || u64::from(stride.unsigned_abs()) < u64::from(width) * 4
        {
            return Err(CommandError::Other(
                "Invalid video frame dimensions or stride".into(),
            ));
        }
        let (out_w, out_h) = if width >= height {
            (
                VIDEO_THUMB_SIZE,
                ((u64::from(VIDEO_THUMB_SIZE) * u64::from(height)) / u64::from(width)).max(1)
                    as u32,
            )
        } else {
            (
                ((u64::from(VIDEO_THUMB_SIZE) * u64::from(width)) / u64::from(height)).max(1)
                    as u32,
                VIDEO_THUMB_SIZE,
            )
        };
        Ok(Self {
            width,
            height,
            stride,
            out_w,
            out_h,
        })
    }
    fn from_media_type(
        media: &windows::Win32::Media::MediaFoundation::IMFMediaType,
    ) -> Result<Self, CommandError> {
        use windows::Win32::Media::MediaFoundation::*;
        unsafe {
            if media.GetGUID(&MF_MT_SUBTYPE).map_err(video_error)? != MFVideoFormat_RGB32 {
                return Err(CommandError::Other("Unexpected video output format".into()));
            }
            let size = media.GetUINT64(&MF_MT_FRAME_SIZE).map_err(video_error)?;
            let width = (size >> 32) as u32;
            let height = size as u32;
            let packed = width
                .checked_mul(4)
                .filter(|value| *value <= i32::MAX as u32)
                .ok_or_else(|| CommandError::Other("Video stride overflow".into()))?;
            let stride = match media.GetUINT32(&MF_MT_DEFAULT_STRIDE) {
                Ok(stride) => stride as i32,
                Err(_) => packed as i32,
            };
            Self::new(width, height, stride)
        }
    }
    fn required_bytes(&self) -> Result<usize, CommandError> {
        let bytes = u64::from(self.height - 1) * u64::from(self.stride.unsigned_abs())
            + u64::from(self.width) * 4;
        usize::try_from(bytes).map_err(|_| CommandError::Other("Video frame size overflow".into()))
    }
}
#[cfg(target_os = "windows")]
fn video_error(error: windows::core::Error) -> CommandError {
    CommandError::SystemError(format!("Video decoder: {error}"))
}

#[cfg(target_os = "windows")]
pub fn stream_video_frames_sync<F>(
    path: String,
    mut on_frame: F,
    cancelled: impl Fn() -> bool,
) -> Result<(), CommandError>
where F: FnMut(RgbaImage,Duration) -> bool,
{
    super::platform().stream_preview_worker(Path::new(&path), &mut on_frame, &cancelled)?;
    Ok(())
}

#[cfg(target_os = "windows")]
pub(super) fn stream_video_frames_local<F>(
    path: String,
    mut on_frame: F,
    cancelled: impl Fn() -> bool,
) -> Result<(), CommandError>
where
    F: FnMut(RgbaImage, Duration) -> bool,
{
    use windows::Win32::Media::MediaFoundation::*;
    if cancelled() {
        return Ok(());
    }
    let _com = super::windows_com::ComApartment::mta_or_existing().map_err(video_error)?;
    let _media = MediaFoundationGuard::new()?;
    let wide_path = windows::core::HSTRING::from(path);
    let stream = MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32;
    unsafe {
        play_video(
            &cancelled,
            || {
                let mut attributes = None;
                MFCreateAttributes(&mut attributes, 2).map_err(video_error)?;
                let attributes = attributes
                    .ok_or_else(|| CommandError::Other("Missing video attributes".into()))?;
                attributes
                    .SetUINT32(&MF_SOURCE_READER_ENABLE_VIDEO_PROCESSING, 1)
                    .map_err(video_error)?;
                attributes
                    .SetUINT32(&MF_READWRITE_ENABLE_HARDWARE_TRANSFORMS, 1)
                    .map_err(video_error)?;
                let reader = MFCreateSourceReaderFromURL(
                    windows::core::PCWSTR(wide_path.as_ptr()),
                    &attributes,
                )
                .map_err(video_error)?;
                reader
                    .SetStreamSelection(MF_SOURCE_READER_ALL_STREAMS.0 as u32, false)
                    .map_err(video_error)?;
                reader
                    .SetStreamSelection(stream, true)
                    .map_err(video_error)?;
                let request = MFCreateMediaType().map_err(video_error)?;
                request
                    .SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)
                    .map_err(video_error)?;
                request
                    .SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_RGB32)
                    .map_err(video_error)?;
                reader
                    .SetCurrentMediaType(stream, None, &request)
                    .map_err(video_error)?;
                let layout = VideoLayout::from_media_type(
                    &reader.GetCurrentMediaType(stream).map_err(video_error)?,
                )?;
                Ok((reader, layout))
            },
            |(reader, layout)| {
                let mut flags = 0;
                let mut sample = None;
                reader
                    .ReadSample(stream, 0, None, Some(&mut flags), None, Some(&mut sample))
                    .map_err(video_error)?;
                if flags & MF_SOURCE_READERF_ERROR.0 as u32 != 0 {
                    return Err(CommandError::Other(
                        "Video source reader reported an error".into(),
                    ));
                }
                if flags & MF_SOURCE_READERF_CURRENTMEDIATYPECHANGED.0 as u32 != 0 {
                    *layout = VideoLayout::from_media_type(
                        &reader.GetCurrentMediaType(stream).map_err(video_error)?,
                    )?;
                }
                if flags & MF_SOURCE_READERF_ENDOFSTREAM.0 as u32 != 0 {
                    return Ok(VideoSample::End);
                }
                let Some(sample) = sample else {
                    return Ok(VideoSample::Gap);
                };
                let timestamp = sample.GetSampleTime().map_err(video_error)?;
                let pts = video_timestamp(timestamp);
                let frame = subsample_frame(sample, *layout)?;
                Ok(VideoSample::Frame(frame, pts))
            },
            &mut on_frame,
            || std::thread::sleep(Duration::from_millis(10)),
        )
    }
}
#[cfg(target_os = "windows")]
fn video_timestamp(timestamp: i64) -> Duration {
    // Negative preroll timestamps are displayed at the beginning of the loop.
    let ticks = timestamp.max(0) as u64;
    // Convert in seconds/remainder so long timestamps cannot overflow nanoseconds.
    Duration::new(ticks / 10_000_000, ((ticks % 10_000_000) * 100) as u32)
}

#[cfg(target_os = "windows")]
struct LockedVideoBuffer<'a>(Option<&'a windows::Win32::Media::MediaFoundation::IMFMediaBuffer>);
#[cfg(target_os = "windows")]
impl Drop for LockedVideoBuffer<'_> {
    fn drop(&mut self) {
        unsafe {
            if let Some(buffer) = self.0.take() {
                let _ = buffer.Unlock();
            }
        }
    }
}
#[cfg(target_os = "windows")]
fn subsample_frame(
    sample: windows::Win32::Media::MediaFoundation::IMFSample,
    layout: VideoLayout,
) -> Result<RgbaImage, CommandError> {
    unsafe {
        let buffer = sample.ConvertToContiguousBuffer().map_err(video_error)?;
        let mut ptr = std::ptr::null_mut();
        let mut len = 0;
        buffer
            .Lock(&mut ptr, None, Some(&mut len))
            .map_err(video_error)?;
        let mut lock = LockedVideoBuffer(Some(&buffer));
        if ptr.is_null() || usize::try_from(len).unwrap_or(0) < layout.required_bytes()? {
            return Err(CommandError::Other("Truncated video frame buffer".into()));
        }
        let bytes = std::slice::from_raw_parts(ptr, len as usize);
        let image = resize_video_frame(bytes, layout)?;
        lock.0.take().unwrap().Unlock().map_err(video_error)?;
        Ok(image)
    }
}
#[cfg(target_os = "windows")]
fn resize_video_frame(bytes: &[u8], layout: VideoLayout) -> Result<RgbaImage, CommandError> {
    if bytes.len() < layout.required_bytes()? {
        return Err(CommandError::Other("Truncated video frame buffer".into()));
    }
    let VideoLayout {
        width: src_w,
        height: src_h,
        out_w,
        out_h,
        stride,
    } = layout;
    let pitch = stride.unsigned_abs() as usize;
    let row = |y: usize| {
        if stride < 0 {
            src_h as usize - 1 - y
        } else {
            y
        }
    };
    let mut out = vec![0; (out_w * out_h * 4) as usize];
    for y in 0..out_h as usize {
        let fy = (y as f32 + 0.5) * src_h as f32 / out_h as f32 - 0.5;
        let y0 = (fy as i32).clamp(0, src_h as i32 - 1) as usize;
        let y1 = (y0 + 1).min(src_h as usize - 1);
        let wy1 = (fy - y0 as f32).max(0.0);
        let wy0 = 1.0 - wy1;
        for x in 0..out_w as usize {
            let fx = (x as f32 + 0.5) * src_w as f32 / out_w as f32 - 0.5;
            let x0 = (fx as i32).clamp(0, src_w as i32 - 1) as usize;
            let x1 = (x0 + 1).min(src_w as usize - 1);
            let wx1 = (fx - x0 as f32).max(0.0);
            let wx0 = 1.0 - wx1;
            let dst_off = (y * out_w as usize + x) * 4;
            for c in 0..3 {
                let p00 = bytes[row(y0) * pitch + x0 * 4 + c] as f32;
                let p10 = bytes[row(y0) * pitch + x1 * 4 + c] as f32;
                let p01 = bytes[row(y1) * pitch + x0 * 4 + c] as f32;
                let p11 = bytes[row(y1) * pitch + x1 * 4 + c] as f32;
                out[dst_off + c] =
                    (wy0 * (wx0 * p00 + wx1 * p10) + wy1 * (wx0 * p01 + wx1 * p11)) as u8;
            }
            out[dst_off + 3] = 255;
        }
    }
    RgbaImage::from_raw(out_w, out_h, out)
        .ok_or_else(|| CommandError::Other("Invalid frame dimensions".into()))
}

fn decode_office_thumbnail(reader: impl std::io::Read) -> Result<RgbaImage, CommandError> {
    let image = decode_thumbnail_image(reader, OFFICE_DECODE_LIMITS, None)?;
    let image = image.thumbnail(THUMB_SIZE, THUMB_SIZE).to_rgba8();
    #[cfg(target_os = "windows")]
    let image = {
        let mut image = image;
        for pixel in image.pixels_mut() { pixel.0.swap(0,2); }
        image
    };
    Ok(image)
}

#[cfg(target_os = "windows")]
fn get_office_thumbnail_uncached(path: String) -> Result<RgbaImage, CommandError> {
    if let Ok(img) = shell_thumbnail_raw(&path) {
        return Ok(img);
    }
    let file = fs::File::open(&path).map_err(|e| CommandError::IoError(e.to_string()))?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| CommandError::Other(e.to_string()))?;
    let target_files = vec!["docProps/thumbnail.jpeg", "Thumbnails/thumbnail.png"];
    for target in target_files {
        if let Ok(content_file) = archive.by_name(target) {
            if let Ok(image) = decode_office_thumbnail(content_file) {
                return Ok(image);
            }
        }
    }
    Err(CommandError::Other("No office thumbnail".to_string()))
}

#[cfg(test)]
mod office_thumbnail_security_tests {
    use super::*;

    #[cfg(target_os = "windows")]
    #[test]
    fn embedded_office_thumbnail_keeps_bgra_channels_for_native_ui() {
        let mut bytes = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(RgbaImage::from_pixel(2,1,image::Rgba([201,43,17,255])))
            .write_to(&mut bytes,image::ImageFormat::Png).unwrap();
        bytes.set_position(0);
        assert_eq!(decode_office_thumbnail(bytes).unwrap().get_pixel(0,0).0,[17,43,201,255]);
    }

    #[test]
    fn oversized_compressed_payload_is_rejected_with_bounded_reads() {
        let mut input = std::io::Cursor::new(vec![0; 9 * 1024 * 1024]);
        assert!(decode_office_thumbnail(&mut input).is_err());
        assert_eq!(input.position(), 8 * 1024 * 1024 + 1);
    }

    #[test]
    fn normal_images_decode_and_excessive_dimensions_are_rejected() {
        for (width, accepted) in [(2, true), (4097, false)] {
            let image = image::DynamicImage::ImageRgba8(RgbaImage::new(width, 1));
            let mut data = std::io::Cursor::new(Vec::new());
            image.write_to(&mut data, image::ImageFormat::Png).unwrap();
            data.set_position(0);
            assert_eq!(decode_office_thumbnail(data).is_ok(), accepted);
        }
    }
}

#[cfg(target_os = "windows")]
pub fn get_pdf_thumbnail_raw(path: String) -> Result<RgbaImage, CommandError> {
    isolated_thumbnail("pdf", &path)
}

#[cfg(target_os = "windows")]
fn shell_thumbnail_raw(path: &str) -> Result<RgbaImage, CommandError> {
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::SIZE;
    use windows::Win32::UI::Shell::{IShellItemImageFactory, SHCreateItemFromParsingName, SIIGBF};
    unsafe {
        let _com = super::windows_com::ComApartment::sta_or_existing()
            .map_err(|error| CommandError::SystemError(error.to_string()))?;
        (|| -> Result<RgbaImage, CommandError> {
            let wide_path = windows::core::HSTRING::from(path);
            let factory: IShellItemImageFactory =
                SHCreateItemFromParsingName(PCWSTR(wide_path.as_ptr()), None)?;
            let size = SIZE {
                cx: THUMB_SIZE as i32,
                cy: THUMB_SIZE as i32,
            };
            let hbitmap = factory.GetImage(size, SIIGBF(0x8))?;
            super::windows_thumbnail_bitmap::into_image(hbitmap)
        })()
    }
}

fn get_office_text_preview_local(path: String) -> Result<String, CommandError> {
    let source_path = Path::new(&path);
    let file = fs::File::open(source_path).map_err(|e| CommandError::IoError(e.to_string()))?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| CommandError::Other(e.to_string()))?;
    let ext = source_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    let targets = match ext.as_str() {
        "docx" | "docm" => vec!["word/document.xml"],
        "xlsx" | "xlsm" => vec!["xl/sharedStrings.xml"],
        "pptx" | "pptm" => vec!["ppt/slides/slide1.xml", "ppt/slides/slide2.xml"],
        "odt" | "ods" | "odp" => vec!["content.xml"],
        _ => vec!["word/document.xml", "xl/sharedStrings.xml", "content.xml"],
    };
    let mut preview = String::new();
    let mut data = Vec::new();
    for target in targets {
        if let Ok(mut content) = archive.by_name(target) {
            use std::io::Read;
            let mut buf = vec![0u8; 8192];
            if let Ok(n) = content.read(&mut buf) {
                data = buf[..n].to_vec();
                break;
            }
        }
    }
    if !data.is_empty() {
        let text = String::from_utf8_lossy(&data);
        let mut in_tag = false;
        let mut tag_content = String::new();
        for c in text.chars() {
            if c == '<' {
                in_tag = true;
                tag_content.clear();
            } else if c == '>' {
                in_tag = false;
                if tag_content == "/t" || tag_content == "t" {
                    if !preview.ends_with(' ') && !preview.ends_with('\n') {
                        preview.push(' ');
                    }
                } else if (tag_content.starts_with("w:p")
                    || tag_content == "/w:p"
                    || tag_content == "text:p")
                    && !preview.ends_with('\n')
                {
                    preview.push('\n');
                }
            } else if in_tag {
                if tag_content.len() < 10 {
                    tag_content.push(c);
                }
            } else {
                preview.push(c);
            }
            if preview.len() > 1000 {
                break;
            }
        }
    }
    Ok(preview.trim().to_string())
}

pub fn get_office_text_preview(path: String) -> Result<String, CommandError> {
    #[cfg(target_os = "windows")]
    {
        let bytes = super::platform().run_preview_worker("office-text", Path::new(&path))?;
        match crate::utils::preview_protocol::decode(&bytes)? {
            crate::utils::preview_protocol::Reply::Text(text) => Ok(text),
            crate::utils::preview_protocol::Reply::Error { detail, .. } => {
                Err(CommandError::Other(detail))
            }
            _ => Err(CommandError::Other(
                "Unexpected Office preview reply".into(),
            )),
        }
    }
    #[cfg(not(target_os = "windows"))]
    get_office_text_preview_local(path)
}

#[cfg(target_os = "windows")]
fn isolated_thumbnail(mode: &str, path: &str) -> Result<RgbaImage, CommandError> {
    let bytes = super::platform().run_preview_worker(mode, Path::new(path))?;
    match crate::utils::preview_protocol::decode(&bytes)? {
        crate::utils::preview_protocol::Reply::Images { mut frames, .. } if frames.len() == 1 => {
            Ok(frames.remove(0).into_buffer())
        }
        crate::utils::preview_protocol::Reply::Error { detail, .. } => {
            Err(CommandError::Other(detail))
        }
        _ => Err(CommandError::Other(
            "Unexpected thumbnail worker reply".into(),
        )),
    }
}

#[cfg(target_os = "windows")]
pub(super) fn worker_reply(mode: &str, path: &Path) -> crate::utils::preview_protocol::Reply {
    use crate::utils::preview_protocol::Reply;
    let result = (|| -> Result<Reply, CommandError> {
        let path = path
            .to_str()
            .ok_or_else(|| CommandError::Other("Preview path is not Unicode".into()))?
            .to_owned();
        if mode == "office-text" {
            return get_office_text_preview_local(path).map(Reply::Text);
        }
        let image = match mode {
            "cached-thumbnail" => cached_thumbnail(path, "image", get_thumbnail_uncached),
            "cached-office" => cached_thumbnail(path, "office", get_office_thumbnail_uncached),
            "icon-small" | "icon-large" => {
                let _com = super::windows_com::ComApartment::mta_or_existing()
                    .map_err(|error| CommandError::SystemError(error.to_string()))?;
                let size = if mode == "icon-small" { "small" } else { "large" };
                let bytes = super::icons::get_file_icon(path, size.into())?;
                super::windows_icon_cache::decode_icon_png(&bytes).map_err(CommandError::Other)
            }
            "thumbnail" => get_thumbnail_uncached(path),
            "office" => get_office_thumbnail_uncached(path),
            "pdf" => super::windows_pdf_thumbnail::get(path),
            _ => Err(CommandError::Other("Unknown preview worker mode".into())),
        }?;
        Ok(Reply::Images {
            frames: vec![image::Frame::new(image)],
            limited: false,
        })
    })();
    result.unwrap_or_else(|error| Reply::Error {
        limit: false,
        detail: error.to_string(),
    })
}

#[cfg(all(test, target_os = "windows"))]
mod video_stream_tests {
    use super::*;
    use std::cell::Cell;
    fn frame() -> VideoSample {
        VideoSample::Frame(RgbaImage::new(1, 1), Duration::ZERO)
    }
    #[test]
    fn opening_and_read_errors_are_returned_without_restart() {
        let opens = Cell::new(0);
        let error = play_video(
            &|| false,
            || -> Result<(), CommandError> {
                opens.set(opens.get() + 1);
                Err(CommandError::Other("open failure".into()))
            },
            |_| panic!("read after failed open"),
            &mut |_, _| panic!("frame after failed open"),
            || panic!("pause after failed open"),
        );
        assert!(error.unwrap_err().to_string().contains("open failure"));
        assert_eq!(opens.get(), 1);
        let error = play_video(
            &|| false,
            || {
                opens.set(opens.get() + 1);
                Ok(())
            },
            |_| Err(CommandError::Other("decode failure".into())),
            &mut |_, _| panic!("frame after failed decode"),
            || panic!("pause after failed decode"),
        );
        assert!(error.unwrap_err().to_string().contains("decode failure"));
        assert_eq!(opens.get(), 2);
    }
    #[test]
    fn an_empty_stream_ends_with_error_instead_of_reopening_forever() {
        let error = play_video(
            &|| false,
            || Ok(()),
            |_| Ok(VideoSample::End),
            &mut |_, _| panic!("empty stream produced frame"),
            || panic!("empty stream restarted"),
        );
        assert!(error.unwrap_err().to_string().contains("without a frame"));
    }
    #[test]
    fn cancellation_stops_gaps_and_suppresses_a_frame_returned_after_it() {
        for with_frame in [false, true] {
            let cancelled = Cell::new(false);
            play_video(
                &|| cancelled.get(),
                || Ok(()),
                |_| {
                    cancelled.set(true);
                    Ok(if with_frame {
                        frame()
                    } else {
                        VideoSample::Gap
                    })
                },
                &mut |_, _| panic!("published after cancellation"),
                || panic!("restarted after cancellation"),
            )
            .unwrap();
        }
        play_video(
            &|| true,
            || -> Result<(), CommandError> { panic!("opened after cancellation") },
            |_| panic!(),
            &mut |_, _| panic!(),
            || panic!(),
        )
        .unwrap();
    }
    #[test]
    fn only_natural_end_restarts_and_callback_refusal_stops_both_loops() {
        let opens = Cell::new(0);
        let frames = Cell::new(0);
        let pauses = Cell::new(0);
        play_video(
            &|| false,
            || {
                opens.set(opens.get() + 1);
                Ok(0)
            },
            |index| {
                *index += 1;
                Ok(if *index <= 2 {
                    frame()
                } else {
                    VideoSample::End
                })
            },
            &mut |_, _| {
                frames.set(frames.get() + 1);
                frames.get() < 3
            },
            || pauses.set(pauses.get() + 1),
        )
        .unwrap();
        assert_eq!(opens.get(), 2);
        assert_eq!(frames.get(), 3);
        assert_eq!(pauses.get(), 20);
    }
    #[test]
    fn cancellation_during_restart_pause_prevents_a_new_reader() {
        let cancelled = Cell::new(false);
        let reads = Cell::new(0);
        let opens = Cell::new(0);
        play_video(
            &|| cancelled.get(),
            || {
                opens.set(opens.get() + 1);
                Ok(())
            },
            |_| {
                reads.set(reads.get() + 1);
                Ok(if reads.get() == 1 {
                    frame()
                } else {
                    VideoSample::End
                })
            },
            &mut |_, _| true,
            || cancelled.set(true),
        )
        .unwrap();
        assert_eq!(opens.get(), 1);
    }
    #[test]
    fn video_pixels_are_bounded_and_signed_stride_is_respected() {
        assert!(VideoLayout::new(0, 2, 8).is_err());
        assert!(VideoLayout::new(2, 2, 4).is_err());
        assert!(VideoLayout::new(2, 2, i32::MIN).is_err());
        assert!(VideoLayout::new(u32::MAX, 2, 8).is_err());
        let mut layout = VideoLayout::new(2, 2, 12).unwrap();
        layout.out_w = 2;
        layout.out_h = 2;
        assert!(resize_video_frame(&[0; 19], layout).is_err());
        let pixels = [
            1, 2, 3, 0, 4, 5, 6, 0, 99, 99, 99, 99, 7, 8, 9, 0, 10, 11, 12, 0,
        ];
        let image = resize_video_frame(&pixels, layout).unwrap();
        assert_eq!(
            image.into_raw(),
            [1, 2, 3, 255, 4, 5, 6, 255, 7, 8, 9, 255, 10, 11, 12, 255]
        );
        layout.stride = -12;
        let image = resize_video_frame(&pixels, layout).unwrap();
        assert_eq!(
            image.into_raw(),
            [7, 8, 9, 255, 10, 11, 12, 255, 1, 2, 3, 255, 4, 5, 6, 255]
        );
    }
    #[test]
    fn video_timestamps_preserve_units_without_overflow() {
        assert_eq!(video_timestamp(10_000_005), Duration::new(1, 500));
        assert_eq!(video_timestamp(-1), Duration::ZERO);
        let large = video_timestamp(i64::MAX);
        assert_eq!(large.as_secs(), i64::MAX as u64 / 10_000_000);
    }
    #[test]
    fn native_video_layout_and_truncated_buffer_are_validated() {
        use windows::Win32::Media::MediaFoundation::*;
        let _com = crate::platform::windows_com::ComApartment::mta_or_existing().unwrap();
        let _media = MediaFoundationGuard::new().unwrap();
        unsafe {
            let media = MFCreateMediaType().unwrap();
            media.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_NV12).unwrap();
            media
                .SetUINT64(&MF_MT_FRAME_SIZE, (2u64 << 32) | 2)
                .unwrap();
            assert!(VideoLayout::from_media_type(&media).is_err());
            media.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_RGB32).unwrap();
            media.SetUINT32(&MF_MT_DEFAULT_STRIDE, 8).unwrap();
            let layout = VideoLayout::from_media_type(&media).unwrap();
            let sample = MFCreateSample().unwrap();
            let buffer = MFCreateMemoryBuffer(16).unwrap();
            let mut ptr = std::ptr::null_mut();
            buffer.Lock(&mut ptr, None, None).unwrap();
            std::slice::from_raw_parts_mut(ptr, 16).copy_from_slice(&[30, 60, 90, 0].repeat(4));
            buffer.Unlock().unwrap();
            buffer.SetCurrentLength(16).unwrap();
            sample.AddBuffer(&buffer).unwrap();
            let frame = subsample_frame(sample.clone(), layout).unwrap();
            assert_eq!(frame.dimensions(), (640, 640));
            assert_eq!(frame.get_pixel(0, 0).0, [30, 60, 90, 255]);
            buffer.SetCurrentLength(15).unwrap();
            assert!(subsample_frame(sample, layout).is_err());
            buffer.Lock(&mut ptr, None, None).unwrap();
            buffer.Unlock().unwrap();
        }
    }

    #[test]
    fn native_video_open_failure_is_reported() {
        let root = tempfile::tempdir().unwrap();
        let path = root
            .path()
            .join("missing.mp4")
            .to_string_lossy()
            .into_owned();
        let result =
            stream_video_frames_local(path, |_, _| panic!("missing file produced image"), || false);
        assert!(result.is_err());
    }
}
