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
#[path = "thumbnails/tests/thumbnail_decode_security_tests.rs"]
mod thumbnail_decode_security_tests;

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
    let Some(cache_root) = crate::app_paths::cache_dir() else {
        log::warn!("Cannot resolve the user cache directory; thumbnail will not be cached");
        return generate(path);
    };
    let cache_dir = cache_root.join("thumbnails");
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
#[path = "thumbnails/tests/thumbnail_cache_write_tests.rs"]
mod thumbnail_cache_write_tests;

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
                    .SetUINT32(&MF_SOURCE_READER_ENABLE_ADVANCED_VIDEO_PROCESSING, 1)
                    .map_err(video_error)?;
                attributes
                    .SetUINT32(&MF_READWRITE_ENABLE_HARDWARE_TRANSFORMS, 1)
                    .map_err(video_error)?;
                let reader = MFCreateSourceReaderFromURL(
                    windows::core::PCWSTR(wide_path.as_ptr()),
                    &attributes,
                )
                .map_err(|error| CommandError::SystemError(format!("Video source open: {error}")))?;
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
                // Scale in the native video processor rather than converting a
                // full-resolution RGB frame and resizing it in Rust every time.
                let native_size = reader.GetNativeMediaType(stream, 0).map_err(video_error)?
                    .GetUINT64(&MF_MT_FRAME_SIZE).map_err(video_error)?;
                let width = (native_size >> 32) as u32;
                let height = native_size as u32;
                let stride = width.checked_mul(4).filter(|stride| *stride <= i32::MAX as u32)
                    .ok_or_else(|| CommandError::Other("Video stride overflow".into()))?;
                let desired = VideoLayout::new(width, height, stride as i32)?;
                if width > VIDEO_THUMB_SIZE || height > VIDEO_THUMB_SIZE {
                    request.SetUINT64(&MF_MT_FRAME_SIZE,
                        (u64::from(desired.out_w) << 32) | u64::from(desired.out_h))
                        .map_err(video_error)?;
                }
                if let Err(error) = reader.SetCurrentMediaType(stream, None, &request) {
                    // Some installed processors cannot scale a particular format.
                    // Keep the former bounded Rust resize inside this worker.
                    if width <= VIDEO_THUMB_SIZE && height <= VIDEO_THUMB_SIZE {
                        return Err(CommandError::SystemError(format!("Video output negotiation: {error}")));
                    }
                    request.DeleteItem(&MF_MT_FRAME_SIZE).map_err(video_error)?;
                    reader.SetCurrentMediaType(stream, None, &request)
                        .map_err(|error| CommandError::SystemError(format!("Video output negotiation: {error}")))?;
                }
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
    if src_w == out_w && src_h == out_h {
        for (y, destination) in out.chunks_exact_mut(out_w as usize * 4).enumerate() {
            let offset = row(y) * pitch;
            destination.copy_from_slice(&bytes[offset..offset + destination.len()]);
            for pixel in destination.chunks_exact_mut(4) { pixel[3] = 255; }
        }
        return RgbaImage::from_raw(out_w, out_h, out)
            .ok_or_else(|| CommandError::Other("Invalid frame dimensions".into()));
    }
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
#[path = "thumbnails/tests/office_thumbnail_security_tests.rs"]
mod office_thumbnail_security_tests;

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
    let source = Path::new(&path);
    let file = fs::File::open(source).map_err(|error| CommandError::IoError(error.to_string()))?;
    crate::utils::office_preview::text(file, source.extension().and_then(|ext| ext.to_str()).unwrap_or(""))
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
#[path = "thumbnails/tests/video_stream_tests.rs"]
mod video_stream_tests;
