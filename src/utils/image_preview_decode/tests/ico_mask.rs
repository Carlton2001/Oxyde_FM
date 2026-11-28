use super::apply_legacy_mask;

/// A one-entry icon of `side` pixels, 32 bits with zero alpha, red where the mask is
/// opaque; `transparent` says which pixels its AND mask hides, top row first.
fn legacy_icon(side: u32, transparent: impl Fn(u32, u32) -> bool) -> Vec<u8> {
    let stride = side.div_ceil(32) * 4;
    let mut colors = Vec::new();
    let mut mask = Vec::new();
    for y in (0..side).rev() {
        for _ in 0..side {
            colors.extend_from_slice(&[0, 0, 0xff, 0]);
        }
        let mut row = vec![0u8; stride as usize];
        for x in 0..side {
            if transparent(x, y) {
                row[(x / 8) as usize] |= 0x80 >> (x % 8);
            }
        }
        mask.extend(row);
    }
    let mut bmp = Vec::new();
    for value in [40u32, side, side * 2] {
        bmp.extend_from_slice(&value.to_le_bytes());
    }
    bmp.extend_from_slice(&1u16.to_le_bytes());
    bmp.extend_from_slice(&32u16.to_le_bytes());
    bmp.extend_from_slice(&[0; 24]);
    bmp.extend(colors);
    bmp.extend(mask);
    let mut file = vec![0, 0, 1, 0, 1, 0, side as u8, side as u8, 0, 0];
    file.extend_from_slice(&1u16.to_le_bytes());
    file.extend_from_slice(&32u16.to_le_bytes());
    file.extend_from_slice(&(bmp.len() as u32).to_le_bytes());
    file.extend_from_slice(&22u32.to_le_bytes());
    file.extend(bmp);
    file
}

#[test]
fn a_legacy_icon_takes_the_transparency_of_its_mask() {
    let data = legacy_icon(32, |x, y| x < 8 && y < 4);
    let decoded = super::super::decode(&data, super::super::POLICY, &|_, _| unreachable!()).unwrap();
    // Previews are BGRA.
    let pixels = decoded.frames[0].buffer();
    assert_eq!(pixels.get_pixel(0, 0).0[3], 0);
    assert_eq!(pixels.get_pixel(7, 3).0[3], 0);
    assert_eq!(pixels.get_pixel(8, 0).0, [0, 0, 0xff, 0xff]);
    assert_eq!(pixels.get_pixel(31, 31).0, [0, 0, 0xff, 0xff]);
}

#[test]
fn an_image_with_alpha_or_without_a_mask_to_read_is_left_as_decoded() {
    let mut image = image::RgbaImage::new(2, 2);
    image.put_pixel(1, 1, image::Rgba([1, 2, 3, 0x80]));
    let kept = apply_legacy_mask(b"", image::DynamicImage::ImageRgba8(image.clone()));
    assert_eq!(kept.to_rgba8(), image);
    let empty = image::RgbaImage::new(2, 2);
    let unread = apply_legacy_mask(b"not an icon", image::DynamicImage::ImageRgba8(empty.clone()));
    assert_eq!(unread.to_rgba8(), empty);
}
