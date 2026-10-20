//! GPUI SVG rasterization stays outside the backend library and its native workers.
use gpui::{DevicePixels, SvgRenderer, SvgSize};
use image::{DynamicImage, Frame, RgbaImage};
use oxyde_lib::utils::image_preview_decode::{read_bounded, Policy, PreviewError, MAX_SVG_SIDE};

fn byte_len(width: u32, height: u32) -> Result<u64, PreviewError> {
    u64::from(width)
        .checked_mul(u64::from(height))
        .and_then(|value| value.checked_mul(4))
        .ok_or(PreviewError::Limit("pixel buffer size"))
}

pub(crate) fn svg(
    data: &[u8],
    renderer: &SvgRenderer,
    policy: Policy,
) -> Result<Vec<Frame>, PreviewError> {
    // usvg accepts SVGZ too. Bound expansion before giving plaintext to its parser.
    let data = if data.starts_with(&[0x1f, 0x8b]) {
        std::borrow::Cow::Owned(read_bounded(
            flate2::read::GzDecoder::new(data),
            policy.svg_bytes,
        )?)
    } else {
        if data.len() > policy.svg_bytes {
            return Err(PreviewError::Limit("SVG byte limit"));
        }
        std::borrow::Cow::Borrowed(data)
    };
    let parsed = renderer
        .parse_svg(&data)
        .map_err(|error| PreviewError::Decode(error.to_string()))?;
    // GPUI's public Size variant preserves aspect ratio from width, and its
    // rasterizer caps height at 8192. Request at most MAX_SVG_SIDE pixels wide,
    // then fit both returned sides to our final output envelope. Parser/filter
    // scratch allocations and embedded images are not bounded by this policy.
    let side = policy.output_side.min(MAX_SVG_SIDE);
    let rendered = renderer
        .render_parsed(
            &parsed,
            SvgSize::Size(gpui::size(DevicePixels(side as i32), DevicePixels(side as i32))),
        )
        .map_err(|error| PreviewError::Decode(error.to_string()))?;
    let size = rendered.size(0);
    let width =
        u32::try_from(size.width.0).map_err(|_| PreviewError::Limit("SVG output dimensions"))?;
    let height =
        u32::try_from(size.height.0).map_err(|_| PreviewError::Limit("SVG output dimensions"))?;
    if width == 0 || height == 0 || width > side || height > 8192 {
        return Err(PreviewError::Limit("SVG output dimensions"));
    }
    let pixels = rendered
        .as_bytes(0)
        .ok_or_else(|| PreviewError::Decode("Missing SVG pixels".into()))?;
    if pixels.len() as u64 != byte_len(width, height)? {
        return Err(PreviewError::Decode("Invalid SVG pixel buffer".into()));
    }
    let mut copy = Vec::new();
    copy.try_reserve_exact(pixels.len())
        .map_err(|error| PreviewError::Memory(error.to_string()))?;
    copy.extend_from_slice(pixels);
    let image = RgbaImage::from_raw(width, height, copy)
        .ok_or_else(|| PreviewError::Decode("Invalid SVG pixel buffer".into()))?;
    // thumbnail also upscales: only call it when the raster exceeds the envelope.
    // These channels are already BGRA, so resize them without a second swap.
    let image = if width > policy.output_side || height > policy.output_side {
        DynamicImage::ImageRgba8(image)
            .thumbnail(policy.output_side, policy.output_side)
            .to_rgba8()
    } else {
        image
    };
    Ok(vec![Frame::new(image)])
}
