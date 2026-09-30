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

    if let Ok(image) = image::open(&cache_file) {
        return Ok(image.to_rgba8());
    }
    if cache_file.exists() {
        let _ = fs::remove_file(&cache_file);
    }
    fs::create_dir_all(&cache_dir).map_err(|error| CommandError::IoError(error.to_string()))?;

    let image = generate(path.clone()).map_err(|error| {
        log::warn!("Could not generate {kind} thumbnail for {path}: {error}");
        error
    })?;
    let temp_id = CACHE_TEMP_ID.fetch_add(1, Ordering::Relaxed);
    let temp_file = cache_dir.join(format!("{key}.{}.{}.tmp", std::process::id(), temp_id));
    let rgb = image::DynamicImage::ImageRgba8(image.clone()).to_rgb8();
    let write_result = (|| -> Result<(), CommandError> {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp_file)
            .map_err(|error| CommandError::IoError(error.to_string()))?;
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut file, 75)
            .encode_image(&image::DynamicImage::ImageRgb8(rgb))
            .map_err(|error| CommandError::Other(error.to_string()))?;
        file.sync_all()
            .map_err(|error| CommandError::IoError(error.to_string()))?;
        match fs::rename(&temp_file, &cache_file) {
            Ok(()) => Ok(()),
            Err(_) if cache_file.exists() => {
                let _ = fs::remove_file(&temp_file);
                Ok(())
            }
            Err(error) => Err(CommandError::IoError(error.to_string())),
        }
    })();
    if write_result.is_err() {
        let _ = fs::remove_file(&temp_file);
    }
    write_result?;
    Ok(image)
}

pub fn get_thumbnail_raw(path: String) -> Result<RgbaImage, CommandError> {
    cached_thumbnail(path, "image", get_thumbnail_uncached)
}

pub fn get_office_thumbnail_raw(path: String) -> Result<RgbaImage, CommandError> {
    cached_thumbnail(path, "office", get_office_thumbnail_uncached)
}

#[cfg(target_os = "windows")]
fn get_thumbnail_uncached(path: String) -> Result<RgbaImage, CommandError> {
    match shell_thumbnail_raw(&path) {
        Ok(image) => Ok(image),
        Err(shell_error) => match image::open(&path) {
            Ok(image) => {
                let mut image = image.thumbnail(THUMB_SIZE, THUMB_SIZE).to_rgba8();
                // RenderImage consumes BGRA buffers; image::open returns RGBA.
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
    image::open(path)
        .map(|image| image.thumbnail(THUMB_SIZE, THUMB_SIZE).to_rgba8())
        .map_err(|error| CommandError::Other(error.to_string()))
}

#[cfg(not(target_os = "windows"))]
fn get_office_thumbnail_uncached(path: String) -> Result<RgbaImage, CommandError> {
    let file = fs::File::open(path).map_err(|error| CommandError::IoError(error.to_string()))?;
    let mut archive =
        zip::ZipArchive::new(file).map_err(|error| CommandError::Other(error.to_string()))?;
    for name in ["docProps/thumbnail.jpeg", "Thumbnails/thumbnail.png"] {
        if let Ok(mut entry) = archive.by_name(name) {
            let mut bytes = Vec::new();
            std::io::Read::read_to_end(&mut entry, &mut bytes)
                .map_err(|error| CommandError::IoError(error.to_string()))?;
            return image::load_from_memory(&bytes)
                .map(|image| image.thumbnail(THUMB_SIZE, THUMB_SIZE).to_rgba8())
                .map_err(|error| CommandError::Other(error.to_string()));
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
pub fn stream_video_frames_sync<F>(_path: String, _on_frame: F) -> Result<(), CommandError>
where
    F: FnMut(RgbaImage, Duration) -> bool,
{
    Err(CommandError::Other(
        "Video preview is unavailable on this platform".to_string(),
    ))
}

#[cfg(target_os = "windows")]
pub fn stream_video_frames_sync<F>(path: String, mut on_frame: F) -> Result<(), CommandError>
where
    F: FnMut(RgbaImage, Duration) -> bool,
{
    use windows::Win32::Media::MediaFoundation::*;
    unsafe {
        let _ = windows::Win32::System::Com::CoInitializeEx(
            None,
            windows::Win32::System::Com::COINIT_MULTITHREADED,
        );
        let _ = MFStartup(MF_SDK_VERSION, MFSTARTUP_FULL);

        let res: Result<(), CommandError> = {
            let wide_path = windows::core::HSTRING::from(&path);

            // Outer loop — restarts the reader from the beginning when the video ends.
            loop {
                let mut attributes: Option<IMFAttributes> = None;
                MFCreateAttributes(&mut attributes, 2).ok();
                if let Some(attr) = &attributes {
                    attr.SetUINT32(&MF_SOURCE_READER_ENABLE_VIDEO_PROCESSING, 1)
                        .ok();
                    attr.SetUINT32(&MF_READWRITE_ENABLE_HARDWARE_TRANSFORMS, 1)
                        .ok();
                }

                let reader: IMFSourceReader = match MFCreateSourceReaderFromURL(
                    windows::core::PCWSTR(wide_path.as_ptr()),
                    attributes.as_ref(),
                ) {
                    Ok(r) => r,
                    Err(_) => break,
                };

                reader
                    .SetStreamSelection(MF_SOURCE_READER_ALL_STREAMS.0 as u32, false)
                    .ok();
                reader
                    .SetStreamSelection(MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32, true)
                    .ok();

                // Read native dimensions from source media type.
                let src_type = match reader
                    .GetCurrentMediaType(MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32)
                {
                    Ok(t) => t,
                    Err(_) => break,
                };

                let src_size = src_type.GetUINT64(&MF_MT_FRAME_SIZE).unwrap_or(0);
                let src_w = ((src_size >> 32) as u32).max(1);
                let src_h = (src_size as u32).max(1);
                let src_stride = src_w * 4;

                // Request RGB32 output.
                let out_req = match MFCreateMediaType() {
                    Ok(t) => t,
                    Err(_) => break,
                };
                out_req.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video).ok();
                out_req.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_RGB32).ok();
                reader
                    .SetCurrentMediaType(
                        MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32,
                        None,
                        &out_req,
                    )
                    .ok();

                // Aspect-ratio-preserving output size.
                let (out_w, out_h) = if src_w >= src_h {
                    let h = ((VIDEO_THUMB_SIZE as u64 * src_h as u64) / src_w as u64).max(1) as u32;
                    (VIDEO_THUMB_SIZE, h)
                } else {
                    let w = ((VIDEO_THUMB_SIZE as u64 * src_w as u64) / src_h as u64).max(1) as u32;
                    (w, VIDEO_THUMB_SIZE)
                };

                // Inner decode loop — reads frames and sends them with their actual PTS.
                let mut caller_stopped = false;
                loop {
                    let mut flags = 0u32;
                    let mut sample: Option<IMFSample> = None;
                    if reader
                        .ReadSample(
                            MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32,
                            0,
                            None,
                            Some(&mut flags),
                            None,
                            Some(&mut sample),
                        )
                        .is_err()
                    {
                        break;
                    }

                    if flags & MF_SOURCE_READERF_ENDOFSTREAM.0 as u32 != 0 {
                        break; // natural end → outer loop will restart reader
                    }

                    if let Some(s) = sample {
                        // Actual presentation timestamp in 100-nanosecond WMF units.
                        let pts_100ns = s.GetSampleTime().unwrap_or(0);
                        let pts = Duration::from_nanos(pts_100ns as u64 * 100);

                        if let Ok(frame) =
                            subsample_frame(s, src_w, src_h, src_stride, out_w, out_h)
                        {
                            if !on_frame(frame, pts) {
                                caller_stopped = true;
                                break;
                            }
                        }
                    }
                }

                if caller_stopped {
                    break; // receiver dropped — stop the outer loop too
                }
                // Video ended naturally — small pause before restarting from the top.
                std::thread::sleep(Duration::from_millis(200));
            }
            Ok(())
        };

        let _ = MFShutdown();
        windows::Win32::System::Com::CoUninitialize();
        res
    }
}

#[cfg(target_os = "windows")]
fn subsample_frame(
    sample: windows::Win32::Media::MediaFoundation::IMFSample,
    src_w: u32,
    src_h: u32,
    src_stride: u32,
    out_w: u32,
    out_h: u32,
) -> Result<RgbaImage, CommandError> {
    unsafe {
        let buffer = sample
            .ConvertToContiguousBuffer()
            .map_err(|e| CommandError::Other(format!("ConvertToContiguousBuffer: {e}")))?;

        let mut ptr = std::ptr::null_mut::<u8>();
        let mut _len = 0u32;
        buffer
            .Lock(&mut ptr, None, Some(&mut _len))
            .map_err(|e| CommandError::Other(format!("Lock: {e}")))?;

        let mut out = vec![0u8; (out_w * out_h * 4) as usize];
        let src_w_f = src_w as f32;
        let src_h_f = src_h as f32;
        let out_w_f = out_w as f32;
        let out_h_f = out_h as f32;
        let stride = src_stride as usize;

        // Bilinear interpolation with center-aligned sampling.
        // RGB32 layout: [B, G, R, X] → output [B, G, R, A=255]
        for y in 0..out_h as usize {
            let fy = (y as f32 + 0.5) * src_h_f / out_h_f - 0.5;
            let y0 = (fy as i32).clamp(0, src_h as i32 - 1) as usize;
            let y1 = (y0 + 1).min(src_h as usize - 1);
            let wy1 = (fy - y0 as f32).max(0.0);
            let wy0 = 1.0 - wy1;

            for x in 0..out_w as usize {
                let fx = (x as f32 + 0.5) * src_w_f / out_w_f - 0.5;
                let x0 = (fx as i32).clamp(0, src_w as i32 - 1) as usize;
                let x1 = (x0 + 1).min(src_w as usize - 1);
                let wx1 = (fx - x0 as f32).max(0.0);
                let wx0 = 1.0 - wx1;

                let dst_off = (y * out_w as usize + x) * 4;

                for c in 0..3usize {
                    let p00 = *ptr.add(y0 * stride + x0 * 4 + c) as f32;
                    let p10 = *ptr.add(y0 * stride + x1 * 4 + c) as f32;
                    let p01 = *ptr.add(y1 * stride + x0 * 4 + c) as f32;
                    let p11 = *ptr.add(y1 * stride + x1 * 4 + c) as f32;
                    let val = wy0 * (wx0 * p00 + wx1 * p10) + wy1 * (wx0 * p01 + wx1 * p11);
                    out[dst_off + c] = val as u8;
                }
                out[dst_off + 3] = 255;
            }
        }

        let _ = buffer.Unlock();

        RgbaImage::from_raw(out_w, out_h, out)
            .ok_or_else(|| CommandError::Other("Invalid frame dimensions".into()))
    }
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
        if let Ok(mut content_file) = archive.by_name(target) {
            use std::io::Read;
            let mut data = Vec::new();
            if content_file.read_to_end(&mut data).is_ok() {
                if let Ok(img) = image::load_from_memory(&data) {
                    return Ok(img.thumbnail(THUMB_SIZE, THUMB_SIZE).to_rgba8());
                }
            }
        }
    }
    Err(CommandError::Other("No office thumbnail".to_string()))
}

#[cfg(target_os = "windows")]
pub fn get_pdf_thumbnail_raw(path: String) -> Result<RgbaImage, CommandError> {
    use windows::Data::Pdf::{PdfDocument, PdfPageRenderOptions};
    use windows::Graphics::Imaging::BitmapDecoder;
    use windows::Storage::StorageFile;
    use windows::Storage::Streams::InMemoryRandomAccessStream;
    unsafe {
        use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_MULTITHREADED};
        let co_ok = CoInitializeEx(None, COINIT_MULTITHREADED).is_ok();
        let res = (|| -> Result<RgbaImage, CommandError> {
            let abs_path =
                std::fs::canonicalize(&path).map_err(|e| CommandError::IoError(e.to_string()))?;
            let mut path_str = abs_path.to_string_lossy().to_string();
            if path_str.starts_with(r"\\?\") {
                path_str = path_str.chars().skip(4).collect();
            }
            let hstring_path = windows::core::HSTRING::from(&path_str);
            let file = StorageFile::GetFileFromPathAsync(&hstring_path)?.join()?;
            let doc = PdfDocument::LoadFromFileAsync(&file)?.join()?;
            if doc.PageCount()? == 0 {
                return Err(CommandError::Other("Empty PDF".to_string()));
            }
            let page = doc.GetPage(0)?;
            let stream = InMemoryRandomAccessStream::new()?;
            let options = PdfPageRenderOptions::new()?;
            options.SetDestinationWidth(THUMB_SIZE * 2)?;
            page.RenderWithOptionsToStreamAsync(&stream, &options)?
                .join()?;
            let decoder = BitmapDecoder::CreateAsync(&stream)?.join()?;
            let pixel_data = decoder.GetPixelDataAsync()?.join()?;
            let pixels = pixel_data.DetachPixelData()?;
            let width = decoder.PixelWidth()?;
            let height = decoder.PixelHeight()?;
            let rgba = pixels.to_vec();
            RgbaImage::from_raw(width, height, rgba)
                .ok_or_else(|| CommandError::Other("Invalid pixel data".to_string()))
        })();
        if co_ok {
            CoUninitialize();
        }
        res
    }
}

#[cfg(target_os = "windows")]
fn shell_thumbnail_raw(path: &str) -> Result<RgbaImage, CommandError> {
    use std::ffi::c_void;
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::SIZE;
    use windows::Win32::Graphics::Gdi::{
        CreateCompatibleDC, DeleteDC, DeleteObject, GetDIBits, GetObjectW, BITMAP, BITMAPINFO,
        BITMAPINFOHEADER, DIB_RGB_COLORS, HGDIOBJ,
    };
    use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED};
    use windows::Win32::UI::Shell::{IShellItemImageFactory, SHCreateItemFromParsingName, SIIGBF};
    unsafe {
        let co_ok = CoInitializeEx(None, COINIT_APARTMENTTHREADED).is_ok();
        let res = (|| -> Result<RgbaImage, CommandError> {
            let wide_path = windows::core::HSTRING::from(path);
            let factory: IShellItemImageFactory =
                SHCreateItemFromParsingName(PCWSTR(wide_path.as_ptr()), None)?;
            let size = SIZE {
                cx: THUMB_SIZE as i32,
                cy: THUMB_SIZE as i32,
            };
            let hbitmap = factory.GetImage(size, SIIGBF(0x8))?;
            let mut bmp: BITMAP = std::mem::zeroed();
            GetObjectW(
                HGDIOBJ(hbitmap.0),
                std::mem::size_of::<BITMAP>() as i32,
                Some(&mut bmp as *mut _ as *mut c_void),
            );
            let hdc = CreateCompatibleDC(None);
            let mut bmi: BITMAPINFO = std::mem::zeroed();
            bmi.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
            bmi.bmiHeader.biWidth = bmp.bmWidth;
            bmi.bmiHeader.biHeight = -bmp.bmHeight;
            bmi.bmiHeader.biPlanes = 1;
            bmi.bmiHeader.biBitCount = 32;
            let mut pixels = vec![0u8; (bmp.bmWidth * bmp.bmHeight * 4) as usize];
            GetDIBits(
                hdc,
                hbitmap,
                0,
                bmp.bmHeight as u32,
                Some(pixels.as_mut_ptr() as *mut c_void),
                &mut bmi,
                DIB_RGB_COLORS,
            );
            let _ = DeleteDC(hdc);
            let _ = DeleteObject(HGDIOBJ(hbitmap.0));
            RgbaImage::from_raw(bmp.bmWidth as u32, bmp.bmHeight as u32, pixels)
                .ok_or_else(|| CommandError::Other("Invalid bitmap".to_string()))
        })();
        if co_ok {
            CoUninitialize();
        }
        res
    }
}

pub fn get_office_text_preview(path: String) -> Result<String, CommandError> {
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
