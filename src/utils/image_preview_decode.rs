//! Bounded image decoding shared by the frontend and the isolated image worker.
use image::{AnimationDecoder, DynamicImage, Frame, ImageDecoder, ImageFormat, RgbaImage};
use std::{
    fmt,
    io::{Cursor, Read},
    time::{Duration, Instant},
};
#[derive(Clone, Copy)]
pub struct Policy {
    pub encoded_bytes: usize,
    pub svg_bytes: usize,
    pub source_side: u32,
    pub source_bytes: u64,
    pub output_side: u32,
    pub frame_bytes: usize,
    pub frame_count: usize,
    pub animation_time: Duration,
}

pub const POLICY: Policy = Policy {
    encoded_bytes: 64 * 1024 * 1024,
    svg_bytes: 2 * 1024 * 1024,
    source_side: 32768,
    source_bytes: 128 * 1024 * 1024,
    output_side: 1024,
    frame_bytes: 32 * 1024 * 1024,
    frame_count: 256,
    animation_time: Duration::from_secs(2),
};

/// Viewer images keep their source resolution up to the usual GPU texture side
/// (16384) and 256 MiB of pixels (about 67 megapixels). Larger sources are
/// reduced to fit; animations beyond 256 MiB of frames become static.
pub const VIEWER_POLICY: Policy = Policy {
    encoded_bytes: 128 * 1024 * 1024,
    svg_bytes: 16 * 1024 * 1024,
    source_side: 65535,
    source_bytes: 256 * 1024 * 1024,
    output_side: 16384,
    frame_bytes: 256 * 1024 * 1024,
    frame_count: 1024,
    animation_time: Duration::from_secs(10),
};
/// Vector sources are rasterized at most this wide, whatever the policy.
pub const MAX_SVG_SIDE: u32 = 4096;

#[derive(Clone, Debug)]
pub enum PreviewError {
    Limit(&'static str),
    Read(String),
    Decode(String),
    Memory(String),
}

impl fmt::Display for PreviewError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Limit(limit) => write!(formatter, "Image preview exceeds {limit}"),
            Self::Read(error) | Self::Decode(error) | Self::Memory(error) => {
                formatter.write_str(error)
            }
        }
    }
}

pub struct DecodedImage {
    pub frames: Vec<Frame>,
    pub limited: bool,
    /// Intrinsic SVG size: the raster stays large and sharp, the display uses this size.
    pub natural_size: Option<(u32, u32)>,
}

/// Largest intrinsic side carried to the display; the raster is bounded separately.
pub const MAX_NATURAL_SIDE: u32 = 32768;

fn svg_natural_size(data: &[u8]) -> Option<(u32, u32)> {
    // usvg resolves units, viewBox and SVGZ; GPUI does not expose the parsed size.
    let size = usvg::Tree::from_data(data, &usvg::Options::default()).ok()?.size();
    let side = |value: f32| (value.ceil() as u32).clamp(1, MAX_NATURAL_SIDE);
    Some((side(size.width()), side(size.height())))
}

pub fn read_bounded(mut input: impl Read, limit: usize) -> Result<Vec<u8>, PreviewError> {
    let mut data = Vec::new();
    let mut chunk = [0u8; 64 * 1024];
    while data.len() <= limit {
        let requested = chunk.len().min(limit - data.len() + 1);
        let count = input
            .read(&mut chunk[..requested])
            .map_err(|error| PreviewError::Read(error.to_string()))?;
        if count == 0 {
            return Ok(data);
        }
        if count > limit - data.len() {
            return Err(PreviewError::Limit("encoded byte limit"));
        }
        let needed = data.len() + count;
        if needed > data.capacity() {
            let capacity = needed.max(data.capacity().saturating_mul(2)).min(limit);
            data.try_reserve_exact(capacity - data.len())
                .map_err(|error| PreviewError::Memory(error.to_string()))?;
        }
        data.extend_from_slice(&chunk[..count]);
    }
    Err(PreviewError::Limit("encoded byte limit"))
}

fn byte_len(width: u32, height: u32) -> Result<u64, PreviewError> {
    u64::from(width)
        .checked_mul(u64::from(height))
        .and_then(|value| value.checked_mul(4))
        .ok_or(PreviewError::Limit("pixel buffer size"))
}

fn limits(policy: Policy) -> image::Limits {
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(policy.source_side);
    limits.max_image_height = Some(policy.source_side);
    // Decoder internals may ignore max_alloc; output buffers are checked explicitly.
    limits.max_alloc = Some(policy.source_bytes);
    limits
}

fn prepare_decoder(decoder: &mut impl ImageDecoder, policy: Policy) -> Result<(), PreviewError> {
    let (width, height) = decoder.dimensions();
    if width == 0
        || height == 0
        || width > policy.source_side
        || height > policy.source_side
        || byte_len(width, height)? > policy.source_bytes
        || decoder.total_bytes() > policy.source_bytes
    {
        return Err(PreviewError::Limit(
            "source image dimensions or decoded bytes",
        ));
    }
    decoder.set_limits(limits(policy)).map_err(classify_decode)
}

fn classify_decode(error: image::ImageError) -> PreviewError {
    match error {
        image::ImageError::Limits(_) => PreviewError::Limit("decoder limits"),
        error => PreviewError::Decode(error.to_string()),
    }
}

fn fitted_bgra(image: DynamicImage, policy: Policy) -> RgbaImage {
    let image = if image.width() > policy.output_side || image.height() > policy.output_side {
        let filter = if policy.output_side <= 128 { image::imageops::FilterType::Triangle }
            else { image::imageops::FilterType::Gaussian };
        image.resize(policy.output_side, policy.output_side, filter)
    } else {
        image
    };
    // Move an existing RGBA buffer instead of cloning every preview/frame.
    let mut image = image.into_rgba8();
    for pixel in image.as_chunks_mut::<4>().0 {
        pixel.swap(0, 2);
    }
    image
}

fn static_image(
    mut decoder: impl ImageDecoder,
    policy: Policy,
) -> Result<Vec<Frame>, PreviewError> {
    prepare_decoder(&mut decoder, policy)?;
    let orientation = decoder.orientation().map_err(classify_decode)?;
    let mut image = DynamicImage::from_decoder(decoder).map_err(classify_decode)?;
    image.apply_orientation(orientation);
    Ok(vec![Frame::new(fitted_bgra(image, policy))])
}

fn collect_frames(
    mut input: impl Iterator<Item = image::ImageResult<Frame>>,
    policy: Policy,
) -> Result<(Vec<Frame>, bool), PreviewError> {
    let started = Instant::now();
    let mut frames: Vec<Frame> = Vec::new();
    let mut retained = 0usize;
    loop {
        if !frames.is_empty() && started.elapsed() >= policy.animation_time {
            frames.truncate(1);
            return Ok((frames, true));
        }
        let Some(frame) = input.next() else {
            break;
        };
        let frame = frame.map_err(classify_decode)?;
        if frames.len() >= policy.frame_count {
            frames.truncate(1);
            return Ok((frames, true));
        }
        let (width, height) = frame.buffer().dimensions();
        if width == 0
            || height == 0
            || width > policy.source_side
            || height > policy.source_side
            || byte_len(width, height)? > policy.source_bytes
        {
            return Err(PreviewError::Limit("animation frame dimensions"));
        }
        let delay = frame.delay();
        let buffer = fitted_bgra(DynamicImage::ImageRgba8(frame.into_buffer()), policy);
        let Some(total) = retained
            .checked_add(buffer.as_raw().len())
            .filter(|total| *total <= policy.frame_bytes)
        else {
            if frames.is_empty() {
                return Err(PreviewError::Limit("retained animation bytes"));
            }
            frames.truncate(1);
            return Ok((frames, true));
        };
        retained = total;
        frames.push(Frame::from_parts(buffer, 0, 0, delay));
    }
    if frames.is_empty() {
        return Err(PreviewError::Decode(
            "Image contains no decodable frame".into(),
        ));
    }
    Ok((frames, false))
}

fn raster(
    data: &[u8],
    format: ImageFormat,
    policy: Policy,
) -> Result<(Vec<Frame>, bool), PreviewError> {
    match format {
        ImageFormat::Gif => {
            let mut decoder =
                image::codecs::gif::GifDecoder::new(Cursor::new(data)).map_err(classify_decode)?;
            prepare_decoder(&mut decoder, policy)?;
            collect_frames(decoder.into_frames(), policy)
        }
        ImageFormat::WebP => {
            let mut decoder = image::codecs::webp::WebPDecoder::new(Cursor::new(data))
                .map_err(classify_decode)?;
            prepare_decoder(&mut decoder, policy)?;
            if decoder.has_animation() {
                decoder
                    .set_background_color(image::Rgba([0, 0, 0, 0]))
                    .map_err(classify_decode)?;
                collect_frames(decoder.into_frames(), policy)
            } else {
                Ok((static_image(decoder, policy)?, false))
            }
        }
        format => {
            let mut reader = image::ImageReader::with_format(Cursor::new(data), format);
            reader.limits(limits(policy));
            let decoder = reader.into_decoder().map_err(classify_decode)?;
            Ok((static_image(decoder, policy)?, false))
        }
    }
}

/// Decodes SVG documents, which the raster decoders do not handle.
pub type SvgDecoder<'a> = dyn Fn(&[u8], Policy) -> Result<Vec<Frame>, PreviewError> + 'a;

pub fn decode(
    data: &[u8],
    policy: Policy,
    svg: &SvgDecoder<'_>,
) -> Result<DecodedImage, PreviewError> {
    let (frames, limited, natural_size) = match image::guess_format(data) {
        Ok(format) => {
            let (frames, limited) = raster(data, format, policy)?;
            (frames, limited, None)
        }
        Err(_) => {
            // The renderer has already bounded SVGZ expansion before this second parse.
            let frames = svg(data, policy)?;
            (frames, false, svg_natural_size(data))
        }
    };
    Ok(DecodedImage { frames, limited, natural_size })
}

#[cfg(test)]
#[path = "image_preview_decode/tests/buffers.rs"]
mod buffer_tests;
