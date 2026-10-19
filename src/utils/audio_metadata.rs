//! Audio tags and bounded cover pixels. Windows calls this only in the LPAC worker.
use lofty::{config::ParseOptions, file::TaggedFileExt, picture::PictureType, probe::Probe};
use serde::{Deserialize, Serialize};
use std::{fs::File, io};

pub const MAX_REPLY: usize = 8 * 1024 * 1024;
const MAX_TAGS: usize = 256;
const MAX_VALUE: usize = 4096;
const MAX_TEXT_BYTES: usize = 256 * 1024;
const COVER_SIDE: u32 = 512;

#[derive(Default, Serialize, Deserialize)]
pub struct AudioMetadata {
    pub tags: Vec<(String, String)>,
    pub cover: Option<Cover>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Cover {
    pub width: u32,
    pub height: u32,
    /// BGRA pixels, already decoded by the worker.
    pub pixels: Vec<u8>,
}

impl AudioMetadata {
    pub fn validate(&self) -> io::Result<()> {
        let tags_ok = self.tags.len() <= MAX_TAGS
            && self
                .tags
                .iter()
                .map(|(key, value)| key.len() + value.len())
                .sum::<usize>()
                <= MAX_TEXT_BYTES
            && self.tags.iter().all(|(key, value)| {
                key.chars().count() <= MAX_VALUE && value.chars().count() <= MAX_VALUE
            });
        let cover_ok = self.cover.as_ref().is_none_or(|cover| {
            cover.width > 0
                && cover.height > 0
                && cover.width <= COVER_SIDE
                && cover.height <= COVER_SIDE
                && cover.pixels.len() == cover.width as usize * cover.height as usize * 4
        });
        if tags_ok && cover_ok {
            Ok(())
        } else {
            Err(io::Error::other("Invalid audio metadata reply"))
        }
    }
}

pub fn read(file: File) -> io::Result<AudioMetadata> {
    let tagged = Probe::new(file)
        .guess_file_type()?
        .options(ParseOptions::new().read_properties(false))
        .read()
        .map_err(io::Error::other)?;
    let mut metadata = AudioMetadata::default();
    // Prefer the primary tag; retain additional values from other tag formats.
    let tags = tagged
        .primary_tag()
        .into_iter()
        .chain(tagged.tags().iter().filter(|tag| {
            Some(tag.tag_type()) != tagged.primary_tag().map(|primary| primary.tag_type())
        }));
    let mut text_bytes = 0;
    for tag in tags {
        for item in tag.items() {
            let Some(value) = item.value().text().or_else(|| item.value().locator()) else {
                continue;
            };
            let key = format!("{:?}", item.key());
            let pair = (bounded(&key), bounded(value));
            let bytes = pair.0.len() + pair.1.len();
            if !pair.1.trim().is_empty()
                && !metadata.tags.contains(&pair)
                && metadata.tags.len() < MAX_TAGS
                && text_bytes + bytes <= MAX_TEXT_BYTES
            {
                text_bytes += bytes;
                metadata.tags.push(pair);
            }
        }
    }
    const ORDER: &[&str] = &[
        "TrackTitle",
        "TrackArtist",
        "AlbumTitle",
        "AlbumArtist",
        "RecordingDate",
        "Year",
        "Genre",
        "TrackNumber",
        "TrackTotal",
        "DiscNumber",
        "DiscTotal",
    ];
    metadata.tags.sort_by_key(|(key, _)| {
        ORDER
            .iter()
            .position(|item| *item == key)
            .unwrap_or(ORDER.len())
    });
    metadata.cover = extract_cover(&tagged, COVER_SIDE, &mut None);
    metadata.validate()?;
    Ok(metadata)
}

pub fn read_cover(file: File, thumbnail: bool) -> io::Result<Option<Cover>> {
    CoverReader::default().read(file, thumbnail)
}

/// A session keeps only its last decoded cover (at most 1 MiB), keyed by encoded content.
#[derive(Default)]
pub struct CoverReader {
    last: Option<(blake3::Hash, u32, Cover)>,
}

impl CoverReader {
    pub fn read(&mut self, file: File, thumbnail: bool) -> io::Result<Option<Cover>> {
        let tagged = Probe::new(file)
            .guess_file_type()?
            .options(ParseOptions::new().read_properties(false))
            .read()
            .map_err(io::Error::other)?;
        let cover = extract_cover(
            &tagged,
            if thumbnail { 128 } else { COVER_SIDE },
            &mut self.last,
        );
        if cover.is_none() && tagged.tags().iter().any(|tag| !tag.pictures().is_empty()) {
            return Err(io::Error::other("Embedded audio cover could not be decoded within its limits"));
        }
        Ok(cover)
    }
}

fn extract_cover(
    tagged: &lofty::file::TaggedFile,
    side: u32,
    cached: &mut Option<(blake3::Hash, u32, Cover)>,
) -> Option<Cover> {
    let pictures: Vec<_> = tagged
        .tags()
        .iter()
        .flat_map(|tag| tag.pictures())
        .collect();
    // Front cover first, then any decodable embedded picture. Never load external URLs.
    for picture in pictures
        .iter()
        .filter(|p| p.pic_type() == PictureType::CoverFront)
        .chain(
            pictures
                .iter()
                .filter(|p| p.pic_type() != PictureType::CoverFront),
        )
    {
        if picture.data().len() > 16 * 1024 * 1024 {
            continue;
        }
        let digest = blake3::hash(picture.data());
        if let Some((hash, cached_side, cover)) = cached {
            if *hash == digest && *cached_side == side {
                return Some(cover.clone());
            }
        }
        let policy = super::image_preview_decode::Policy {
            output_side: side,
            frame_count: 1,
            frame_bytes: 4 * 1024 * 1024,
            source_bytes: 64 * 1024 * 1024,
            ..super::image_preview_decode::POLICY
        };
        let decoded = super::image_preview_decode::decode(picture.data(), policy, &|_, _| {
            Err(super::image_preview_decode::PreviewError::Decode(
                "Unsupported cover format".into(),
            ))
        });
        if let Ok(decoded) = decoded {
            if let Some(frame) = decoded.frames.into_iter().next() {
                let pixels = frame.into_buffer();
                let cover = Cover {
                    width: pixels.width(),
                    height: pixels.height(),
                    pixels: pixels.into_raw(),
                };
                *cached = Some((digest, side, cover.clone()));
                return Some(cover);
            }
        }
    }
    None
}

fn bounded(value: &str) -> String {
    value
        .chars()
        .filter(|c| !c.is_control() || *c == '\n' || *c == '\t')
        .take(MAX_VALUE)
        .collect()
}

#[cfg(test)]
#[path = "audio_metadata/tests.rs"]
mod tests;
