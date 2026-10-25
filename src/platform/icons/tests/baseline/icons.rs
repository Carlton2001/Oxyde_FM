// Frozen benchmark baseline: fcf724874a05fc4c58ab4f6f3093b9bafe13a645:src/platform/icons.rs
#![cfg(target_os = "windows")]

use crate::models::CommandError;
use image::RgbaImage;
use lazy_static::lazy_static;
use std::io::Cursor;
use std::os::windows::ffi::OsStrExt;
use std::path::PathBuf;
use std::sync::Mutex;
use windows::core::PCWSTR;
use windows::Win32::UI::Controls::IImageList;
use windows::Win32::UI::Shell::{
    SHGetFileInfoW, SHGetImageList, SHFILEINFOW, SHGFI_ICON, SHGFI_LARGEICON, SHGFI_SMALLICON,
    SHGFI_SYSICONINDEX, SHIL_JUMBO,
};
use windows::Win32::UI::WindowsAndMessaging::HICON;

lazy_static! {
    static ref ICON_CACHE: Mutex<super::windows_icon_cache::IconCache> = Mutex::new(Default::default());
}

pub fn purge_icon_cache() {
    super::windows_icon_pixels::clear();
    let mut cache = ICON_CACHE.lock().unwrap();
    cache.clear();
}

fn shell_icon_path(path: &str) -> String {
    path.replace('/', "\\")
}

pub(super) fn cache_key(path: &str, size: &str) -> String {
    let path = shell_icon_path(path);
    format!("{}_{}", blake3::hash(path.as_bytes()).to_hex(), size)
}

pub fn get_file_icon(path: String, size: String) -> Result<Vec<u8>, CommandError> {
    // Breadcrumbs use forward slashes; Shell APIs require Windows separators.
    let path = shell_icon_path(&path);
    let cache_dir = crate::app_paths::cache_dir().map(|p| p.join("icons"));

    // Virtual path logic (copying the folder "C:\Windows" spirit)
    let (real_path, use_attributes) = if let Some(ext) = path.strip_prefix("oxyde_ext_") {
        (format!("C:\\template.{}", ext), true)
    } else if path == "oxyde_file_generic" {
        ("C:\\template".to_string(), true)
    } else if path == "oxyde_dir_generic" {
        ("C:\\Windows".to_string(), false)
    } else {
        (path.clone(), false)
    };

    // Cache key is based on the input path to be 100% stable
    // Hash the exact path to avoid collisions and overly long cache filenames.
    let cache_key = cache_key(&path, &size);

    extract_icon_png(&real_path, &size, use_attributes, cache_dir, cache_key)
        .map_err(|e| CommandError::SystemError(format!("Failed to extract icon: {}", e)))
}

pub fn extract_icon_png(
    path: &str,
    size: &str,
    use_attributes: bool,
    cache_dir: Option<PathBuf>,
    cache_key: String,
) -> Result<Vec<u8>, String> {
    super::windows_icon_cache::cached_or_generate(
        &ICON_CACHE,
        cache_dir.as_deref(),
        &cache_key,
        || generate_icon_png(path, size, use_attributes),
    )
}

fn generate_icon_png(path: &str, size: &str, use_attributes: bool) -> Result<Vec<u8>, String> {
    let wide_path: Vec<u16> = std::ffi::OsStr::new(path)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    let mut shfileinfo = SHFILEINFOW::default();
    let mut flags = SHGFI_SYSICONINDEX
        | if size == "small" {
            SHGFI_SMALLICON
        } else {
            SHGFI_LARGEICON
        };

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

        if result == 0 { return Err("Shell icon lookup failed".into()); }
        let icon_index = shfileinfo.iIcon;

        // Try to get high quality icon (JUMBO = 256, EXTRALARGE = 48)
        let list_id = if size == "small" {
            2
        } else {
            SHIL_JUMBO as i32
        };
        let image_list: windows::core::Result<IImageList> = SHGetImageList(list_id);

        let mut hicon = HICON::default();
        if let Ok(list) = image_list {
            hicon = list.GetIcon(icon_index, 0).unwrap_or_default();
        }

        if hicon.is_invalid() {
            let mut shfileinfo_fallback = SHFILEINFOW::default();
            let fallback_flags = SHGFI_ICON
                | if size == "small" {
                    SHGFI_SMALLICON
                } else {
                    SHGFI_LARGEICON
                };
            SHGetFileInfoW(
                PCWSTR(wide_path.as_ptr()),
                attributes,
                Some(&mut shfileinfo_fallback),
                std::mem::size_of::<SHFILEINFOW>() as u32,
                fallback_flags | if use_attributes { windows::Win32::UI::Shell::SHGFI_USEFILEATTRIBUTES } else { windows::Win32::UI::Shell::SHGFI_FLAGS(0) },
            );
            hicon = shfileinfo_fallback.hIcon;
        }

        if hicon.is_invalid() {
            return Err("Invalid icon handle".to_string());
        }

        // Target size for display (Retina/High DPI friendly)
        let target_size = if size == "small" { 32 } else { 96 };

        // The converter owns the Shell icon and releases it even on an error/unwind.
        let icon_bitmap = icon_to_bitmap(hicon, target_size)
            .map_err(|error| format!("Failed to convert icon to bitmap: {error}"))?;

        // Encode to PNG
        let mut png_buffer = Vec::new();
        let mut cursor = Cursor::new(&mut png_buffer);

        if let Err(e) = icon_bitmap.write_to(&mut cursor, image::ImageFormat::Png) {
            return Err(format!("Failed to encode icon to PNG: {}", e));
        }

        Ok(png_buffer)
    }
}

fn icon_to_bitmap(hicon: HICON, target_size: i32) -> Result<RgbaImage, String> {
    let native_image = super::windows_icon_bitmap::into_image(hicon, target_size)?;
    let actual_width = native_image.width();
    let actual_height = native_image.height();
    // --- SMART RENDERING PIPELINE ---

    // 1. CONTENT-AWARE CROPPING (Always enabled to fix tiny icons in large canvases)
    let mut min_x = actual_width;
    let mut max_x = 0;
    let mut min_y = actual_height;
    let mut max_y = 0;
    let mut has_content = false;

    for (x, y, pixel) in native_image.enumerate_pixels() {
        if pixel[3] > 8 {
            // Alpha threshold
            has_content = true;
            if x < min_x {
                min_x = x;
            }
            if x > max_x {
                max_x = x;
            }
            if y < min_y {
                min_y = y;
            }
            if y > max_y {
                max_y = y;
            }
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
    let upscaled = image::imageops::resize(
        &processed_img,
        nw,
        nh,
        image::imageops::FilterType::CatmullRom,
    );

    // Step B: Center on 2x Canvas
    let mut canvas_2x = RgbaImage::new(intermediate_size, intermediate_size);
    let ox = (intermediate_size - nw) / 2;
    let oy = (intermediate_size - nh) / 2;
    image::imageops::overlay(&mut canvas_2x, &upscaled, ox as i64, oy as i64);

    // Step C: Downscale to Final Target with high-precision Lanczos3
    // This is the "magic" for smooth corners.
    Ok(image::imageops::resize(
        &canvas_2x,
        target_size as u32,
        target_size as u32,
        image::imageops::FilterType::Lanczos3,
    ))
}
