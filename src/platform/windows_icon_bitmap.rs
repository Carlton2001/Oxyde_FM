use image::RgbaImage;
use std::ffi::c_void;
use std::ptr::NonNull;
use windows::Win32::Graphics::Gdi::{
    CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, GdiFlush, GetObjectW,
    SelectObject, BITMAP, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, HBITMAP, HDC,
    HGDIOBJ,
};
use windows::Win32::UI::WindowsAndMessaging::{
    DestroyIcon, DrawIconEx, GetIconInfo, DI_NORMAL, HICON, ICONINFO,
};

const MAX_NATIVE_SIDE: i32 = 1024;
const MAX_TARGET_SIDE: i32 = 256;

#[derive(Debug)]
struct IconLayout {
    width: i32,
    height: i32,
    pixel_count: usize,
    byte_len: usize,
}

impl IconLayout {
    fn color(width: i32, height: i32) -> Result<Self, String> {
        if width <= 0 || height <= 0 || width > MAX_NATIVE_SIDE || height > MAX_NATIVE_SIDE {
            return Err(format!("Invalid native icon dimensions {width}x{height} (maximum {MAX_NATIVE_SIDE} per side)"));
        }
        let pixel_count = (width as usize)
            .checked_mul(height as usize)
            .ok_or("Native icon pixel count overflow")?;
        let byte_len = pixel_count
            .checked_mul(4)
            .ok_or("Native icon buffer size overflow")?;
        Ok(Self {
            width,
            height,
            pixel_count,
            byte_len,
        })
    }

    fn monochrome(width: i32, mask_height: i32) -> Result<Self, String> {
        if mask_height <= 0 || mask_height % 2 != 0 {
            return Err("Invalid monochrome icon mask height".into());
        }
        Self::color(width, mask_height / 2)
    }
}

fn validate_target(target: i32) -> Result<(), String> {
    if target <= 0 || target > MAX_TARGET_SIDE {
        return Err(format!(
            "Invalid icon display size {target} (maximum {MAX_TARGET_SIDE})"
        ));
    }
    Ok(())
}

struct OwnedIcon(HICON);

impl Drop for OwnedIcon {
    fn drop(&mut self) {
        if !self.0.is_invalid() {
            unsafe {
                let _ = DestroyIcon(self.0);
            }
        }
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

struct IconBitmaps {
    color: OwnedBitmap,
    mask: OwnedBitmap,
}

impl IconBitmaps {
    fn for_icon(icon: HICON) -> Result<Self, String> {
        let mut info = ICONINFO::default();
        let result = unsafe { GetIconInfo(icon, &mut info) };
        let bitmaps = Self {
            color: OwnedBitmap(info.hbmColor),
            mask: OwnedBitmap(info.hbmMask),
        };
        result.map_err(|error| format!("GetIconInfo failed: {error}"))?;
        Ok(bitmaps)
    }

    fn layout(&self) -> Result<IconLayout, String> {
        // A failed color description must not reinterpret its ordinary mask as
        // a double-height monochrome mask.
        if !self.color.0.is_invalid() {
            let bitmap = describe_bitmap(self.color.0)?;
            IconLayout::color(bitmap.bmWidth, bitmap.bmHeight)
        } else {
            let bitmap = describe_bitmap(self.mask.0)?;
            IconLayout::monochrome(bitmap.bmWidth, bitmap.bmHeight)
        }
    }
}

fn describe_bitmap(bitmap: HBITMAP) -> Result<BITMAP, String> {
    let mut description = BITMAP::default();
    let bytes = unsafe {
        GetObjectW(
            HGDIOBJ(bitmap.0),
            std::mem::size_of::<BITMAP>() as i32,
            Some(&mut description as *mut BITMAP as *mut c_void),
        )
    };
    if bytes != std::mem::size_of::<BITMAP>() as i32 {
        return Err("GetObjectW failed to describe native icon bitmap".into());
    }
    Ok(description)
}

struct OwnedMemoryDc(HDC);

impl OwnedMemoryDc {
    fn create() -> Result<Self, String> {
        let dc = unsafe { CreateCompatibleDC(None) };
        if dc.is_invalid() {
            return Err("CreateCompatibleDC failed for icon".into());
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

struct DibSurface {
    // Fields drop in declaration order: destroy the DC before the bitmap, even
    // if restoring the previous selection fails or saved DC states still refer to it.
    dc: OwnedMemoryDc,
    _bitmap: OwnedBitmap,
    previous: HGDIOBJ,
    bits: NonNull<u32>,
}

impl DibSurface {
    fn new(layout: &IconLayout) -> Result<Self, String> {
        let dc = OwnedMemoryDc::create()?;
        let info = BITMAPINFO {
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
        let mut address = std::ptr::null_mut();
        let bitmap = OwnedBitmap(
            unsafe { CreateDIBSection(Some(dc.0), &info, DIB_RGB_COLORS, &mut address, None, 0) }
                .map_err(|error| format!("CreateDIBSection failed for icon: {error}"))?,
        );
        let bits = NonNull::new(address.cast::<u32>()).ok_or("Icon DIB has no pixel buffer")?;
        if bits.as_ptr() as usize % std::mem::align_of::<u32>() != 0 {
            return Err("Icon DIB pixel buffer is misaligned".into());
        }
        // Fully initialize the surface, including transparent areas not drawn by GDI.
        unsafe {
            std::ptr::write_bytes(bits.as_ptr().cast::<u8>(), 0, layout.byte_len);
        }
        let previous = unsafe { SelectObject(dc.0, HGDIOBJ(bitmap.0 .0)) };
        if previous.is_invalid() {
            return Err("SelectObject failed for icon DIB".into());
        }
        Ok(Self {
            dc,
            _bitmap: bitmap,
            previous,
            bits,
        })
    }

    fn draw(&self, icon: HICON, layout: &IconLayout) -> Result<(), String> {
        unsafe {
            DrawIconEx(
                self.dc.0,
                0,
                0,
                icon,
                layout.width,
                layout.height,
                0,
                None,
                DI_NORMAL,
            )
        }
        .map_err(|error| format!("DrawIconEx failed: {error}"))?;
        // The DIB pointer must not be read until GDI has completed the drawing.
        if !unsafe { GdiFlush() }.as_bool() {
            return Err("GdiFlush failed after icon drawing".into());
        }
        Ok(())
    }
}

impl Drop for DibSurface {
    fn drop(&mut self) {
        unsafe {
            let _ = SelectObject(self.dc.0, self.previous);
        }
    }
}

// Consumes an owned, non-shared HICON, returning native RGBA pixels for the
// existing crop/resampling pipeline. The caller's display size is checked first.
pub(super) fn into_image(icon: HICON, target: i32) -> Result<RgbaImage, String> {
    let icon = OwnedIcon(icon);
    validate_target(target)?;
    let bitmaps = IconBitmaps::for_icon(icon.0)?;
    let layout = bitmaps.layout()?;
    let mut rgba = Vec::new();
    rgba.try_reserve_exact(layout.byte_len)
        .map_err(|error| format!("Cannot allocate icon buffer: {error}"))?;
    let surface = DibSurface::new(&layout)?;
    surface.draw(icon.0, &layout)?;
    // The initialized, aligned allocation covers all checked 32bpp pixels and
    // remains owned by the live surface; draw() has synchronized GDI access.
    let pixels = unsafe { std::slice::from_raw_parts(surface.bits.as_ptr(), layout.pixel_count) };
    for &pixel in pixels {
        rgba.extend_from_slice(&[
            ((pixel >> 16) & 0xff) as u8,
            ((pixel >> 8) & 0xff) as u8,
            (pixel & 0xff) as u8,
            ((pixel >> 24) & 0xff) as u8,
        ]);
    }
    RgbaImage::from_raw(layout.width as u32, layout.height as u32, rgba)
        .ok_or_else(|| "Failed to create native icon image".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows::Win32::Graphics::Gdi::{CreateBitmap, SaveDC};
    use windows::Win32::UI::WindowsAndMessaging::CreateIconIndirect;

    fn monochrome_icon(width: i32, height: i32) -> HICON {
        let stride = (width as usize + 15) / 16 * 2;
        // AND=0, XOR=0 yields a black monochrome icon, independent of row order.
        let data = vec![0; stride * height as usize * 2];
        let mask = OwnedBitmap(unsafe {
            CreateBitmap(width, height * 2, 1, 1, Some(data.as_ptr().cast()))
        });
        assert!(!mask.0.is_invalid());
        unsafe {
            CreateIconIndirect(&ICONINFO {
                fIcon: true.into(),
                hbmMask: mask.0,
                ..Default::default()
            })
        }
        .unwrap()
    }

    #[test]
    fn color_mask_and_display_limits_reject_invalid_geometry() {
        for (width, height) in [
            (0, 1),
            (1, 0),
            (-1, 1),
            (1, -1),
            (i32::MIN, 1),
            (1, i32::MIN),
            (i32::MAX, 1),
            (1, i32::MAX),
            (1025, 1),
            (1, 1025),
        ] {
            assert!(IconLayout::color(width, height).is_err());
        }
        let layout = IconLayout::color(1024, 1024).unwrap();
        assert_eq!(layout.byte_len, 4 * 1024 * 1024);
        assert_eq!(layout.pixel_count, 1024 * 1024);
        assert_eq!(IconLayout::monochrome(1024, 2048).unwrap().height, 1024);
        for height in [0, -2, 1, 3, 2049, 2050, i32::MIN, i32::MAX] {
            assert!(IconLayout::monochrome(2, height).is_err());
        }
        for target in [0, -1, 257, i32::MIN, i32::MAX] {
            assert!(validate_target(target).is_err());
        }
        for target in [1, 32, 96, 256] {
            assert!(validate_target(target).is_ok());
        }
    }

    #[test]
    fn monochrome_icon_uses_half_the_mask_height_and_releases_icon() {
        let icon = monochrome_icon(16, 4);
        let image = into_image(icon, 32).unwrap();
        assert_eq!((image.width(), image.height()), (16, 4));
        assert!(image.pixels().all(|pixel| pixel.0[0..3] == [0, 0, 0]));
        assert!(IconBitmaps::for_icon(icon).is_err());
    }

    #[test]
    fn invalid_native_icon_is_rejected() {
        assert!(into_image(HICON::default(), 32).is_err());
    }

    #[test]
    fn failed_draw_releases_initialized_dib_and_memory_dc() {
        let layout = IconLayout::color(2, 2).unwrap();
        let surface = DibSurface::new(&layout).unwrap();
        let bitmap = surface._bitmap.0;
        let dc = surface.dc.0;
        let pixels =
            unsafe { std::slice::from_raw_parts(surface.bits.as_ptr(), layout.pixel_count) };
        assert!(pixels.iter().all(|pixel| *pixel == 0));
        assert!(surface.draw(HICON::default(), &layout).is_err());
        drop(surface);
        assert!(describe_bitmap(bitmap).is_err());
        assert_eq!(unsafe { SaveDC(dc) }, 0);
    }

    #[test]
    fn unwind_releases_icon_copies_and_selected_surface_with_saved_dc_state() {
        let icon = OwnedIcon(monochrome_icon(16, 4));
        let bitmaps = IconBitmaps::for_icon(icon.0).unwrap();
        let layout = bitmaps.layout().unwrap();
        let surface = DibSurface::new(&layout).unwrap();
        let icon_handle = icon.0;
        let mask_handle = bitmaps.mask.0;
        let dib_handle = surface._bitmap.0;
        let dc_handle = surface.dc.0;
        assert!(describe_bitmap(mask_handle).is_ok());
        assert_ne!(unsafe { SaveDC(dc_handle) }, 0);
        let result = std::panic::catch_unwind(move || {
            let _icon = icon;
            let _bitmaps = bitmaps;
            let _surface = surface;
            panic!("injected native icon conversion unwind");
        });
        assert!(result.is_err());
        assert!(IconBitmaps::for_icon(icon_handle).is_err());
        assert!(describe_bitmap(mask_handle).is_err());
        assert!(describe_bitmap(dib_handle).is_err());
        assert_eq!(unsafe { SaveDC(dc_handle) }, 0);
    }
}
