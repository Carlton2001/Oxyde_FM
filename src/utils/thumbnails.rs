use std::path::Path;
use std::fs;
use image::{imageops::FilterType, GenericImageView, RgbaImage};
use crate::models::CommandError;

/// Target thumbnail size in pixels (longest side).
const THUMB_SIZE: u32 = 256;

#[cfg(target_os = "windows")]
pub fn get_thumbnail_raw(path: String) -> Result<RgbaImage, CommandError> {
    shell_thumbnail_raw(&path)
}

#[cfg(target_os = "windows")]
pub fn get_office_thumbnail_raw(path: String) -> Result<RgbaImage, CommandError> {
    // 1. Try Windows Shell
    if let Ok(img) = shell_thumbnail_raw(&path) {
        return Ok(img);
    }

    // 2. Try extraction from Zip
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

            let mut rgba = pixels.to_vec();
            for chunk in rgba.chunks_exact_mut(4) { chunk.swap(0, 2); }

            RgbaImage::from_raw(width, height, rgba)
                .ok_or_else(|| CommandError::Other("Invalid pixel data".to_string()))
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
            let hbitmap = factory.GetImage(size, SIIGBF(0x8))?; // SIIGBF_THUMBNAILONLY

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

            for chunk in pixels.chunks_exact_mut(4) { chunk.swap(0, 2); }
            RgbaImage::from_raw(bmp.bmWidth as u32, bmp.bmHeight as u32, pixels)
                .ok_or_else(|| CommandError::Other("Invalid bitmap".to_string()))
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
            if c == '<' { 
                in_tag = true; 
                tag_content.clear();
            } else if c == '>' { 
                in_tag = false; 
                // Pour Excel (t) ou Word (p/br), on gère l'espacement
                if tag_content == "/t" || tag_content == "t" {
                    if !preview.ends_with(' ') && !preview.ends_with('\n') {
                        preview.push(' ');
                    }
                } else if tag_content.starts_with("w:p") || tag_content == "/w:p" || tag_content == "text:p" {
                    if !preview.ends_with('\n') {
                        preview.push('\n');
                    }
                }
            } else if in_tag {
                if tag_content.len() < 10 { tag_content.push(c); }
            } else {
                preview.push(c);
            }
            if preview.len() > 1000 { break; }
        }
    }
    Ok(preview.trim().to_string())
}
