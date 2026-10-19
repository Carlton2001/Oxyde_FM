use super::*;

#[test]
fn audio_thumbnails_share_one_disk_blob_and_keep_independent_source_revisions() {
    let root = tempfile::tempdir().unwrap();
    let first = root.path().join("first.mp3");
    let second = root.path().join("second.flac");
    std::fs::write(&first, b"first source").unwrap();
    std::fs::write(&second, b"second source").unwrap();
    let cache = root.path().join("cache");
    let first_key = Key::in_root(&first, "grid-audio-cover-v2", &cache).unwrap();
    let second_key = Key::in_root(&second, "grid-audio-cover-v2", &cache).unwrap();
    let pixels = Pixels { bgra: RgbaImage::from_pixel(128, 64, image::Rgba([20, 40, 80, 120])), animated: false, limited: false };
    first_key.write(&pixels);
    second_key.write(&pixels);
    let reference = std::fs::read(&first_key.file).unwrap();
    assert_eq!(reference.len(), 40);
    assert_eq!(reference, std::fs::read(&second_key.file).unwrap());
    let images = std::fs::read_dir(&cache).unwrap().filter(|entry| {
        entry.as_ref().unwrap().path().extension().is_some_and(|extension| extension == "jpg")
    }).count();
    assert_eq!(images, 3, "Two references must share one thumbnail blob");
    assert_eq!(first_key.read().unwrap().bgra, pixels.bgra);
    assert_eq!(second_key.read().unwrap().bgra, pixels.bgra);
    std::fs::write(&first, b"changed source with new tags").unwrap();
    assert!(first_key.read().is_none());
    assert!(second_key.read().is_some());
    let digest = blake3::Hash::from(<[u8; 32]>::try_from(&reference[8..]).unwrap());
    let blob = cache.join(format!("{}.jpg", digest.to_hex()));
    std::fs::write(&blob, b"corrupt blob").unwrap();
    assert!(second_key.read().is_none());
    second_key.write(&pixels);
    assert_eq!(second_key.read().unwrap().bgra, pixels.bgra);
    std::fs::remove_file(blob).unwrap();
    assert!(second_key.read().is_none(), "Quota eviction must become a cache miss");
}
