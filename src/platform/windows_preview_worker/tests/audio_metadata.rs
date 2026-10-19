use super::run_audio_metadata_in;
use crate::utils::audio_metadata::AudioMetadata;
use lofty::{
    config::WriteOptions,
    picture::{MimeType, Picture, PictureType},
    prelude::TagExt,
    tag::{ItemKey, Tag, TagType},
};
use std::io::Write;

#[test]
fn isolated_audio_metadata_worker_reads_inherited_source() {
    let executable = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("oxyde-image-worker.exe");
    assert!(
        executable.is_file(),
        "Build oxyde-image-worker before this test"
    );
    let mut file = tempfile::NamedTempFile::new().unwrap();
    file.write_all(b"RIFF\x26\0\0\0WAVEfmt \x10\0\0\0\x01\0\x01\0\x44\xac\0\0\x88\x58\x01\0\x02\0\x10\0data\x02\0\0\0\0\0").unwrap();
    let mut tag = Tag::new(TagType::Id3v2);
    tag.insert_text(ItemKey::TrackTitle, "Pochette et tags".into());
    let mut png = std::io::Cursor::new(Vec::new());
    image::RgbaImage::from_pixel(1024, 576, image::Rgba([200, 30, 10, 255]))
        .write_to(&mut png, image::ImageFormat::Png)
        .unwrap();
    tag.push_picture(
        Picture::unchecked(png.into_inner())
            .pic_type(PictureType::CoverFront)
            .mime_type(MimeType::Png)
            .build(),
    );
    tag.save_to(&mut file.reopen().unwrap(), WriteOptions::default())
        .unwrap();
    // Close fixture writers before retaining the source with read-only sharing.
    let source = file.into_temp_path();
    let bytes = run_audio_metadata_in(&executable, &source).unwrap();
    let metadata: AudioMetadata = serde_json::from_slice(&bytes).unwrap();
    metadata.validate().unwrap();
    assert!(metadata
        .tags
        .contains(&("TrackTitle".into(), "Pochette et tags".into())));
    let cover = metadata.cover.unwrap();
    assert_eq!((cover.width, cover.height), (512, 288));
    assert_eq!(&cover.pixels[..4], &[10, 30, 200, 255]);
}
