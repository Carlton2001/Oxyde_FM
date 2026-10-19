use crate::models::CommandError;
use image::RgbaImage;
use windows::Data::Pdf::{PdfDocument, PdfPage, PdfPageRenderOptions};
use windows::Graphics::Imaging::{
    BitmapAlphaMode, BitmapDecoder, BitmapPixelFormat, BitmapTransform, ColorManagementMode,
    ExifOrientationMode,
};
use windows::Storage::StorageFile;
use windows::Storage::Streams::InMemoryRandomAccessStream;

const MAX_SIDE: u32 = crate::utils::image_preview_decode::POLICY.output_side;
const MAX_ENCODED_BYTES: u64 = 4 * 1024 * 1024;

// WinRT render options are DIPs, not physical pixels. Round down before native
// rendering; normalize the small DPI rounding afterwards.
fn render_dips(pixels: u32, dpi: u32) -> Result<u32, CommandError> {
    if !(96..=768).contains(&dpi) {
        return Err(CommandError::Other("Unsupported PDF preview DPI".into()));
    }
    Ok(((u64::from(pixels) * 96) / u64::from(dpi)).max(1) as u32)
}

#[derive(Debug, PartialEq, Eq)]
struct PixelLayout {
    width: u32,
    height: u32,
    byte_len: usize,
}

impl PixelLayout {
    #[cfg(test)]
    fn new(width: u32, height: u32) -> Result<Self, CommandError> {
        Self::bounded(width, height, MAX_SIDE)
    }

    fn bounded(width: u32, height: u32, max_side: u32) -> Result<Self, CommandError> {
        if width == 0 || height == 0 || width > max_side || height > max_side {
            return Err(CommandError::Other(format!(
                "Invalid PDF preview dimensions {width}x{height} (maximum {max_side} per side)"
            )));
        }
        let byte_len = (width as usize)
            .checked_mul(height as usize)
            .and_then(|pixels| pixels.checked_mul(4))
            .ok_or_else(|| CommandError::Other("PDF preview buffer size overflow".into()))?;
        Ok(Self {
            width,
            height,
            byte_len,
        })
    }

    #[cfg(test)]
    fn fit(width: f32, height: f32) -> Result<Self, CommandError> {
        Self::fit_within(width, height, MAX_SIDE)
    }

    fn fit_within(width: f32, height: f32, max_side: u32) -> Result<Self, CommandError> {
        if !width.is_finite() || !height.is_finite() || width <= 0.0 || height <= 0.0 {
            return Err(CommandError::Other("Invalid PDF page size".into()));
        }
        // Normalize in f64 to avoid overflowing a scale factor for tiny valid sizes.
        // One pixel is the minimum representable side for an extremely thin page.
        let longest = f64::from(width.max(height));
        let fit = |side: f32| {
            (f64::from(side) / longest * f64::from(max_side))
                .round()
                .clamp(1.0, f64::from(max_side)) as u32
        };
        Self::bounded(fit(width), fit(height), max_side)
    }

    fn copy_bgra(&self, pixels: &[u8]) -> Result<RgbaImage, CommandError> {
        if pixels.len() != self.byte_len {
            return Err(CommandError::Other(
                "Invalid PDF preview pixel buffer length".into(),
            ));
        }
        let mut data = Vec::new();
        data.try_reserve_exact(self.byte_len).map_err(|error| {
            CommandError::Other(format!("Cannot allocate PDF preview buffer: {error}"))
        })?;
        data.extend_from_slice(pixels);
        RgbaImage::from_raw(self.width, self.height, data)
            .ok_or_else(|| CommandError::Other("Invalid PDF preview pixel buffer".into()))
    }
}

struct OwnedPage(PdfPage);

impl Drop for OwnedPage {
    fn drop(&mut self) {
        if let Err(error) = self.0.Close() {
            log::warn!("Could not close PDF preview page: {error}");
        }
    }
}

/// Viewer pages stay within the preview reply's side bound.
pub(super) const VIEWER_SIDE: u32 = crate::utils::preview_protocol::PREVIEW.side;
/// Page sizes are listed once; a larger document still lists its first pages.
const MAX_LISTED_PAGES: u32 = 100_000;

pub(super) fn open(path: &str) -> Result<PdfDocument, CommandError> {
    let abs_path = std::fs::canonicalize(path)?;
    let mut path_str = abs_path.to_string_lossy().to_string();
    if path_str.starts_with(r"\\?\") {
        path_str = path_str.chars().skip(4).collect();
    }
    let file =
        StorageFile::GetFileFromPathAsync(&windows::core::HSTRING::from(&path_str))?.join()?;
    Ok(PdfDocument::LoadFromFileAsync(&file)?.join()?)
}

pub(super) fn get(path: String) -> Result<RgbaImage, CommandError> {
    render(&path, 0, MAX_SIDE)
}

pub(super) fn get_grid(path: String) -> Result<RgbaImage, CommandError> {
    render(&path, 0, 128)
}

/// One `width height` line per page (WinRT DIPs, after rotation).
pub(super) fn sizes_of(document: &PdfDocument) -> Result<String, CommandError> {
    let count = document.PageCount()?.min(MAX_LISTED_PAGES);
    if count == 0 {
        return Err(CommandError::Other("Empty PDF".into()));
    }
    let mut sizes = String::new();
    for index in 0..count {
        let page = OwnedPage(document.GetPage(index)?);
        let size = page.0.Size()?;
        sizes.push_str(&format!("{} {}\n", size.Width, size.Height));
    }
    Ok(sizes)
}

fn render(path: &str, index: u32, max_side: u32) -> Result<RgbaImage, CommandError> {
    let _com = super::windows_com::ComApartment::mta_or_existing()
        .map_err(|error| CommandError::SystemError(error.to_string()))?;
    render_page(&open(path)?, index, max_side)
}

pub(super) fn render_page(document: &PdfDocument, index: u32, max_side: u32) -> Result<RgbaImage, CommandError> {
    if index >= document.PageCount()? {
        return Err(CommandError::Other("PDF page is out of range".into()));
    }
    let page = OwnedPage(document.GetPage(index)?);
    // Size accounts for CropBox, MediaBox and rotation; bound BOTH requested sides
    // before invoking the native renderer, not just the resulting Rust allocation.
    let size = page.0.Size()?;
    let requested = PixelLayout::fit_within(size.Width, size.Height, max_side)?;
    let options = PdfPageRenderOptions::new()?;
    let dpi = unsafe { windows::Win32::UI::HiDpi::GetDpiForSystem() };
    options.SetDestinationWidth(render_dips(requested.width, dpi)?)?;
    options.SetDestinationHeight(render_dips(requested.height, dpi)?)?;
    let stream = InMemoryRandomAccessStream::new()?;
    page.0
        .RenderWithOptionsToStreamAsync(&stream, &options)?
        .join()?;
    let encoded_limit = MAX_ENCODED_BYTES * u64::from((max_side / MAX_SIDE).pow(2).max(1));
    if stream.Size()? > encoded_limit {
        return Err(CommandError::Other(
            "PDF preview exceeds the encoded size limit".into(),
        ));
    }
    let decoder = BitmapDecoder::CreateAsync(&stream)?.join()?;
    // Validate before asking WinRT for uncompressed pixels. Native parser/renderer
    // internals remain outside this allocation policy; the production caller runs
    // this decoder in the resource-limited preview process.
    let layout = PixelLayout::bounded(decoder.PixelWidth()?, decoder.PixelHeight()?, max_side)?;
    let pixels = decoder
        .GetPixelDataTransformedAsync(
            BitmapPixelFormat::Bgra8,
            BitmapAlphaMode::Premultiplied,
            &BitmapTransform::new()?,
            ExifOrientationMode::IgnoreExifOrientation,
            ColorManagementMode::ColorManageToSRgb,
        )?
        .join()?
        .DetachPixelData()?;
    // The UI consumes premultiplied BGRA; never assume the decoder's default format.
    let image = layout.copy_bgra(&pixels)?;
    if image.dimensions() != (requested.width, requested.height) {
        Ok(image::imageops::resize(
            &image,
            requested.width,
            requested.height,
            image::imageops::FilterType::Nearest,
        ))
    } else {
        Ok(image)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn pdf_render_dips_never_request_more_than_the_pixel_envelope_at_supported_dpi() {
        for dpi in [96, 120, 144, 168, 192, 240, 288, 384, 480, 768] {
            for pixels in [1, 51, 256, 512] {
                let dips = render_dips(pixels, dpi).unwrap();
                assert!(dips >= 1);
                if pixels >= 8 {
                    assert!(u64::from(dips) * u64::from(dpi) <= u64::from(pixels) * 96);
                }
            }
        }
        assert!(render_dips(512, 0).is_err());
        assert!(render_dips(512, 769).is_err());
    }

    #[test]
    fn pdf_preview_fit_keeps_both_sides_bounded_for_extreme_ratios() {
        for (width, height, expected) in [
            (72.0, 144.0, (512, 1024)),
            (144.0, 72.0, (1024, 512)),
            (72.0, 720.0, (102, 1024)),
            (720.0, 72.0, (1024, 102)),
            (f32::MIN_POSITIVE, f32::MAX, (1, 1024)),
            (f32::MAX, f32::MIN_POSITIVE, (1024, 1)),
            (f32::from_bits(1), f32::from_bits(1), (1024, 1024)),
        ] {
            let layout = PixelLayout::fit(width, height).unwrap();
            assert_eq!((layout.width, layout.height), expected);
            assert!(layout.byte_len <= 4 * 1024 * 1024);
        }
    }

    #[test]
    fn pdf_preview_fit_rejects_nonfinite_and_nonpositive_page_sizes() {
        for invalid in [0.0, -0.0, -1.0, f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            assert!(PixelLayout::fit(invalid, 72.0).is_err());
            assert!(PixelLayout::fit(72.0, invalid).is_err());
        }
    }

    #[test]
    fn pdf_preview_decoded_layout_checks_both_boundaries_before_allocating() {
        assert_eq!(PixelLayout::new(1024, 1024).unwrap().byte_len, 4 * 1024 * 1024);
        for (width, height) in [(0, 1), (1, 0), (MAX_SIDE + 1, 1), (1, MAX_SIDE + 1), (u32::MAX, u32::MAX)] {
            assert!(PixelLayout::new(width, height).is_err());
        }
    }

    #[test]
    fn pdf_preview_pixel_copy_rejects_truncated_and_extra_data() {
        let layout = PixelLayout::new(1, 1).unwrap();
        for length in [0, 3, 5, 1024] {
            assert!(layout.copy_bgra(&vec![0; length]).is_err());
        }
        assert_eq!(
            layout.copy_bgra(&[10, 20, 30, 40]).unwrap().as_raw(),
            &[10, 20, 30, 40]
        );
    }

    // Small local PDF fixtures: a single opaque red page, no font or image assets.
    fn write_pdf(path: &Path, width: u32, height: u32, rotation: u32) {
        let content = format!("1 0 0 rg 0 0 {width} {height} re f\n");
        let objects = [
            "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
            "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_string(),
            format!("<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {width} {height}] /Rotate {rotation} /Resources << >> /Contents 4 0 R >>"),
            format!("<< /Length {} >>\nstream\n{content}endstream", content.len()),
        ];
        let mut pdf = b"%PDF-1.4\n".to_vec();
        let mut offsets = Vec::new();
        for (index, object) in objects.iter().enumerate() {
            offsets.push(pdf.len());
            pdf.extend_from_slice(format!("{} 0 obj\n{object}\nendobj\n", index + 1).as_bytes());
        }
        let xref = pdf.len();
        pdf.extend_from_slice(b"xref\n0 5\n0000000000 65535 f \n");
        for offset in offsets {
            pdf.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
        }
        pdf.extend_from_slice(
            format!("trailer\n<< /Size 5 /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n").as_bytes(),
        );
        std::fs::write(path, pdf).unwrap();
    }

    #[test]
    fn pdf_preview_native_pipeline_bounds_both_sides_and_preserves_bgra() {
        std::thread::spawn(|| {
            let directory = tempfile::tempdir().unwrap();
            for (width, height, rotation, expected) in [
                (72, 144, 0, (512, 1024)),
                (144, 72, 0, (1024, 512)),
                (72, 720, 0, (102, 1024)),
                (720, 72, 0, (1024, 102)),
                (72, 144, 90, (1024, 512)),
                (72, 72, 0, (1024, 1024)),
            ] {
                let path = directory
                    .path()
                    .join(format!("page-{width}-{height}-{rotation}.pdf"));
                write_pdf(&path, width, height, rotation);
                let image = get(path.to_string_lossy().into_owned()).unwrap();
                assert_eq!(
                    image.dimensions(),
                    expected,
                    "page {width}x{height}, rotation {rotation}"
                );
                assert_eq!(
                    image.get_pixel(image.width() / 2, image.height() / 2).0,
                    [0, 0, 255, 255]
                );
                assert!(image.as_raw().len() <= 4 * 1024 * 1024);
                // Rendering must release file/page resources before returning.
                std::fs::remove_file(path).unwrap();
            }
        })
        .join()
        .unwrap();
    }

    #[test]
    fn viewer_lists_pages_and_renders_them_within_the_viewer_bound() {
        std::thread::spawn(|| {
            let directory = tempfile::tempdir().unwrap();
            let path = directory.path().join("viewer.pdf");
            write_pdf(&path, 72, 144, 0);
            let path = path.to_string_lossy().into_owned();
            let _com = super::super::windows_com::ComApartment::mta_or_existing().unwrap();
            let document = open(&path).unwrap();
            let sizes = sizes_of(&document).unwrap();
            let (width, height) = sizes.trim().split_once(' ').unwrap();
            assert_eq!(height.parse::<f32>().unwrap() / width.parse::<f32>().unwrap(), 2.0);
            let image = render_page(&document, 0, VIEWER_SIDE).unwrap();
            assert_eq!(image.dimensions(), (VIEWER_SIDE / 2, VIEWER_SIDE));
            assert!(render_page(&document, 1, VIEWER_SIDE).is_err());
        })
        .join()
        .unwrap();
    }
}
