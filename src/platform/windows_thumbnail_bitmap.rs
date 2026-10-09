use crate::models::CommandError;
use image::RgbaImage;
use std::ffi::c_void;
use windows::Win32::Graphics::Gdi::{
    CreateCompatibleDC, DeleteDC, DeleteObject, GetDIBits, GetObjectW, BITMAP, BITMAPINFO,
    BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, HBITMAP, HDC, HGDIOBJ,
};

// Match the cache's side admission limit, leaving margin over the Shell's 256px request.
const MAX_SIDE: i32 = 1024;

#[derive(Debug)]
struct BitmapLayout {
    width: i32,
    height: i32,
    byte_len: usize,
}

impl BitmapLayout {
    fn new(width: i32, height: i32) -> Result<Self, CommandError> {
        if width <= 0 || height <= 0 || width > MAX_SIDE || height > MAX_SIDE {
            return Err(CommandError::Other(format!(
                "Invalid Shell thumbnail dimensions {width}x{height} (maximum {MAX_SIDE} per side)"
            )));
        }
        // The 32bpp output has four bytes per pixel, with DWORD-aligned rows.
        let byte_len = (width as usize)
            .checked_mul(height as usize)
            .and_then(|pixels| pixels.checked_mul(4))
            .ok_or_else(|| CommandError::Other("Shell thumbnail buffer size overflow".into()))?;
        Ok(Self {
            width,
            height,
            byte_len,
        })
    }
}

struct OwnedBitmap(HBITMAP);

impl Drop for OwnedBitmap {
    fn drop(&mut self) {
        if !self.0.is_invalid() {
            unsafe {
                let _ = DeleteObject(HGDIOBJ(self.0 .0));
            }
        }
    }
}

struct OwnedMemoryDc(HDC);

impl OwnedMemoryDc {
    fn create() -> Result<Self, CommandError> {
        let dc = unsafe { CreateCompatibleDC(None) };
        if dc.is_invalid() {
            return Err(CommandError::SystemError(
                "CreateCompatibleDC failed for Shell thumbnail".into(),
            ));
        }
        Ok(Self(dc))
    }
}

impl Drop for OwnedMemoryDc {
    fn drop(&mut self) {
        unsafe {
            let _ = DeleteDC(self.0);
        }
    }
}

// Consumes the caller-owned, unselected bitmap returned by the Shell image factory.
pub(super) fn into_image(hbitmap: HBITMAP) -> Result<RgbaImage, CommandError> {
    let bitmap = OwnedBitmap(hbitmap);
    let mut description = BITMAP::default();
    let described = unsafe {
        GetObjectW(
            HGDIOBJ(bitmap.0 .0),
            std::mem::size_of::<BITMAP>() as i32,
            Some(&mut description as *mut BITMAP as *mut c_void),
        )
    };
    if described != std::mem::size_of::<BITMAP>() as i32 {
        return Err(CommandError::SystemError(
            "GetObjectW failed to describe Shell thumbnail bitmap".into(),
        ));
    }
    let layout = BitmapLayout::new(description.bmWidth, description.bmHeight)?;
    let mut pixels = Vec::new();
    pixels.try_reserve_exact(layout.byte_len).map_err(|error| {
        CommandError::Other(format!("Cannot allocate Shell thumbnail buffer: {error}"))
    })?;
    pixels.resize(layout.byte_len, 0u8);
    let dc = OwnedMemoryDc::create()?;
    let mut info = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: layout.width,
            biHeight: -layout.height,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            biSizeImage: layout.byte_len as u32,
            ..Default::default()
        },
        ..Default::default()
    };
    // Positive dimensions are validated before negation/casts. The live buffer holds
    // every requested 32bpp row; the owned bitmap is never selected into this DC.
    let copied_lines = unsafe {
        GetDIBits(
            dc.0,
            bitmap.0,
            0,
            layout.height as u32,
            Some(pixels.as_mut_ptr() as *mut c_void),
            &mut info,
            DIB_RGB_COLORS,
        )
    };
    if copied_lines != layout.height {
        return Err(CommandError::SystemError(format!(
            "GetDIBits copied {copied_lines} of {} Shell thumbnail rows",
            layout.height
        )));
    }
    // Preserve the native BGRA order consumed by RenderImage, without a channel swap.
    RgbaImage::from_raw(layout.width as u32, layout.height as u32, pixels)
        .ok_or_else(|| CommandError::Other("Invalid Shell thumbnail bitmap buffer".into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows::Win32::Graphics::Gdi::{CreateDIBSection, SaveDC};

    fn bitmap(width: i32, height: i32, pixels: Option<&[u8]>) -> HBITMAP {
        let info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width,
                biHeight: -height,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        unsafe {
            let mut bits = std::ptr::null_mut();
            let bitmap = CreateDIBSection(None, &info, DIB_RGB_COLORS, &mut bits, None, 0).unwrap();
            assert!(!bits.is_null());
            let len = width as usize * height as usize * 4;
            std::ptr::write_bytes(bits as *mut u8, 0, len);
            if let Some(pixels) = pixels {
                assert_eq!(pixels.len(), len);
                std::ptr::copy_nonoverlapping(pixels.as_ptr(), bits as *mut u8, len);
            }
            bitmap
        }
    }

    fn bitmap_description_bytes(bitmap: HBITMAP) -> i32 {
        let mut description = BITMAP::default();
        unsafe {
            GetObjectW(
                HGDIOBJ(bitmap.0),
                std::mem::size_of::<BITMAP>() as i32,
                Some(&mut description as *mut BITMAP as *mut c_void),
            )
        }
    }

    #[test]
    fn native_thumbnail_rejects_bitmap_beyond_dimension_budget() {
        for (width, height, accepted) in [
            (1024, 1, true),
            (1, 1024, true),
            (1025, 1, false),
            (1, 1025, false),
        ] {
            let bitmap = bitmap(width, height, None);
            let result = into_image(bitmap);
            assert_eq!(result.is_ok(), accepted, "dimensions {width}x{height}");
            assert_eq!(
                bitmap_description_bytes(bitmap),
                0,
                "Bitmap leaked after conversion"
            );
        }
    }

    #[test]
    fn native_thumbnail_rejects_failed_bitmap_description() {
        assert!(
            into_image(HBITMAP::default()).is_err(),
            "Failed GetObjectW was accepted"
        );
    }

    #[test]
    fn native_thumbnail_preserves_bgra_pixels_and_top_down_rows() {
        let pixels = [1, 2, 3, 255, 10, 20, 30, 255, 4, 5, 6, 255, 40, 50, 60, 255];
        let bitmap = bitmap(2, 2, Some(&pixels));
        let image = into_image(bitmap).unwrap();
        assert_eq!((image.width(), image.height()), (2, 2));
        assert_eq!(image.as_raw(), &pixels);
        assert_eq!(bitmap_description_bytes(bitmap), 0);
    }

    #[test]
    fn invalid_and_excessive_dimensions_fail_without_overflow_or_native_allocations() {
        for (width, height) in [
            (0, 1),
            (1, 0),
            (-1, 1),
            (1, -1),
            (i32::MIN, 1),
            (1, i32::MIN),
            (i32::MAX, 1),
            (1, i32::MAX),
            (32768, 32768),
            (46341, 46341),
        ] {
            assert!(BitmapLayout::new(width, height).is_err());
        }
        for (width, height) in [(1, 1), (256, 256), (1024, 1024)] {
            let layout = BitmapLayout::new(width, height).unwrap();
            assert_eq!(layout.byte_len, width as usize * height as usize * 4);
            assert!(layout.byte_len <= 4 * 1024 * 1024);
        }
    }

    #[test]
    fn square_bitmap_at_limit_has_a_four_mib_buffer() {
        let bitmap = bitmap(1024, 1024, None);
        let image = into_image(bitmap).unwrap();
        assert_eq!((image.width(), image.height()), (1024, 1024));
        assert_eq!(image.as_raw().len(), 4 * 1024 * 1024);
        assert!(image.as_raw().iter().all(|byte| *byte == 0));
        assert_eq!(bitmap_description_bytes(bitmap), 0);
    }

    #[test]
    fn unwind_releases_bitmap_and_memory_dc() {
        let bitmap = bitmap(2, 2, None);
        let dc = OwnedMemoryDc::create().unwrap();
        let dc_handle = dc.0;
        assert_ne!(unsafe { SaveDC(dc_handle) }, 0);
        let bitmap_guard = OwnedBitmap(bitmap);
        let result = std::panic::catch_unwind(move || {
            let _bitmap = bitmap_guard;
            let _dc = dc;
            panic!("injected thumbnail conversion unwind");
        });
        assert!(result.is_err());
        assert_eq!(bitmap_description_bytes(bitmap), 0);
        // Check that native operations fail; GetObjectType still reports a type
        // for a deleted handle on this Windows host.
        assert_eq!(unsafe { SaveDC(dc_handle) }, 0);
    }
}
