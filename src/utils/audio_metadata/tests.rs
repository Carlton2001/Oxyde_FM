use super::*;
use lofty::{
    config::WriteOptions,
    picture::{MimeType, Picture},
    prelude::TagExt,
    tag::{ItemKey, Tag, TagType},
};
use std::io::{Cursor, Write};

fn wav() -> tempfile::NamedTempFile {
    let mut file = tempfile::NamedTempFile::new().unwrap();
    // One silent PCM sample, enough for container probing without audio playback.
    file.write_all(b"RIFF\x26\0\0\0WAVEfmt \x10\0\0\0\x01\0\x01\0\x44\xac\0\0\x88\x58\x01\0\x02\0\x10\0data\x02\0\0\0\0\0").unwrap();
    file
}

#[test]
fn reads_unicode_tags_and_bgra_front_cover() {
    let file = wav();
    let mut tag = Tag::new(TagType::Id3v2);
    tag.insert_text(ItemKey::TrackTitle, "Été à Rome".into());
    tag.insert_text(ItemKey::TrackArtist, "Artiste".into());
    tag.insert_text(ItemKey::TrackNumber, "3".into());
    let pixels = image::RgbaImage::from_pixel(2, 1, image::Rgba([255, 20, 5, 255]));
    let mut png = Cursor::new(Vec::new());
    pixels.write_to(&mut png, image::ImageFormat::Png).unwrap();
    tag.push_picture(
        Picture::unchecked(png.into_inner())
            .pic_type(PictureType::CoverFront)
            .mime_type(MimeType::Png)
            .build(),
    );
    tag.save_to(&mut file.reopen().unwrap(), WriteOptions::default())
        .unwrap();
    let metadata = read(file.reopen().unwrap()).unwrap();
    assert!(metadata
        .tags
        .contains(&("TrackTitle".into(), "Été à Rome".into())));
    assert!(metadata.tags.contains(&("TrackNumber".into(), "3".into())));
    let cover = metadata.cover.unwrap();
    assert_eq!((cover.width, cover.height), (2, 1));
    assert_eq!(&cover.pixels[..4], &[5, 20, 255, 255]);
}

#[test]
fn missing_tags_and_invalid_picture_keep_metadata_readable() {
    let file = wav();
    assert!(read(file.reopen().unwrap()).unwrap().tags.is_empty());
    let mut tag = Tag::new(TagType::Id3v2);
    tag.insert_text(ItemKey::AlbumTitle, "Album".into());
    tag.push_picture(
        Picture::unchecked(vec![1, 2, 3])
            .pic_type(PictureType::CoverFront)
            .mime_type(MimeType::Png)
            .build(),
    );
    tag.save_to(&mut file.reopen().unwrap(), WriteOptions::default())
        .unwrap();
    let metadata = read(file.reopen().unwrap()).unwrap();
    assert!(metadata.cover.is_none());
    assert!(metadata
        .tags
        .contains(&("AlbumTitle".into(), "Album".into())));
}

#[test]
fn rejects_invalid_cover_pixels_and_excessive_tags() {
    let mut metadata = AudioMetadata {
        tags: vec![],
        cover: Some(Cover {
            width: 513,
            height: 1,
            pixels: vec![0; 2052],
        }),
    };
    assert!(metadata.validate().is_err());
    metadata.cover = Some(Cover {
        width: 1,
        height: 1,
        pixels: vec![0; 3],
    });
    assert!(metadata.validate().is_err());
    metadata.cover = None;
    metadata.tags = vec![("key".into(), "value".into()); MAX_TAGS + 1];
    assert!(metadata.validate().is_err());
}

#[test]
fn rejects_non_audio_and_caps_tag_text() {
    let mut file = tempfile::NamedTempFile::new().unwrap();
    file.write_all(b"not an audio file").unwrap();
    assert!(read(file.reopen().unwrap()).is_err());
    assert_eq!(
        bounded(&"é".repeat(MAX_VALUE + 10)).chars().count(),
        MAX_VALUE
    );
    assert_eq!(bounded("a\0b"), "ab");
}
