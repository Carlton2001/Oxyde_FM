use super::*;
use image::{Delay, Rgba};
use std::io::Write;

fn renderer() -> SvgRenderer {
    SvgRenderer::new(Arc::new(()))
}

#[test]
fn grid_decode_reduces_directly_to_128_pixels_with_triangle() {
    let policy = Policy { output_side: 128, frame_count: 1, ..POLICY };
    let mut source = RgbaImage::new(257, 193);
    for (x, y, pixel) in source.enumerate_pixels_mut() {
        *pixel = Rgba([x as u8, y as u8, (x ^ y) as u8, 255]);
    }
    let mut data = Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(source.clone()).write_to(&mut data, ImageFormat::Png).unwrap();
    let decoded = decode(data.get_ref(), &renderer(), policy).unwrap();
    let mut expected = DynamicImage::ImageRgba8(source).resize(128, 128, image::imageops::FilterType::Triangle).to_rgba8();
    for pixel in expected.pixels_mut() { pixel.0.swap(0, 2); }
    assert_eq!(bytes(&decoded, 0), expected.as_raw().as_slice());
}

#[test]
fn tooltip_reduction_uses_lanczos_without_jpeg_roundtrip() {
    let mut source = RgbaImage::new(257, 193);
    for (x, y, pixel) in source.enumerate_pixels_mut() {
        *pixel = Rgba([x as u8, y as u8, (x ^ y) as u8, 128]);
    }
    let mut data = Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(source.clone()).write_to(&mut data, ImageFormat::Png).unwrap();
    let decoded = decode(data.get_ref(), &renderer(), Policy { output_side: 192, ..POLICY }).unwrap();
    let mut expected = DynamicImage::ImageRgba8(source).resize(192, 192, image::imageops::FilterType::Lanczos3).to_rgba8();
    for pixel in expected.pixels_mut() { pixel.0.swap(0, 2); }
    assert_eq!(bytes(&decoded, 0), expected.as_raw().as_slice());
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
            Message::PreviewTimedOut,
            Message::PreviewAnimationStatic,
        ] {
            let text = i18n.text(message);
            assert!(!text.is_empty() && !text.starts_with('['));
            assert!(!text.contains("%TEMP%"));
        }
    }
}

#[test]
fn worker_timeout_has_a_translated_terminal_message_without_reclassifying_other_errors() {
    let detail = "IO Error: PreviewWorkerTimeout: Preview exceeded 15 seconds";
    assert_eq!(
        PreviewError::Worker(detail.into()).message(),
        Message::PreviewTimedOut
    );
    assert_eq!(worker_message("bad PDF"), Message::PreviewUnavailable);
    for language in crate::ui::i18n::Language::ALL {
        let text = crate::ui::i18n::I18n::for_language(language).text(worker_message(detail));
        assert!(text.contains("15"));
        assert!(!text.starts_with('[') && !text.contains("PreviewWorkerTimeout"));
    }
}
