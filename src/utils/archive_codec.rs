//! Stream compressions shared by compressed TAR and single compressed files.
use serde::{Deserialize, Serialize};
use std::io::{self, Read, Write};

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum Codec {
    Gzip,
    Xz,
    Bzip2,
    Zstd,
    Lz4,
    Lzma,
}

/// Same order as the protocol quality: fast, normal, best.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Level {
    Fast,
    Normal,
    Best,
}

// LZMA-alone streams can declare dictionaries up to 4 GiB; refuse them before allocation.
const LZMA_MEMORY: u64 = 1024 * 1024 * 1024;
// Bound multithreaded xz under the worker's 1 GiB job, with room for I/O buffers.
const XZ_ENCODER_MEMORY: u64 = 512 * 1024 * 1024;

fn invalid_stream() -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, "Invalid compressed stream")
}

impl Codec {
    /// Single compressed file suffix, without the TAR forms.
    pub fn from_extension(extension: &str) -> Option<Self> {
        Some(match extension {
            "gz" => Self::Gzip,
            "xz" => Self::Xz,
            "bz2" => Self::Bzip2,
            "zst" => Self::Zstd,
            "lz4" => Self::Lz4,
            "lzma" => Self::Lzma,
            _ => return None,
        })
    }

    fn magic_matches(self, header: &[u8]) -> bool {
        match self {
            Self::Gzip => header.starts_with(&[0x1f, 0x8b]),
            Self::Xz => header.starts_with(&[0xfd, b'7', b'z', b'X', b'Z', 0]),
            Self::Bzip2 => header.starts_with(b"BZh"),
            Self::Zstd => {
                header.len() >= 4 && {
                    let magic = u32::from_le_bytes([header[0], header[1], header[2], header[3]]);
                    // A standard frame, or the skippable frame pzstd writes first.
                    magic == 0xFD2F_B528 || magic & 0xFFFF_FFF0 == 0x184D_2A50
                }
            }
            Self::Lz4 => {
                header.starts_with(&[0x04, 0x22, 0x4d, 0x18])
                    || header.starts_with(&[0x02, 0x21, 0x4c, 0x18])
            }
            // LZMA-alone has no magic; its first byte packs lc/lp/pb below 225.
            Self::Lzma => header.first().is_some_and(|props| *props < 225),
        }
    }

    /// Decoders accept concatenated members, as `gzip -d`, `xz -d` and `bzip2 -d` do.
    /// The prefix check refuses an empty or foreign file instead of decoding nothing.
    pub fn decoder<'a, R: Read + 'a>(self, mut source: R) -> io::Result<Box<dyn Read + 'a>> {
        let mut prefix = [0u8; 6];
        let mut length = 0;
        while length < prefix.len() {
            let count = source.read(&mut prefix[length..])?;
            if count == 0 {
                break;
            }
            length += count;
        }
        if !self.magic_matches(&prefix[..length]) {
            return Err(invalid_stream());
        }
        let source = io::Cursor::new(prefix[..length].to_vec()).chain(source);
        Ok(match self {
            Self::Gzip => Box::new(flate2::read::MultiGzDecoder::new(source)),
            Self::Xz => Box::new(xz2::read::XzDecoder::new_multi_decoder(source)),
            Self::Bzip2 => Box::new(bzip2::read::MultiBzDecoder::new(source)),
            Self::Zstd => Box::new(zstd::stream::read::Decoder::new(source)?),
            Self::Lz4 => Box::new(Lz4Frames(lz4_flex::frame::FrameDecoder::new(Watched {
                inner: source,
                ended: false,
            }))),
            Self::Lzma => Box::new(xz2::read::XzDecoder::new_stream(
                source,
                xz2::stream::Stream::new_lzma_decoder(LZMA_MEMORY).map_err(io::Error::other)?,
            )),
        })
    }

    /// LZMA-alone is legacy and stays read-only.
    pub fn writable(self) -> bool {
        self != Self::Lzma
    }

    pub fn encoder<W: Write>(self, output: W, level: Level, threads: usize) -> io::Result<Encoder<W>> {
        let pick = |fast: u32, normal: u32, best: u32| match level {
            Level::Fast => fast,
            Level::Normal => normal,
            Level::Best => best,
        };
        Ok(match self {
            Self::Gzip => Encoder::Gzip(flate2::write::GzEncoder::new(
                output,
                flate2::Compression::new(pick(1, 6, 9)),
            )),
            Self::Bzip2 => Encoder::Bzip2(bzip2::write::BzEncoder::new(
                output,
                bzip2::Compression::new(pick(1, 6, 9)),
            )),
            Self::Xz => Encoder::Xz(xz2::write::XzEncoder::new_stream(
                output,
                // Preset 8 has the 32 MiB dictionary of 7z best; preset 9 needs 674 MiB per thread.
                xz_stream(pick(1, 6, 8), threads)?,
            )),
            Self::Zstd => {
                let level = pick(1, 3, 19) as i32;
                let mut encoder = zstd::stream::write::Encoder::new(output, level)?;
                let count = threads.min(if level >= 19 { 2 } else { 8 });
                encoder.multithread(if count > 1 { count as u32 } else { 0 })?;
                encoder.window_log(23)?;
                Encoder::Zstd(encoder)
            }
            Self::Lz4 => Encoder::Lz4(lz4_flex::frame::FrameEncoder::with_frame_info(
                lz4_flex::frame::FrameInfo::new()
                    .block_size(lz4_flex::frame::BlockSize::Max4MB)
                    .content_checksum(true),
                output,
            )),
            Self::Lzma => return Err(io::Error::new(io::ErrorKind::Unsupported, "LZMA output is not supported")),
        })
    }
}

/// `FrameDecoder` returns 0 at the end of each frame; `lz4 -d` continues with the next.
struct Lz4Frames<R: Read>(lz4_flex::frame::FrameDecoder<Watched<R>>);
struct Watched<R> {
    inner: R,
    ended: bool,
}
impl<R: Read> Read for Watched<R> {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        let count = self.inner.read(bytes)?;
        self.ended |= count == 0 && !bytes.is_empty();
        Ok(count)
    }
}
impl<R: Read> Read for Lz4Frames<R> {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        loop {
            // Each empty result consumed a frame end or a header: the source advances.
            let count = self.0.read(bytes)?;
            if count != 0 || bytes.is_empty() || self.0.get_ref().ended {
                return Ok(count);
            }
        }
    }
}

fn xz_stream(preset: u32, threads: usize) -> io::Result<xz2::stream::Stream> {
    let mut count = threads.max(1) as u32;
    loop {
        let mut builder = xz2::stream::MtStreamBuilder::new();
        builder.preset(preset).threads(count).check(xz2::stream::Check::Crc64);
        if count == 1 || builder.memusage() <= XZ_ENCODER_MEMORY {
            return builder.encoder().map_err(io::Error::other);
        }
        count -= 1;
    }
}

pub enum Encoder<W: Write> {
    Gzip(flate2::write::GzEncoder<W>),
    Xz(xz2::write::XzEncoder<W>),
    Bzip2(bzip2::write::BzEncoder<W>),
    Zstd(zstd::stream::write::Encoder<'static, W>),
    Lz4(lz4_flex::frame::FrameEncoder<W>),
}

impl<W: Write> Encoder<W> {
    pub fn finish(self) -> io::Result<W> {
        match self {
            Self::Gzip(encoder) => encoder.finish(),
            Self::Xz(encoder) => encoder.finish(),
            Self::Bzip2(encoder) => encoder.finish(),
            Self::Zstd(encoder) => encoder.finish(),
            Self::Lz4(encoder) => encoder.finish().map_err(io::Error::other),
        }
    }
}

impl<W: Write> Write for Encoder<W> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        match self {
            Self::Gzip(encoder) => encoder.write(bytes),
            Self::Xz(encoder) => encoder.write(bytes),
            Self::Bzip2(encoder) => encoder.write(bytes),
            Self::Zstd(encoder) => encoder.write(bytes),
            Self::Lz4(encoder) => encoder.write(bytes),
        }
    }
    // A codec flush ends a block (and the xz multithreaded encoder refuses
    // sync flushes); only `finish` terminates the stream.
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
#[path = "archive_codec/tests/codec.rs"]
mod tests;
