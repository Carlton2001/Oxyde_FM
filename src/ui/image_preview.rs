//! Bounded decoding for local images displayed in tooltips.
use crate::ui::i18n::Message;
use gpui::{DevicePixels, RenderImage, SvgRenderer, SvgSize};
use image::{AnimationDecoder, DynamicImage, Frame, ImageDecoder, ImageFormat, RgbaImage};
use std::fmt;
use std::io::{Cursor, Read};
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

#[derive(Clone, Copy)]
struct Policy {
    encoded_bytes: usize,
    svg_bytes: usize,
    source_side: u32,
    source_bytes: u64,
    output_side: u32,
    frame_bytes: usize,
    frame_count: usize,
    animation_time: Duration,
}

const POLICY: Policy = Policy {
    encoded_bytes: 64 * 1024 * 1024,
    svg_bytes: 2 * 1024 * 1024,
    source_side: 32768,
    source_bytes: 128 * 1024 * 1024,
    output_side: 1024,
    frame_bytes: 32 * 1024 * 1024,
    frame_count: 256,
    animation_time: Duration::from_secs(2),
};

#[derive(Clone, Debug)]
pub(crate) enum PreviewError {
    Limit(&'static str),
    Read(String),
    Decode(String),
    Memory(String),
    Worker(String),
}

impl PreviewError {
    pub(crate) fn message(&self) -> Message {
        match self {
            Self::Limit(_) => Message::PreviewImageLimit,
            _ => Message::PreviewUnavailable,
        }
    }
}

impl fmt::Display for PreviewError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Limit(limit) => write!(formatter, "Image preview exceeds {limit}"),
            Self::Read(error) | Self::Decode(error) | Self::Memory(error) | Self::Worker(error) => {
                formatter.write_str(error)
            }
        }
    }
}

#[derive(Clone)]
pub(crate) struct PreviewImage {
    pub(crate) image: Arc<RenderImage>,
    pub(crate) notice: Option<Message>,
}

fn read_bounded(mut input: impl Read, limit: usize) -> Result<Vec<u8>, PreviewError> {
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
        image.thumbnail(policy.output_side, policy.output_side)
    } else {
        image
    };
    let mut image = image.to_rgba8();
    for pixel in image.chunks_exact_mut(4) {
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

fn svg(data: &[u8], renderer: &SvgRenderer, policy: Policy) -> Result<Vec<Frame>, PreviewError> {
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
    // rasterizer caps height at 8192. Request 1024px width (<=32MiB main buffer),
    // then fit both returned sides to our final output envelope. Parser/filter
    // scratch allocations and embedded images are not bounded by this policy.
    let rendered = renderer
        .render_parsed(
            &parsed,
            SvgSize::Size(gpui::size(
                DevicePixels(policy.output_side as i32),
                DevicePixels(policy.output_side as i32),
            )),
        )
        .map_err(|error| PreviewError::Decode(error.to_string()))?;
    let size = rendered.size(0);
    let width =
        u32::try_from(size.width.0).map_err(|_| PreviewError::Limit("SVG output dimensions"))?;
    let height =
        u32::try_from(size.height.0).map_err(|_| PreviewError::Limit("SVG output dimensions"))?;
    if width == 0 || height == 0 || width > policy.output_side || height > 8192 {
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
    // These channels are already BGRA, so resize them without a second swap.
    let image = DynamicImage::ImageRgba8(image)
        .thumbnail(policy.output_side, policy.output_side)
        .to_rgba8();
    Ok(vec![Frame::new(image)])
}

fn decode(
    data: &[u8],
    renderer: &SvgRenderer,
    policy: Policy,
) -> Result<PreviewImage, PreviewError> {
    let (frames, limited) = match image::guess_format(data) {
        Ok(format) => raster(data, format, policy)?,
        Err(_) => (svg(data, renderer, policy)?, false),
    };
    Ok(PreviewImage {
        image: Arc::new(RenderImage::new(frames)),
        notice: limited.then_some(Message::PreviewAnimationStatic),
    })
}

pub(crate) fn load_path(path: &Path, renderer: &SvgRenderer) -> Result<PreviewImage, PreviewError> {
    let file = std::fs::File::open(path).map_err(|error| PreviewError::Read(error.to_string()))?;
    if file
        .metadata()
        .map_err(|error| PreviewError::Read(error.to_string()))?
        .len()
        > POLICY.encoded_bytes as u64
    {
        return Err(PreviewError::Limit("encoded byte limit (64 MiB)"));
    }
    let data = read_bounded(file, POLICY.encoded_bytes)?;
    decode(&data, renderer, POLICY)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Delay, Rgba};
    use std::io::Write;

    fn renderer() -> SvgRenderer {
        SvgRenderer::new(Arc::new(()))
    }
    fn png(width: u32, height: u32) -> Vec<u8> {
        let mut data = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(RgbaImage::from_pixel(
            width,
            height,
            Rgba([10, 20, 30, 128]),
        ))
        .write_to(&mut data, ImageFormat::Png)
        .unwrap();
        data.into_inner()
    }
    fn gif(count: usize) -> Vec<u8> {
        let mut data = Vec::new();
        let frames = (0..count).map(|index| {
            Frame::from_parts(
                RgbaImage::from_pixel(
                    2,
                    1,
                    Rgba([
                        if index % 2 == 0 { 255 } else { 0 },
                        0,
                        if index % 2 == 0 { 0 } else { 255 },
                        255,
                    ]),
                ),
                0,
                0,
                Delay::from_numer_denom_ms(if index % 2 == 0 { 30 } else { 70 }, 1),
            )
        });
        image::codecs::gif::GifEncoder::new(&mut data)
            .encode_frames(frames)
            .unwrap();
        data
    }
    fn bytes(image: &PreviewImage, index: usize) -> &[u8] {
        image.image.as_bytes(index).unwrap()
    }

    #[test]
    fn image_preview_read_caps_actual_bytes_and_accepts_exact_boundary() {
        let mut input = Cursor::new(vec![0; 9]);
        assert!(matches!(
            read_bounded(&mut input, 8),
            Err(PreviewError::Limit(_))
        ));
        assert_eq!(input.position(), 9);
        let mut input = Cursor::new(vec![0; 8]);
        assert_eq!(read_bounded(&mut input, 8).unwrap().len(), 8);
        assert_eq!(read_bounded(Cursor::new(Vec::new()), 8).unwrap().len(), 0);
    }

    #[test]
    fn image_preview_source_limits_accept_boundary_and_refuse_next_pixel() {
        let renderer = renderer();
        for (width, height) in [(32768, 1), (1, 32768)] {
            let image = decode(&png(width, height), &renderer, POLICY).unwrap();
            assert!(image.image.size(0).width.0 <= 1024 && image.image.size(0).height.0 <= 1024);
        }
        for (width, height) in [(32769, 1), (1, 32769)] {
            assert!(matches!(
                decode(&png(width, height), &renderer, POLICY),
                Err(PreviewError::Limit(_))
            ));
        }
        let policy = Policy {
            source_bytes: 7,
            ..POLICY
        };
        assert!(matches!(
            decode(&png(2, 1), &renderer, policy),
            Err(PreviewError::Limit(_))
        ));
    }

    #[test]
    fn image_preview_static_keeps_bgra_alpha_and_bounds_final_side() {
        let renderer = renderer();
        let image = decode(&png(2, 1), &renderer, POLICY).unwrap();
        assert_eq!(
            (image.image.size(0).width.0, image.image.size(0).height.0),
            (2, 1)
        );
        assert_eq!(bytes(&image, 0).len(), 8);
        assert_eq!(&bytes(&image, 0)[..4], &[30, 20, 10, 128]);
        assert!(image.notice.is_none());
        let image = decode(&png(2048, 1024), &renderer, POLICY).unwrap();
        assert_eq!(
            (image.image.size(0).width.0, image.image.size(0).height.0),
            (1024, 512)
        );
        assert_eq!(bytes(&image, 0).len(), 1024 * 512 * 4);
    }

    #[test]
    fn image_preview_gif_preserves_frames_delays_and_bgra() {
        let image = decode(&gif(2), &renderer(), POLICY).unwrap();
        assert_eq!(image.image.frame_count(), 2);
        assert_eq!(image.image.delay(0).numer_denom_ms(), (30, 1));
        assert_eq!(image.image.delay(1).numer_denom_ms(), (70, 1));
        assert_eq!(&bytes(&image, 0)[..4], &[0, 0, 255, 255]);
        assert_eq!(&bytes(&image, 1)[..4], &[255, 0, 0, 255]);
        assert!(image.notice.is_none());
    }

    #[test]
    fn image_preview_animation_count_boundary_then_falls_back_to_first_frame() {
        let renderer = renderer();
        let image = decode(&gif(256), &renderer, POLICY).unwrap();
        assert_eq!(image.image.frame_count(), 256);
        assert!(image.notice.is_none());
        let image = decode(&gif(257), &renderer, POLICY).unwrap();
        assert_eq!(image.image.frame_count(), 1);
        assert_eq!(image.notice, Some(Message::PreviewAnimationStatic));
        assert_eq!(&bytes(&image, 0)[..4], &[0, 0, 255, 255]);
    }

    #[test]
    fn image_preview_animation_byte_budget_boundary_and_cooperative_deadline() {
        let renderer = renderer();
        for (budget, count, limited) in [(16, 2, false), (15, 1, true)] {
            let image = decode(
                &gif(2),
                &renderer,
                Policy {
                    frame_bytes: budget,
                    ..POLICY
                },
            )
            .unwrap();
            assert_eq!(image.image.frame_count(), count);
            assert_eq!(image.notice.is_some(), limited);
        }
        let image = decode(
            &gif(2),
            &renderer,
            Policy {
                animation_time: Duration::ZERO,
                ..POLICY
            },
        )
        .unwrap();
        assert_eq!(image.image.frame_count(), 1);
        assert!(image.notice.is_some());
    }

    // Small animated RIFF fixtures following the WebP animation container spec.
    fn animated_webp() -> Vec<u8> {
        fn chunk(tag: &[u8; 4], bytes: &[u8]) -> Vec<u8> {
            let mut out = tag.to_vec();
            out.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
            out.extend_from_slice(bytes);
            if bytes.len() % 2 == 1 {
                out.push(0);
            }
            out
        }
        let mut body = b"WEBP".to_vec();
        body.extend(chunk(b"VP8X", &[2, 0, 0, 0, 1, 0, 0, 0, 0, 0]));
        body.extend(chunk(b"ANIM", &[0; 6]));
        for (rgb, delay) in [([255, 0, 0], 30), ([0, 0, 255], 70)] {
            let mut static_image = Vec::new();
            image::codecs::webp::WebPEncoder::new_lossless(&mut static_image)
                .encode(&[rgb, rgb].concat(), 2, 1, image::ExtendedColorType::Rgb8)
                .unwrap();
            let mut frame = vec![0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, delay, 0, 0, 2];
            frame.extend_from_slice(&static_image[12..]);
            body.extend(chunk(b"ANMF", &frame));
        }
        let mut data = b"RIFF".to_vec();
        data.extend_from_slice(&(body.len() as u32).to_le_bytes());
        data.extend(body);
        data
    }

    #[test]
    fn image_preview_webp_animation_is_preserved_and_uses_same_budget() {
        let renderer = renderer();
        let data = animated_webp();
        let image = decode(&data, &renderer, POLICY).unwrap();
        assert_eq!(image.image.frame_count(), 2);
        assert_eq!(&bytes(&image, 0)[..4], &[0, 0, 255, 255]);
        assert_eq!(&bytes(&image, 1)[..4], &[255, 0, 0, 255]);
        assert_eq!(image.image.delay(1).numer_denom_ms(), (70, 1));
        let image = decode(
            &data,
            &renderer,
            Policy {
                frame_count: 1,
                ..POLICY
            },
        )
        .unwrap();
        assert_eq!(image.image.frame_count(), 1);
        assert_eq!(image.notice, Some(Message::PreviewAnimationStatic));
    }

    #[test]
    fn image_preview_jpeg_exif_orientation_is_preserved() {
        let renderer = renderer();
        let mut data = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(RgbaImage::new(2, 1))
            .to_rgb8()
            .write_to(&mut data, ImageFormat::Jpeg)
            .unwrap();
        let jpeg = data.into_inner();
        let exif = [
            b'E', b'x', b'i', b'f', 0, 0, b'I', b'I', 42, 0, 8, 0, 0, 0, 1, 0, 0x12, 1, 3, 0, 1, 0,
            0, 0, 6, 0, 0, 0, 0, 0, 0, 0,
        ];
        let mut with_exif = jpeg[..2].to_vec();
        with_exif.extend_from_slice(&[0xff, 0xe1]);
        with_exif.extend_from_slice(&((exif.len() + 2) as u16).to_be_bytes());
        with_exif.extend_from_slice(&exif);
        with_exif.extend_from_slice(&jpeg[2..]);
        let image = decode(&with_exif, &renderer, POLICY).unwrap();
        assert_eq!(
            (image.image.size(0).width.0, image.image.size(0).height.0),
            (1, 2)
        );
    }

    #[test]
    fn image_preview_svg_renders_colors_and_preserves_tall_aspect_ratio() {
        let renderer = renderer();
        for (width, height) in [(2000, 1000), (2, 20)] {
            let data = format!(
                r#"<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}"><rect width="100%" height="100%" fill="red"/></svg>"#
            );
            let image = decode(data.as_bytes(), &renderer, POLICY).unwrap();
            let size = image.image.size(0);
            assert!(size.width.0 <= 1024 && size.height.0 <= 1024);
            assert!(
                (size.width.0 as f32 / size.height.0 as f32 - width as f32 / height as f32).abs()
                    < 0.01
            );
            let offset = ((size.width.0 as usize * size.height.0 as usize / 2)
                + size.width.0 as usize / 2)
                * 4;
            assert_eq!(&bytes(&image, 0)[offset..offset + 4], &[0, 0, 255, 255]);
        }
    }

    #[test]
    fn image_preview_svgz_expansion_and_plain_svg_are_bounded() {
        let renderer = renderer();
        let data = b"<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"2\" height=\"1\"/>";
        let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(data).unwrap();
        let compressed = encoder.finish().unwrap();
        assert!(decode(&compressed, &renderer, POLICY).is_ok());
        let policy = Policy {
            svg_bytes: data.len() - 1,
            ..POLICY
        };
        assert!(matches!(
            decode(data, &renderer, policy),
            Err(PreviewError::Limit(_))
        ));
        assert!(matches!(
            decode(&compressed, &renderer, policy),
            Err(PreviewError::Limit(_))
        ));
    }

    #[test]
    fn image_preview_invalid_content_and_missing_file_have_terminal_messages() {
        let renderer = renderer();
        let error = decode(b"invalid image", &renderer, POLICY).err().unwrap();
        assert_eq!(error.message(), Message::PreviewUnavailable);
        assert_eq!(
            PreviewError::Limit("fixture").message(),
            Message::PreviewImageLimit
        );
        let directory = tempfile::tempdir().unwrap();
        let error = load_path(&directory.path().join("absent.png"), &renderer)
            .err()
            .unwrap();
        assert!(matches!(error, PreviewError::Read(_)));
        for language in crate::ui::i18n::Language::ALL {
            let i18n = crate::ui::i18n::I18n::for_language(language);
            for message in [
                error.message(),
                Message::PreviewImageLimit,
                Message::PreviewAnimationStatic,
            ] {
                let text = i18n.text(message);
                assert!(!text.is_empty() && !text.starts_with('['));
                assert!(!text.contains("%TEMP%"));
            }
        }
    }
}
