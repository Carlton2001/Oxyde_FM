//! Raw pixels only across the worker boundary; never decode its reply as an image.
use image::{Delay, Frame, RgbaImage};
use std::io::{self, Cursor, Read, Write};

/// Pixel envelopes enforced by the receiver of each request kind.
#[derive(Clone, Copy)]
pub struct Limits {
    pub pixels: usize,
    pub side: u32,
}
/// Tooltips, thumbnails, icons and Viewer PDF pages.
pub const PREVIEW: Limits = Limits { pixels: 32 * 1024 * 1024, side: 2048 };
/// Full-resolution Viewer images, up to the usual GPU texture side.
pub const VIEWER: Limits = Limits { pixels: 256 * 1024 * 1024, side: 16384 };
pub const MAX_PIXELS: usize = PREVIEW.pixels;
pub const MAX_REPLY: usize = MAX_PIXELS + 16 * 1024;
pub const VIEWER_MAX_REPLY: usize = VIEWER.pixels + 16 * 1024;
/// Complete Viewer documents and PDF page lists; tooltips truncate earlier.
pub const MAX_TEXT: usize = MAX_PIXELS;
/// The Viewer's frame count; the hover policy stops at 256.
const MAX_FRAMES: usize = super::image_preview_decode::VIEWER_POLICY.frame_count;

pub enum Reply {
    /// `natural_size` is the intrinsic display size of a vector source.
    Images { frames: Vec<Frame>, limited: bool, natural_size: Option<(u32, u32)> },
    Text(String),
    Error { limit: bool, detail: String },
}

fn invalid() -> io::Error {
    io::Error::other("Invalid preview worker reply")
}
fn number(input: &mut impl Read) -> io::Result<u32> {
    let mut bytes = [0; 4];
    input.read_exact(&mut bytes)?;
    Ok(u32::from_le_bytes(bytes))
}
fn put(out: &mut impl Write, value: u32) -> io::Result<()> {
    out.write_all(&value.to_le_bytes())
}
fn natural_side(value: u32) -> io::Result<u32> {
    if value == 0 || value > crate::utils::image_preview_decode::MAX_NATURAL_SIDE {
        return Err(invalid());
    }
    Ok(value)
}
fn layout(width: u32, height: u32, limits: Limits) -> io::Result<usize> {
    if width == 0 || height == 0 || width > limits.side || height > limits.side {
        return Err(invalid());
    }
    Ok(width as usize * height as usize * 4)
}

pub fn encode(reply: Reply, out: &mut impl Write) -> io::Result<()> {
    out.write_all(b"OXYP1")?;
    match reply {
        Reply::Images { frames, limited, natural_size } => {
            if frames.is_empty() || frames.len() > MAX_FRAMES {
                return Err(invalid());
            }
            out.write_all(&[0, u8::from(limited) | if natural_size.is_some() { 2 } else { 0 }])?;
            if let Some((width, height)) = natural_size {
                natural_side(width)?;
                natural_side(height)?;
                put(out, width)?;
                put(out, height)?;
            }
            put(out, frames.len() as u32)?;
            let mut total = 0;
            for frame in frames {
                let image = frame.buffer();
                let length = layout(image.width(), image.height(), VIEWER)?;
                total += length;
                if total > VIEWER.pixels || length != image.as_raw().len() {
                    return Err(invalid());
                }
                let (num, den) = frame.delay().numer_denom_ms();
                if den == 0 {
                    return Err(invalid());
                }
                for value in [image.width(), image.height(), num, den] {
                    put(out, value)?;
                }
                out.write_all(image.as_raw())?;
            }
        }
        Reply::Text(text) => {
            if text.len() > MAX_TEXT {
                return Err(invalid());
            }
            out.write_all(&[3])?;
            put(out, text.len() as u32)?;
            out.write_all(text.as_bytes())?;
        }
        Reply::Error { limit, detail } => {
            // Diagnostic text is internal only, never used as a UI translation key.
            let detail: String = detail.chars().take(1000).collect();
            out.write_all(&[if limit { 1 } else { 2 }])?;
            put(out, detail.len() as u32)?;
            out.write_all(detail.as_bytes())?;
        }
    }
    out.flush()
}

pub fn decode(bytes: &[u8]) -> io::Result<Reply> {
    decode_with(bytes, PREVIEW)
}

pub fn decode_with(bytes: &[u8], limits: Limits) -> io::Result<Reply> {
    build(layout_of(bytes, limits)?, bytes)
}

/// As [`decode_with`], reusing `bytes` for a single image: a Viewer image of
/// tens of megapixels is not held twice.
pub fn decode_owned_with(mut bytes: Vec<u8>, limits: Limits) -> io::Result<Reply> {
    match layout_of(&bytes, limits)? {
        Layout::Images { frames, limited, natural_size } if frames.len() == 1 => {
            let span = &frames[0];
            bytes.truncate(span.start + span.length);
            bytes.drain(..span.start);
            let image = RgbaImage::from_raw(span.width, span.height, bytes).ok_or_else(invalid)?;
            Ok(Reply::Images {
                frames: vec![Frame::from_parts(image, 0, 0, span.delay)],
                limited,
                natural_size,
            })
        }
        layout => build(layout, &bytes),
    }
}

/// What a reply holds, checked as [`decode_with`] checks it, without copying
/// its pixels.
pub enum ReplyKind {
    Images(usize),
    /// Whether the text is empty.
    Text(bool),
    Error,
}

pub fn reply_kind_with(bytes: &[u8], limits: Limits) -> io::Result<ReplyKind> {
    Ok(match layout_of(bytes, limits)? {
        Layout::Images { frames, .. } => ReplyKind::Images(frames.len()),
        Layout::Text(range) => ReplyKind::Text(range.is_empty()),
        Layout::Error { .. } => ReplyKind::Error,
    })
}

/// Where a frame's pixels lie in a reply.
struct Span {
    start: usize,
    length: usize,
    width: u32,
    height: u32,
    delay: Delay,
}

/// A checked reply whose pixels and text are still in its bytes.
enum Layout {
    Images { frames: Vec<Span>, limited: bool, natural_size: Option<(u32, u32)> },
    Text(std::ops::Range<usize>),
    Error { limit: bool, detail: std::ops::Range<usize> },
}

fn build(layout: Layout, bytes: &[u8]) -> io::Result<Reply> {
    // Text ranges were checked as UTF-8 by layout_of.
    let text = |range: std::ops::Range<usize>| {
        std::str::from_utf8(&bytes[range]).map(str::to_owned).map_err(|_| invalid())
    };
    Ok(match layout {
        Layout::Images { frames, limited, natural_size } => {
            let mut images = Vec::with_capacity(frames.len());
            for span in frames {
                let mut owned = Vec::new();
                owned.try_reserve_exact(span.length).map_err(io::Error::other)?;
                owned.extend_from_slice(&bytes[span.start..span.start + span.length]);
                let image = RgbaImage::from_raw(span.width, span.height, owned).ok_or_else(invalid)?;
                images.push(Frame::from_parts(image, 0, 0, span.delay));
            }
            Reply::Images { frames: images, limited, natural_size }
        }
        Layout::Text(range) => Reply::Text(text(range)?),
        Layout::Error { limit, detail } => Reply::Error { limit, detail: text(detail)? },
    })
}

fn layout_of(bytes: &[u8], limits: Limits) -> io::Result<Layout> {
    if bytes.len() > limits.pixels + 16 * 1024 || !bytes.starts_with(b"OXYP1") {
        return Err(invalid());
    }
    let mut input = Cursor::new(&bytes[5..]);
    let mut kind = [0];
    input.read_exact(&mut kind)?;
    let layout = match kind[0] {
        0 => {
            let mut flag = [0];
            input.read_exact(&mut flag)?;
            if flag[0] > 3 {
                return Err(invalid());
            }
            let natural_size = if flag[0] & 2 != 0 {
                Some((natural_side(number(&mut input)?)?, natural_side(number(&mut input)?)?))
            } else {
                None
            };
            let count = number(&mut input)? as usize;
            if count == 0 || count > MAX_FRAMES {
                return Err(invalid());
            }
            let mut frames = Vec::with_capacity(count);
            let mut total = 0;
            for _ in 0..count {
                let width = number(&mut input)?;
                let height = number(&mut input)?;
                let num = number(&mut input)?;
                let den = number(&mut input)?;
                if den == 0 {
                    return Err(invalid());
                }
                let length = layout(width, height, limits)?;
                total += length;
                if total > limits.pixels {
                    return Err(invalid());
                }
                let offset = input.position() as usize;
                if input.get_ref().get(offset..offset + length).is_none() {
                    return Err(invalid());
                }
                input.set_position((offset + length) as u64);
                frames.push(Span {
                    start: 5 + offset,
                    length,
                    width,
                    height,
                    // The worker already turns a delay under 20 ms into 100 ms; a reply that
                    // does not must not spin the interface without a pause.
                    delay: if u64::from(num) < 20 * u64::from(den) {
                        Delay::from_numer_denom_ms(100, 1)
                    } else {
                        Delay::from_numer_denom_ms(num, den)
                    },
                });
            }
            Layout::Images {
                frames,
                limited: flag[0] & 1 != 0,
                natural_size,
            }
        }
        1..=3 => {
            let length = number(&mut input)? as usize;
            if length > if kind[0] == 3 { MAX_TEXT } else { 4096 } {
                return Err(invalid());
            }
            let offset = input.position() as usize;
            let text = input
                .get_ref()
                .get(offset..offset + length)
                .ok_or_else(invalid)?;
            std::str::from_utf8(text).map_err(|_| invalid())?;
            input.set_position((offset + length) as u64);
            let range = 5 + offset..5 + offset + length;
            if kind[0] == 3 {
                Layout::Text(range)
            } else {
                Layout::Error {
                    limit: kind[0] == 1,
                    detail: range,
                }
            }
        }
        _ => return Err(invalid()),
    };
    if input.position() as usize != bytes.len() - 5 {
        return Err(invalid());
    }
    Ok(layout)
}

#[cfg(test)]
#[path = "preview_protocol/tests/delays.rs"]
mod delay_tests;

#[cfg(test)]
#[path = "preview_protocol/tests/owned.rs"]
mod owned_tests;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pixels_and_delays_roundtrip_without_image_decoder() {
        let frame = Frame::from_parts(
            RgbaImage::from_pixel(2, 1, image::Rgba([1, 2, 3, 4])),
            0,
            0,
            Delay::from_numer_denom_ms(30, 1),
        );
        let mut bytes = Vec::new();
        encode(
            Reply::Images {
                frames: vec![frame],
                limited: true,
                natural_size: Some((21, 30)),
            },
            &mut bytes,
        )
        .unwrap();
        let Reply::Images { frames, limited, natural_size } = decode(&bytes).unwrap() else {
            panic!()
        };
        assert!(limited);
        assert_eq!(natural_size, Some((21, 30)));
        assert_eq!(frames[0].buffer().as_raw(), &[1, 2, 3, 4, 1, 2, 3, 4]);
        assert_eq!(frames[0].delay().numer_denom_ms(), (30, 1));
        for end in 0..bytes.len() {
            assert!(decode(&bytes[..end]).is_err());
        }
        bytes.push(0);
        assert!(decode(&bytes).is_err());
    }
    #[test]
    fn malformed_sizes_counts_delays_and_flags_are_refused() {
        for (flag, count, width, height, num, den) in [
            (4, 1, 1, 1, 0, 1),
            (0, 0, 1, 1, 0, 1),
            (0, 257, 1, 1, 0, 1),
            (0, 1, 0, 1, 0, 1),
            (0, 1, 1025, 1, 0, 1),
            (0, 1, 1, 1, 0, 0),
        ] {
            let mut bytes = b"OXYP1".to_vec();
            bytes.extend([0, flag]);
            for value in [count, width, height, num, den] {
                put(&mut bytes, value).unwrap();
            }
            bytes.extend([0; 4]);
            assert!(decode(&bytes).is_err());
        }
    }
    #[test]
    fn text_and_errors_are_bounded_and_typed() {
        for reply in [
            Reply::Text("école".into()),
            Reply::Error {
                limit: true,
                detail: "limit".into(),
            },
        ] {
            let mut bytes = Vec::new();
            encode(reply, &mut bytes).unwrap();
            assert!(decode(&bytes).is_ok());
        }
        assert!(encode(Reply::Text("a".repeat(MAX_TEXT + 1)), &mut Vec::new()).is_err());
        assert!(decode(&vec![0; MAX_REPLY + 1]).is_err());
    }
}
