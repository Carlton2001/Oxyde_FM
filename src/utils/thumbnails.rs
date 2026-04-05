use std::path::{Path, PathBuf};
use std::time::{SystemTime, Duration};
use std::fs;
use image::RgbaImage;
use crate::models::CommandError;

/// Target thumbnail size in pixels (longest side).
const THUMB_SIZE: u32 = 256;
const VIDEO_THUMB_SIZE: u32 = 192;

#[cfg(target_os = "windows")]
pub fn get_thumbnail_raw(path: String) -> Result<RgbaImage, CommandError> {
    shell_thumbnail_raw(&path)
}

#[cfg(target_os = "windows")]
pub fn stream_video_frames_sync<F>(path: String, mut on_frame: F) -> Result<(), CommandError>
where F: FnMut(RgbaImage, Duration) -> bool
{
    use windows::Win32::Media::MediaFoundation::*;
    unsafe {
        let _ = windows::Win32::System::Com::CoInitializeEx(None, windows::Win32::System::Com::COINIT_MULTITHREADED);
        let _ = MFStartup(MF_SDK_VERSION, MFSTARTUP_FULL);

        let res = (|| -> Result<(), CommandError> {
            let wide_path = windows::core::HSTRING::from(&path);

            let mut attributes: Option<IMFAttributes> = None;
            MFCreateAttributes(&mut attributes, 2).ok();
            if let Some(attr) = &attributes {
                attr.SetUINT32(&MF_SOURCE_READER_ENABLE_VIDEO_PROCESSING, 1).ok();
                attr.SetUINT32(&MF_READWRITE_ENABLE_HARDWARE_TRANSFORMS, 1).ok();
            }

            let reader: IMFSourceReader = MFCreateSourceReaderFromURL(
                windows::core::PCWSTR(wide_path.as_ptr()), attributes.as_ref()
            ).map_err(|e| CommandError::Other(format!("MFCreateSourceReaderFromURL: {e}")))?;

            reader.SetStreamSelection(MF_SOURCE_READER_ALL_STREAMS.0 as u32, false).ok();
            reader.SetStreamSelection(MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32, true).ok();

            // ── Read native dimensions BEFORE touching the output type ──────────
            // GetCurrentMediaType here returns the decoded source format (e.g. NV12
            // at 3840×2160) with true content dimensions.
            // SetCurrentMediaType with MF_MT_FRAME_SIZE is silently ignored for many
            // codecs — the buffer is always delivered at native resolution.
            let src_type = reader.GetCurrentMediaType(MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32)
                .map_err(|e| CommandError::Other(format!("GetCurrentMediaType(src): {e}")))?;

            let src_size = src_type.GetUINT64(&MF_MT_FRAME_SIZE).unwrap_or(0);
            let src_w = ((src_size >> 32) as u32).max(1);
            let src_h = (src_size as u32).max(1);

            let fps_packed = src_type.GetUINT64(&MF_MT_FRAME_RATE).unwrap_or((30u64 << 32) | 1);
            let fps_num = (fps_packed >> 32) as u32;
            let fps_den = (fps_packed & 0xFFFF_FFFF) as u32;
            let frame_duration = Duration::from_micros(1_000_000 * fps_den as u64 / fps_num.max(1) as u64);

            // ── Request RGB32 output (format conversion only, no size constraint) ─
            let out_req = MFCreateMediaType()
                .map_err(|e| CommandError::Other(format!("MFCreateMediaType: {e}")))?;
            out_req.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video).ok();
            out_req.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_RGB32).ok();
            reader.SetCurrentMediaType(MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32, None, &out_req).ok();

            // ── Thumbnail output size: aspect-ratio-preserved, max VIDEO_THUMB_SIZE ─
            let (out_w, out_h) = if src_w >= src_h {
                let h = ((VIDEO_THUMB_SIZE as u64 * src_h as u64) / src_w as u64).max(1) as u32;
                (VIDEO_THUMB_SIZE, h)
            } else {
                let w = ((VIDEO_THUMB_SIZE as u64 * src_w as u64) / src_h as u64).max(1) as u32;
                (w, VIDEO_THUMB_SIZE)
            };

            // RGB32 row stride = src_w * 4 (no column padding for standard video widths).
            // Codec block alignment only adds extra rows at the bottom; we stay within src_h.
            let src_stride = src_w * 4;

            loop {
                let mut flags = 0u32;
                let mut sample: Option<IMFSample> = None;
                if reader.ReadSample(
                    MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32, 0,
                    None, Some(&mut flags), None, Some(&mut sample)
                ).is_err() { break; }

                if flags & MF_SOURCE_READERF_ENDOFSTREAM.0 as u32 != 0 { break; }

                if let Some(s) = sample {
                    if let Ok(frame) = subsample_frame(s, src_w, src_h, src_stride, out_w, out_h) {
                        if !on_frame(frame, frame_duration) { break; }
                    }
                }
            }
            Ok(())
        })();

        let _ = MFShutdown();
        let _ = windows::Win32::System::Com::CoUninitialize();
        res
    }
}

/// Point-samples a native-resolution RGB32 WMF frame down to (out_w × out_h).
///
/// The WMF buffer (e.g. 33 MB for 4K) is read directly through the locked pointer
/// and released immediately — only out_w*out_h*4 bytes are ever allocated.
#[cfg(target_os = "windows")]
fn subsample_frame(
    sample: windows::Win32::Media::MediaFoundation::IMFSample,
    src_w: u32, src_h: u32, src_stride: u32,
    out_w: u32, out_h: u32,
) -> Result<RgbaImage, CommandError> {
    unsafe {
        let buffer = sample.ConvertToContiguousBuffer()
            .map_err(|e| CommandError::Other(format!("ConvertToContiguousBuffer: {e}")))?;

        let mut ptr = std::ptr::null_mut::<u8>();
        let mut _len = 0u32;
        buffer.Lock(&mut ptr, None, Some(&mut _len))
            .map_err(|e| CommandError::Other(format!("Lock: {e}")))?;

        let mut out = vec![0u8; (out_w * out_h * 4) as usize];

        // Integer point sampling: src_x = x * src_w / out_w  (floor)
        for y in 0..out_h as usize {
            let src_y = (y * src_h as usize) / out_h as usize;
            let src_row = src_y * src_stride as usize;
            let dst_row = y * out_w as usize * 4;
            for x in 0..out_w as usize {
                let src_x = (x * src_w as usize) / out_w as usize;
                let src_off = src_row + src_x * 4;
                let dst_off = dst_row + x * 4;
                // RGB32 layout: [B, G, R, X] — GPUI expects [B, G, R, A]
                let p = ptr.add(src_off);
                out[dst_off]     = *p;
                out[dst_off + 1] = *p.add(1);
                out[dst_off + 2] = *p.add(2);
                out[dst_off + 3] = 255;
            }
        }

        let _ = buffer.Unlock();

        RgbaImage::from_raw(out_w, out_h, out)
            .ok_or_else(|| CommandError::Other("Invalid frame dimensions".into()))
    }
}

#[cfg(target_os = "windows")]
pub fn get_office_thumbnail_raw(path: String) -> Result<RgbaImage, CommandError> {
    if let Ok(img) = shell_thumbnail_raw(&path) { return Ok(img); }
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
    use windows::Storage::StorageFile;
    use windows::Storage::Streams::InMemoryRandomAccessStream;
    use windows::Graphics::Imaging::BitmapDecoder;
    unsafe {
        use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_MULTITHREADED};
        let co_ok = CoInitializeEx(None, COINIT_MULTITHREADED).is_ok();
        let res = (|| -> Result<RgbaImage, CommandError> {
            let abs_path = std::fs::canonicalize(&path).map_err(|e| CommandError::IoError(e.to_string()))?;
            let mut path_str = abs_path.to_string_lossy().to_string();
            if path_str.starts_with(r"\\?\") { path_str = path_str.chars().skip(4).collect(); }
            let hstring_path = windows::core::HSTRING::from(&path_str);
            let file = StorageFile::GetFileFromPathAsync(&hstring_path)?.get()?;
            let doc = PdfDocument::LoadFromFileAsync(&file)?.get()?;
            if doc.PageCount()? == 0 { return Err(CommandError::Other("Empty PDF".to_string())); }
            let page = doc.GetPage(0)?;
            let stream = InMemoryRandomAccessStream::new()?;
            let options = PdfPageRenderOptions::new()?;
            options.SetDestinationWidth(THUMB_SIZE * 2)?;
            page.RenderWithOptionsToStreamAsync(&stream, &options)?.get()?;
            let decoder = BitmapDecoder::CreateAsync(&stream)?.get()?;
            let pixel_data = decoder.GetPixelDataAsync()?.get()?;
            let pixels = pixel_data.DetachPixelData()?;
            let width = decoder.PixelWidth()?;
            let height = decoder.PixelHeight()?;
            let rgba = pixels.to_vec();
            RgbaImage::from_raw(width, height, rgba).ok_or_else(|| CommandError::Other("Invalid pixel data".to_string()))
        })();
        if co_ok { CoUninitialize(); }
        res
    }
}

#[cfg(target_os = "windows")]
fn shell_thumbnail_raw(path: &str) -> Result<RgbaImage, CommandError> {
    use std::ffi::c_void;
    use windows::Win32::Foundation::SIZE;
    use windows::Win32::Graphics::Gdi::{
        BITMAP, BITMAPINFO, BITMAPINFOHEADER, CreateCompatibleDC, DeleteDC, DeleteObject,
        DIB_RGB_COLORS, GetDIBits, GetObjectW, HGDIOBJ,
    };
    use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED};
    use windows::Win32::UI::Shell::{IShellItemImageFactory, SHCreateItemFromParsingName, SIIGBF};
    use windows::core::PCWSTR;
    unsafe {
        let co_ok = CoInitializeEx(None, COINIT_APARTMENTTHREADED).is_ok();
        let res = (|| -> Result<RgbaImage, CommandError> {
            let wide_path = windows::core::HSTRING::from(path);
            let factory: IShellItemImageFactory = SHCreateItemFromParsingName(PCWSTR(wide_path.as_ptr()), None)?;
            let size = SIZE { cx: THUMB_SIZE as i32, cy: THUMB_SIZE as i32 };
            let hbitmap = factory.GetImage(size, SIIGBF(0x8))?;
            let mut bmp: BITMAP = std::mem::zeroed();
            GetObjectW(HGDIOBJ(hbitmap.0), std::mem::size_of::<BITMAP>() as i32, Some(&mut bmp as *mut _ as *mut c_void));
            let hdc = CreateCompatibleDC(None);
            let mut bmi: BITMAPINFO = std::mem::zeroed();
            bmi.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
            bmi.bmiHeader.biWidth = bmp.bmWidth;
            bmi.bmiHeader.biHeight = -bmp.bmHeight;
            bmi.bmiHeader.biPlanes = 1;
            bmi.bmiHeader.biBitCount = 32;
            let mut pixels = vec![0u8; (bmp.bmWidth * bmp.bmHeight * 4) as usize];
            GetDIBits(hdc, hbitmap, 0, bmp.bmHeight as u32, Some(pixels.as_mut_ptr() as *mut c_void), &mut bmi, DIB_RGB_COLORS);
            let _ = DeleteDC(hdc);
            let _ = DeleteObject(HGDIOBJ(hbitmap.0));
            RgbaImage::from_raw(bmp.bmWidth as u32, bmp.bmHeight as u32, pixels).ok_or_else(|| CommandError::Other("Invalid bitmap".to_string()))
        })();
        if co_ok { CoUninitialize(); }
        res
    }
}

pub fn get_office_text_preview(path: String) -> Result<String, CommandError> {
    let source_path = Path::new(&path);
    let file = fs::File::open(source_path).map_err(|e| CommandError::IoError(e.to_string()))?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| CommandError::Other(e.to_string()))?;
    let ext = source_path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    let targets = match ext.as_str() {
        "docx" | "docm" => vec!["word/document.xml"],
        "xlsx" | "xlsm" => vec!["xl/sharedStrings.xml"],
        "pptx" | "pptm" => vec!["ppt/slides/slide1.xml", "ppt/slides/slide2.xml"],
        "odt" | "ods" | "odp" => vec!["content.xml"],
        _ => vec!["word/document.xml", "xl/sharedStrings.xml", "content.xml"]
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
            if c == '<' { in_tag = true; tag_content.clear(); }
            else if c == '>' { in_tag = false;
                if tag_content == "/t" || tag_content == "t" {
                    if !preview.ends_with(' ') && !preview.ends_with('\n') { preview.push(' '); }
                } else if tag_content.starts_with("w:p") || tag_content == "/w:p" || tag_content == "text:p" {
                    if !preview.ends_with('\n') { preview.push('\n'); }
                }
            } else if in_tag { if tag_content.len() < 10 { tag_content.push(c); } }
            else { preview.push(c); }
            if preview.len() > 1000 { break; }
        }
    }
    Ok(preview.trim().to_string())
}
