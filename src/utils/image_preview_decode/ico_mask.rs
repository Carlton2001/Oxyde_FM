//! Legacy icons: a 32-bit entry whose alpha is zero everywhere, as icons from before
//! Windows XP are, is transparent only where its AND mask says so. The decoder reads
//! the alpha alone and would draw such an icon empty; Windows draws it with its mask.

/// Gives `image`, decoded from the icon file `data`, the transparency of its AND mask
/// when every pixel's alpha is zero and the entry decoded is a 32-bit BMP. Anything
/// else (an image with alpha, a PNG entry) is left as decoded.
pub(crate) fn apply_legacy_mask(data: &[u8], image: image::DynamicImage) -> image::DynamicImage {
    let image::DynamicImage::ImageRgba8(mut pixels) = image else { return image };
    if pixels.pixels().any(|pixel| pixel[3] != 0) {
        return image::DynamicImage::ImageRgba8(pixels);
    }
    let (width, height) = pixels.dimensions();
    if let Some(transparent) = mask(data, width, height) {
        for (index, pixel) in pixels.pixels_mut().enumerate() {
            pixel[3] = if transparent[index] { 0 } else { 0xff };
        }
    }
    image::DynamicImage::ImageRgba8(pixels)
}

/// The AND mask of the 32-bit BMP entry of `width` by `height`, top row first: true
/// where the icon is transparent.
fn mask(data: &[u8], width: u32, height: u32) -> Option<Vec<bool>> {
    let read_u16 = |at: usize| data.get(at..at + 2).map(|bytes| u16::from_le_bytes([bytes[0], bytes[1]]));
    let read_u32 = |at: usize| data.get(at..at + 4).map(|bytes| u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]));
    if read_u16(0)? != 0 || read_u16(2)? != 1 {
        return None;
    }
    let count = usize::from(read_u16(4)?);
    let (width, height) = (usize::try_from(width).ok()?, usize::try_from(height).ok()?);
    (0..count).find_map(|entry| {
        let at = 6 + entry * 16;
        let offset = usize::try_from(read_u32(at + 12)?).ok()?;
        // The directory says 0 for 256; the BMP header has the real size.
        let header = usize::try_from(read_u32(offset)?).ok()?;
        let bmp_width = usize::try_from(read_u32(offset + 4)?).ok()?;
        let bmp_height = usize::try_from(read_u32(offset + 8)?).ok()? / 2;
        if header < 40 || bmp_width != width || bmp_height != height || read_u16(offset + 14)? != 32 {
            return None;
        }
        let colors = offset.checked_add(header)?;
        let mask_start = colors.checked_add(width.checked_mul(4)?.checked_mul(height)?)?;
        let stride = width.div_ceil(32) * 4;
        let rows = data.get(mask_start..mask_start.checked_add(stride.checked_mul(height)?)?)?;
        let mut transparent = Vec::with_capacity(width * height);
        // Rows are stored bottom-up.
        for row in rows.chunks_exact(stride).rev() {
            transparent.extend((0..width).map(|x| row[x / 8] >> (7 - x % 8) & 1 == 1));
        }
        Some(transparent)
    })
}

#[cfg(test)]
#[path = "tests/ico_mask.rs"]
mod tests;
