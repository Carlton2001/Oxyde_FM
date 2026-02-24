use image::RgbaImage;
use std::io::Cursor;
use std::os::windows::ffi::OsStrExt;
use windows::core::PCWSTR;
use windows::Win32::UI::Shell::{
    SHGetFileInfoW, SHFILEINFOW, SHGFI_ICON, SHGFI_LARGEICON, SHGFI_SMALLICON, SHGFI_SYSICONINDEX,
    SHGetImageList, SHIL_JUMBO,
};
use windows::Win32::UI::Controls::IImageList;
use windows::Win32::UI::WindowsAndMessaging::{DestroyIcon, HICON, GetIconInfo, DrawIconEx, DI_NORMAL};
use windows::Win32::Graphics::Gdi::{
    GetDC, ReleaseDC, DeleteObject, CreateCompatibleDC, DeleteDC, HGDIOBJ, 
    CreateDIBSection, SelectObject, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS,
    GetObjectW, BITMAP
};
use crate::models::CommandError;
use std::collections::HashMap;
use std::sync::Mutex;
use lazy_static::lazy_static;

lazy_static! {
    static ref ICON_CACHE: Mutex<HashMap<String, Vec<u8>>> = Mutex::new(HashMap::new());
}

#[tauri::command]
pub fn purge_icon_cache() {
    let mut cache = ICON_CACHE.lock().unwrap();
    cache.clear();
    cache.shrink_to_fit();
}

#[tauri::command]
pub fn get_file_icon(path: String, size: String) -> Result<Vec<u8>, CommandError> {
    extract_icon_png(&path, &size, false)
        .map_err(|e| CommandError::SystemError(format!("Failed to extract icon: {}", e)))
}

pub fn extract_icon_png(path: &str, size: &str, use_attributes: bool) -> Result<Vec<u8>, String> {
    let wide_path: Vec<u16> = std::ffi::OsStr::new(path)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    let mut shfileinfo = SHFILEINFOW::default();
    let mut flags = SHGFI_SYSICONINDEX | if size == "small" { SHGFI_SMALLICON } else { SHGFI_LARGEICON };
    
    let mut attributes = windows::Win32::Storage::FileSystem::FILE_FLAGS_AND_ATTRIBUTES(0);
    if use_attributes {
        flags |= windows::Win32::UI::Shell::SHGFI_USEFILEATTRIBUTES;
        attributes = windows::Win32::Storage::FileSystem::FILE_ATTRIBUTE_NORMAL;
    }

    unsafe {
        let result = SHGetFileInfoW(
            PCWSTR(wide_path.as_ptr()),
            attributes,
            Some(&mut shfileinfo),
            std::mem::size_of::<SHFILEINFOW>() as u32,
            flags,
        );

        if result == 0 {
            return Err("Failed to get file icon info".to_string());
        }

        let icon_index = shfileinfo.iIcon;
        // Bump version key to v8
        let cache_key = format!("v8_{}_{}", icon_index, size);

        {
            let cache = ICON_CACHE.lock().unwrap();
            if let Some(data) = cache.get(&cache_key) {
                return Ok(data.clone());
            }
        }

        // Try to get high quality icon (JUMBO = 256, EXTRALARGE = 48)
        let list_id = if size == "small" { 2 } else { SHIL_JUMBO as i32 }; 
        let image_list: windows::core::Result<IImageList> = SHGetImageList(list_id);
        
        let mut hicon = HICON::default();
        if let Ok(list) = image_list {
            hicon = list.GetIcon(icon_index, 0).unwrap_or_default();
        }

        if hicon.is_invalid() {
            let mut shfileinfo_fallback = SHFILEINFOW::default();
            let fallback_flags = SHGFI_ICON | if size == "small" { SHGFI_SMALLICON } else { SHGFI_LARGEICON };
            SHGetFileInfoW(
                PCWSTR(wide_path.as_ptr()),
                windows::Win32::Storage::FileSystem::FILE_FLAGS_AND_ATTRIBUTES(0),
                Some(&mut shfileinfo_fallback),
                std::mem::size_of::<SHFILEINFOW>() as u32,
                fallback_flags,
            );
            hicon = shfileinfo_fallback.hIcon;
        }

        if hicon.is_invalid() {
            return Err("Invalid icon handle".to_string());
        }

        // Target size for display (Retina/High DPI friendly)
        let target_size = if size == "small" { 32 } else { 96 }; 
        
        let icon_bitmap = match icon_to_bitmap(hicon, target_size) {
            Ok(b) => b,
            Err(e) => {
                let _ = DestroyIcon(hicon);
                return Err(format!("Failed to convert icon to bitmap: {}", e));
            }
        };

        let _ = DestroyIcon(hicon);

        // Encode to PNG
        let mut png_buffer = Vec::new();
        let mut cursor = Cursor::new(&mut png_buffer);
        
        if let Err(e) = icon_bitmap.write_to(&mut cursor, image::ImageFormat::Png) {
             return Err(format!("Failed to encode icon to PNG: {}", e));
        }

        {
            let mut cache = ICON_CACHE.lock().unwrap();
            if cache.len() > 1000 {
                if let Some(key) = cache.keys().next().cloned() {
                    cache.remove(&key);
                }
            }
            cache.insert(cache_key, png_buffer.clone());
        }

        Ok(png_buffer)
    }
}

fn icon_to_bitmap(hicon: HICON, target_size: i32) -> Result<RgbaImage, String> {
    unsafe {
        // 1. Get the actual dimensions of the icon provided by Windows
        let mut icon_info = std::mem::zeroed();
        if GetIconInfo(hicon, &mut icon_info).is_err() {
            // Clean up handles from GetIconInfo if it fails
            let _ = DeleteObject(HGDIOBJ(icon_info.hbmColor.0));
            let _ = DeleteObject(HGDIOBJ(icon_info.hbmMask.0));
            return Err("GetIconInfo failed".to_string());
        }
        
        // Clean up handle from GetIconInfo later
        let hbm_color = icon_info.hbmColor;
        let hbm_mask = icon_info.hbmMask;

        let mut actual_width = 0;
        let mut actual_height = 0;
        let mut bitmap: BITMAP = std::mem::zeroed();
        
        if !hbm_color.is_invalid() && GetObjectW(
            HGDIOBJ(hbm_color.0), 
            std::mem::size_of::<BITMAP>() as i32, 
            Some(&mut bitmap as *mut _ as *mut _)
        ) > 0 {
            actual_width = bitmap.bmWidth;
            actual_height = bitmap.bmHeight;
        } else if !hbm_mask.is_invalid() && GetObjectW(
            HGDIOBJ(hbm_mask.0), 
            std::mem::size_of::<BITMAP>() as i32, 
            Some(&mut bitmap as *mut _ as *mut _)
        ) > 0 {
            actual_width = bitmap.bmWidth;
            actual_height = bitmap.bmHeight / 2; // Mask bitmap is double height (XOR + AND)
        }

        if actual_width == 0 || actual_height == 0 {
            let _ = DeleteObject(HGDIOBJ(hbm_color.0));
            let _ = DeleteObject(HGDIOBJ(hbm_mask.0));
            return Err("Could not detect icon size".to_string());
        }

        // 2. Draw icon into a DIB at its NATIVE size to avoid initial aliasing
        let dc = GetDC(None);
        if dc.is_invalid() {
            let _ = DeleteObject(HGDIOBJ(hbm_color.0));
            let _ = DeleteObject(HGDIOBJ(hbm_mask.0));
            return Err("GetDC failed".to_string());
        }

        let mem_dc = CreateCompatibleDC(Some(dc));
        if mem_dc.is_invalid() {
            let _ = ReleaseDC(None, dc);
            let _ = DeleteObject(HGDIOBJ(hbm_color.0));
            let _ = DeleteObject(HGDIOBJ(hbm_mask.0));
            return Err("CreateCompatibleDC failed".to_string());
        }
        
        let bi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: actual_width,
                biHeight: -actual_height, // Top-down
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };

        let mut bits: *mut std::ffi::c_void = std::ptr::null_mut();
        let h_bitmap = CreateDIBSection(
            Some(mem_dc),
            &bi as *const BITMAPINFO,
            DIB_RGB_COLORS,
            &mut bits,
            None,
            0
        ).map_err(|e| {
            let _ = DeleteDC(mem_dc);
            let _ = ReleaseDC(None, dc);
            let _ = DeleteObject(HGDIOBJ(hbm_color.0));
            let _ = DeleteObject(HGDIOBJ(hbm_mask.0));
            format!("CreateDIBSection failed: {}", e)
        })?;

        let old_obj = SelectObject(mem_dc, HGDIOBJ(h_bitmap.0));

        // Draw at native size
        if DrawIconEx(mem_dc, 0, 0, hicon, actual_width, actual_height, 0, None, DI_NORMAL).is_err() {
            let _ = SelectObject(mem_dc, old_obj);
            let _ = DeleteObject(HGDIOBJ(h_bitmap.0));
            let _ = DeleteDC(mem_dc);
            let _ = ReleaseDC(None, dc);
            let _ = DeleteObject(HGDIOBJ(hbm_color.0));
            let _ = DeleteObject(HGDIOBJ(hbm_mask.0));
            return Err("DrawIconEx failed".to_string());
        }

        let num_pixels = (actual_width * actual_height) as usize;
        let src_pixels = std::slice::from_raw_parts(bits as *const u32, num_pixels);
        
        let mut rgba_pixels = Vec::with_capacity(num_pixels * 4);
        for &pixel in src_pixels {
            rgba_pixels.push(((pixel >> 16) & 0xFF) as u8); // R
            rgba_pixels.push(((pixel >> 8) & 0xFF) as u8);  // G
            rgba_pixels.push((pixel & 0xFF) as u8);         // B
            rgba_pixels.push(((pixel >> 24) & 0xFF) as u8); // A
        }

        // Cleanup GDI
        let _ = SelectObject(mem_dc, old_obj);
        let _ = DeleteObject(HGDIOBJ(h_bitmap.0));
        let _ = DeleteDC(mem_dc);
        let _ = ReleaseDC(None, dc);
        let _ = DeleteObject(HGDIOBJ(hbm_color.0));
        let _ = DeleteObject(HGDIOBJ(hbm_mask.0));

        let native_image = RgbaImage::from_raw(actual_width as u32, actual_height as u32, rgba_pixels)
            .ok_or_else(|| "Failed to create RgbaImage".to_string())?;

        // --- SMART RENDERING PIPELINE ---
        
        // 1. CONTENT-AWARE CROPPING (Always enabled to fix tiny icons in large canvases)
        let mut min_x = actual_width as u32;
        let mut max_x = 0;
        let mut min_y = actual_height as u32;
        let mut max_y = 0;
        let mut has_content = false;

        for (x, y, pixel) in native_image.enumerate_pixels() {
            if pixel[3] > 8 { // Alpha threshold
                has_content = true;
                if x < min_x { min_x = x; }
                if x > max_x { max_x = x; }
                if y < min_y { min_y = y; }
                if y > max_y { max_y = y; }
            }
        }

        let processed_img = if has_content {
            let cw = (max_x - min_x) + 1;
            let ch = (max_y - min_y) + 1;
            image::imageops::crop_imm(&native_image, min_x, min_y, cw, ch).to_image()
        } else {
            native_image.clone()
        };

        // 2. SUPER-SAMPLING for Perfect Curves
        // We scale to 2x target size with a smooth filter, then down to 1x with a sharp filter.
        let intermediate_size = (target_size * 2) as u32;
        let margin_factor = 0.94f32; // Slight padding
        let max_dim = (intermediate_size as f32 * margin_factor) as u32;

        let (pw, ph) = (processed_img.width(), processed_img.height());
        let ratio = pw as f32 / ph as f32;
        let (nw, nh) = if ratio > 1.0 {
            (max_dim, (max_dim as f32 / ratio) as u32)
        } else {
            ((max_dim as f32 * ratio) as u32, max_dim)
        };

        // Step A: Upscale to 2x Target with smooth CatmullRom
        let upscaled = image::imageops::resize(&processed_img, nw, nh, image::imageops::FilterType::CatmullRom);

        // Step B: Center on 2x Canvas
        let mut canvas_2x = RgbaImage::new(intermediate_size, intermediate_size);
        let ox = (intermediate_size - nw) / 2;
        let oy = (intermediate_size - nh) / 2;
        image::imageops::overlay(&mut canvas_2x, &upscaled, ox as i64, oy as i64);

        // Step C: Downscale to Final Target with high-precision Lanczos3
        // This is the "magic" for smooth corners.
        Ok(image::imageops::resize(&canvas_2x, target_size as u32, target_size as u32, image::imageops::FilterType::Lanczos3))
    }
}
