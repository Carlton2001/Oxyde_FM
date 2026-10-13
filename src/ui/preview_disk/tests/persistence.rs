use super::*;

fn fixture() -> (tempfile::TempDir, PathBuf, Key) {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source.png");
    std::fs::write(&source, b"source signature").unwrap();
    let key = Key::in_root(&source, "image", &root.path().join("cache")).unwrap();
    (root, source, key)
}
fn pixels(animated: bool) -> Pixels {
    Pixels {
        bgra: RgbaImage::from_pixel(2, 3, image::Rgba([10, 20, 200, 77])),
        animated,
        limited: false,
    }
}

#[test]
fn independent_readers_reuse_png_preserving_alpha_and_bgra() {
    let (_root, source, key) = fixture();
    key.write(&pixels(false));
    let reopened = Key::in_root(&source, "image", key.file.parent().unwrap()).unwrap();
    let cached = reopened.read().unwrap();
    assert_eq!(cached.bgra, pixels(false).bgra);
    assert!(!cached.animated);
    let png = image::open(&key.file).unwrap().to_rgba8();
    assert_eq!(png.get_pixel(0, 0).0, [200, 20, 10, 77]);
}

#[test]
fn animation_metadata_survives_for_static_grid_without_replacing_animation() {
    let (_root, _, key) = fixture();
    key.write(&pixels(true));
    assert!(key.read().unwrap().animated);
    assert!(!key.read().unwrap().limited);
}

#[test]
fn changed_source_invalidates_existing_key_and_has_a_different_cache_name() {
    let (_root, source, key) = fixture();
    key.write(&pixels(false));
    std::fs::write(&source, b"new different length").unwrap();
    assert!(key.read().is_none());
    let next = Key::in_root(&source, "image", key.file.parent().unwrap()).unwrap();
    assert_ne!(key.file, next.file);
    assert!(next.read().is_none());
    key.write(&pixels(false));
    assert!(key.read().is_none());
}

#[test]
fn malformed_and_oversized_cache_files_are_misses() {
    let (_root, _, key) = fixture();
    std::fs::create_dir_all(key.file.parent().unwrap()).unwrap();
    std::fs::write(&key.file, b"not a PNG").unwrap();
    assert!(key.read().is_none());
    std::fs::write(&key.file, vec![0; MAX_BYTES + 1]).unwrap();
    assert!(key.read().is_none());
}

#[test]
fn persistence_failure_and_oversized_pixels_leave_no_cache_entry() {
    let (root, _, mut key) = fixture();
    let blocked = root.path().join("blocked");
    std::fs::write(&blocked, b"not a directory").unwrap();
    key.file = blocked.join("entry_large.png");
    key.write(&pixels(false));
    assert!(key.read().is_none());
    let oversized = Pixels {
        bgra: RgbaImage::new(SIDE + 1, 1),
        animated: false,
        limited: false,
    };
    key.write(&oversized);
    assert!(key.read().is_none());
}

#[test]
fn image_and_pdf_renditions_do_not_alias_and_names_use_existing_quota() {
    let (_root, source, key) = fixture();
    let pdf = Key::in_root(&source, "pdf", key.file.parent().unwrap()).unwrap();
    assert_ne!(key.file, pdf.file);
    let name = key.file.file_name().unwrap().to_str().unwrap();
    assert_eq!(name.len(), 64 + "_large.png".len());
    assert!(name[..64].bytes().all(|byte| byte.is_ascii_hexdigit()));
}

#[test]
fn grid_and_tooltip_requests_share_a_generation_lock() {
    let (_root, source, key) = fixture();
    let other = Key::in_root(&source, "image", key.file.parent().unwrap()).unwrap();
    let first = generation_lock(Some(&key)).unwrap();
    let second = generation_lock(Some(&other)).unwrap();
    assert!(Arc::ptr_eq(&first, &second));
    let guard = first.lock().unwrap();
    assert!(second.try_lock().is_err());
    drop(guard);
    assert!(second.try_lock().is_ok());
}
