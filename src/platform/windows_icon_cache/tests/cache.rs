use super::*;
use std::fs;

fn icon_image(width: u32, height: u32) -> RgbaImage {
    RgbaImage::from_pixel(width, height, image::Rgba([201, 43, 17, 255]))
}

fn png(width: u32, height: u32) -> Vec<u8> {
    let image = icon_image(width, height);
    let mut bytes = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(image)
        .write_to(&mut bytes, image::ImageFormat::Png)
        .unwrap();
    bytes.into_inner()
}

#[test]
fn cached_png_rejects_dimensions_over_the_icon_policy() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("icon.png");
    for (width, height, accepted) in [
        (256, 1, true),
        (1, 256, true),
        (257, 1, false),
        (1, 257, false),
    ] {
        fs::write(&path, png(width, height)).unwrap();
        assert_eq!(read_cached_png(&path).is_ok(), accepted, "width {width}");
        assert_eq!(
            decode_icon_png(&fs::read(&path).unwrap()).is_ok(),
            accepted,
            "width {width}"
        );
    }
}

#[test]
fn cached_png_rejects_encoded_bytes_over_one_mib() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("icon.png");
    fs::write(&path, png(2, 1)).unwrap();
    fs::OpenOptions::new()
        .write(true)
        .open(&path)
        .unwrap()
        .set_len(MAX_PNG_BYTES as u64)
        .unwrap();
    assert!(read_cached_png(&path).is_ok());
    fs::OpenOptions::new()
        .write(true)
        .open(&path)
        .unwrap()
        .set_len(MAX_PNG_BYTES as u64 + 1)
        .unwrap();
    assert!(
        read_cached_png(&path).is_err(),
        "Oversized cached PNG was accepted"
    );
}

#[test]
fn cache_does_not_dispatch_a_non_png_decoder() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("icon.png");
    image::DynamicImage::ImageRgba8(RgbaImage::new(2, 1))
        .save_with_format(&path, image::ImageFormat::Bmp)
        .unwrap();
    assert!(
        read_cached_png(&path).is_err(),
        "Non-PNG cached image was accepted"
    );
    assert!(decode_icon_png(&fs::read(&path).unwrap()).is_err());
}

#[test]
fn normal_cached_png_preserves_dimensions_and_pixels() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("icon.png");
    let bytes = png(32, 96);
    fs::write(&path, &bytes).unwrap();
    assert_eq!(
        read_cached_png(&path).unwrap(),
        decode_icon_png(&bytes).unwrap()
    );
    let image = decode_icon_png(&bytes).unwrap();
    assert_eq!((image.width(), image.height()), (32, 96));
    assert!(image.pixels().all(|pixel| pixel.0 == [201, 43, 17, 255]));
}

#[test]
fn reader_stops_after_the_limit_plus_one() {
    let mut input = std::io::repeat(0).take(MAX_PNG_BYTES as u64 + 9);
    let error = read_png(&mut input).unwrap_err();
    assert!(error.contains("encoded size limit"));
    assert_eq!(input.limit(), 8);
}

#[test]
fn reader_accepts_exact_budget_and_rejects_one_extra_byte() {
    let mut bytes = png(2, 1);
    bytes.resize(MAX_PNG_BYTES, 0);
    assert!(read_png(Cursor::new(&bytes)).is_ok());
    bytes.push(0);
    let mut input = Cursor::new(&bytes);
    assert!(read_png(&mut input).is_err());
    assert_eq!(input.position(), MAX_PNG_BYTES as u64 + 1);
    assert!(decode_icon_png(&bytes).is_err());
}

#[test]
fn invalid_disk_cache_is_regenerated_without_returning_stale_bytes() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("entry.png");
    let mut oversized = png(2, 1);
    oversized.resize(MAX_PNG_BYTES + 1, 0);
    let replacement = icon_image(32, 32);
    for invalid in [b"not a PNG".to_vec(), png(257, 1), oversized] {
        fs::write(&path, &invalid).unwrap();
        let memory = Mutex::new(IconCache::default());
        let calls = std::cell::Cell::new(0);
        let output = cached_or_generate(&memory, Some(directory.path()), "entry", || {
            calls.set(calls.get() + 1);
            Ok(replacement.clone())
        })
        .unwrap();
        assert_eq!(calls.get(), 1);
        assert_eq!(output, replacement);
        assert_eq!(read_cached_png(&path).unwrap(), replacement);
    }
}

#[test]
fn valid_disk_cache_avoids_generation() {
    let directory = tempfile::tempdir().unwrap();
    let bytes = png(32, 32);
    fs::write(directory.path().join("entry.png"), &bytes).unwrap();
    let memory = Mutex::new(IconCache::default());
    let output = cached_or_generate(&memory, Some(directory.path()), "entry", || {
        panic!("Valid disk hit must not call the Shell generator")
    })
    .unwrap();
    assert_eq!(output, decode_icon_png(&bytes).unwrap());
    // Retain decoded pixels: a later hit needs neither the PNG nor the Shell.
    fs::remove_file(directory.path().join("entry.png")).unwrap();
    assert_eq!(
        cached_or_generate(&memory, Some(directory.path()), "entry", || {
            panic!("Disk hit must populate the pixel cache")
        })
        .unwrap(),
        output
    );
}

#[test]
fn generated_pixels_preserve_alpha_in_memory_and_persisted_png() {
    let directory = tempfile::tempdir().unwrap();
    let memory = Mutex::new(IconCache::default());
    let mut expected = icon_image(32, 96);
    expected.put_pixel(0, 0, image::Rgba([91, 23, 77, 0]));
    expected.put_pixel(1, 0, image::Rgba([23, 91, 77, 128]));
    let result = cached_or_generate(&memory, Some(directory.path()), "entry", || {
        Ok(expected.clone())
    })
    .unwrap();
    assert_eq!(result, expected);
    assert_eq!(memory.lock().unwrap().get("entry").unwrap(), expected);
    assert_eq!(
        read_cached_png(&directory.path().join("entry.png")).unwrap(),
        expected
    );
    // Even a bad persisted entry cannot affect an already validated memory hit.
    fs::write(directory.path().join("entry.png"), b"bad PNG").unwrap();
    assert_eq!(
        cached_or_generate(&memory, Some(directory.path()), "entry", || {
            panic!("Memory hit must not generate again")
        })
        .unwrap(),
        expected
    );
}

#[test]
fn generation_failure_does_not_return_or_publish_a_bad_cache_entry() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("entry.png");
    fs::write(&path, b"bad cache").unwrap();
    let memory = Mutex::new(IconCache::default());
    let error = cached_or_generate(&memory, Some(directory.path()), "entry", || {
        Err("injected generation failure".into())
    })
    .unwrap_err();
    assert_eq!(error, "injected generation failure");
    assert_eq!(fs::read(&path).unwrap(), b"bad cache");
    assert!(memory.lock().unwrap().entries.is_empty());
}

#[test]
fn cache_write_failure_does_not_block_a_valid_icon() {
    let directory = tempfile::tempdir().unwrap();
    let blocked_directory = directory.path().join("regular_file");
    fs::write(&blocked_directory, b"independent file").unwrap();
    let memory = Mutex::new(IconCache::default());
    let bytes = icon_image(32, 32);
    let output = cached_or_generate(&memory, Some(&blocked_directory), "entry", || {
        Ok(bytes.clone())
    })
    .unwrap();
    assert_eq!(output, bytes);
    assert_eq!(fs::read(&blocked_directory).unwrap(), b"independent file");
    assert_eq!(
        cached_or_generate(&memory, Some(&blocked_directory), "entry", || panic!(
            "In-memory hit must remain available"
        ))
        .unwrap(),
        bytes
    );
}

#[test]
fn memory_evicts_oldest_entries_on_count_or_byte_pressure() {
    let bytes = icon_image(32, 32);
    for (entry_limit, byte_limit) in [(2, usize::MAX), (10, bytes.as_raw().len() * 2)] {
        let memory = Mutex::new(IconCache::new(entry_limit, byte_limit));
        let calls = std::cell::Cell::new(0);
        for key in ["one", "two", "three"] {
            assert_eq!(
                cached_or_generate(&memory, None, key, || {
                    calls.set(calls.get() + 1);
                    Ok(bytes.clone())
                })
                .unwrap(),
                bytes
            );
        }
        {
            let cache = memory.lock().unwrap();
            assert_eq!(cache.entries.len(), 2);
            assert_eq!(cache.bytes, bytes.as_raw().len() * 2);
            assert!(!cache.entries.contains_key("one"));
        }
        assert_eq!(
            cached_or_generate(&memory, None, "two", || panic!(
                "Remaining icon must be cached"
            ))
            .unwrap(),
            bytes
        );
        assert_eq!(
            cached_or_generate(&memory, None, "one", || {
                calls.set(calls.get() + 1);
                Ok(bytes.clone())
            })
            .unwrap(),
            bytes
        );
        assert_eq!(calls.get(), 4);
    }
}

#[test]
fn replacement_accounts_bytes_once_and_purge_resets_the_cache() {
    let old = icon_image(1, 1);
    let replacement = icon_image(32, 32);
    let mut cache = IconCache::new(2, replacement.as_raw().len() * 2);
    cache.insert("same".into(), old);
    cache.insert("same".into(), replacement.clone());
    assert_eq!(cache.bytes, replacement.as_raw().len());
    assert_eq!(cache.entries.len(), 1);
    assert_eq!(cache.insertion_order.len(), 1);
    cache.insert("other".into(), replacement.clone());
    assert_eq!(cache.bytes, replacement.as_raw().len() * 2);
    cache.clear();
    assert_eq!(cache.bytes, 0);
    assert!(cache.entries.is_empty() && cache.insertion_order.is_empty());
    cache.insert("next".into(), replacement.clone());
    assert_eq!(cache.get("next").unwrap(), replacement);
}

#[test]
fn icon_larger_than_the_memory_budget_remains_displayable_uncached() {
    let memory = Mutex::new(IconCache::new(10, 1));
    let bytes = icon_image(32, 32);
    let calls = std::cell::Cell::new(0);
    for _ in 0..2 {
        assert_eq!(
            cached_or_generate(&memory, None, "entry", || {
                calls.set(calls.get() + 1);
                Ok(bytes.clone())
            })
            .unwrap(),
            bytes
        );
    }
    assert_eq!(calls.get(), 2);
    assert!(memory.lock().unwrap().entries.is_empty());
}

#[test]
fn inadmissible_generator_output_is_not_published() {
    let directory = tempfile::tempdir().unwrap();
    let memory = Mutex::new(IconCache::default());
    for (width, height) in [(257, 1), (1, 257), (0, 1), (1, 0)] {
        assert!(
            cached_or_generate(&memory, Some(directory.path()), "entry", || {
                Ok(icon_image(width, height))
            })
            .is_err()
        );
    }
    assert!(!directory.path().join("entry.png").exists());
    assert!(memory.lock().unwrap().entries.is_empty());
}

#[test]
fn junction_cache_is_ignored_without_reading_or_writing_its_target() {
    let root = tempfile::tempdir().unwrap();
    let outside = root.path().join("outside");
    fs::create_dir(&outside).unwrap();
    let previous = png(1, 1);
    fs::write(outside.join("entry.png"), &previous).unwrap();
    let directory = root.path().join("cache");
    super::super::windows_reparse_tests::junction(&directory, &outside);
    let memory = Mutex::new(IconCache::default());
    let expected = icon_image(32, 32);
    let calls = std::cell::Cell::new(0);
    let output = cached_or_generate(&memory, Some(&directory), "entry", || {
        calls.set(calls.get() + 1);
        Ok(expected.clone())
    })
    .unwrap();
    fs::remove_dir(directory).unwrap();
    assert_eq!(calls.get(), 1);
    assert_eq!(output, expected);
    assert_eq!(fs::read(outside.join("entry.png")).unwrap(), previous);
    assert_eq!(fs::read_dir(outside).unwrap().count(), 1);
    assert_eq!(memory.lock().unwrap().get("entry").unwrap(), expected);
}

#[test]
fn occupied_disk_quota_lease_keeps_the_generated_icon_usable_in_memory() {
    let root = tempfile::tempdir().unwrap();
    let lease = root.path().join(".preview-cache-lock");
    fs::write(&lease, b"lease witness").unwrap();
    let reader = fs::File::open(&lease).unwrap();
    let key = format!("{:064x}_small", 1);
    let memory = Mutex::new(IconCache::default());
    let expected = icon_image(32, 32);
    let output =
        cached_or_generate(&memory, Some(root.path()), &key, || Ok(expected.clone())).unwrap();
    assert_eq!(output, expected);
    assert_eq!(memory.lock().unwrap().get(&key).unwrap(), expected);
    assert!(!root.path().join(format!("{key}.png")).exists());
    assert_eq!(fs::read(&lease).unwrap(), b"lease witness");
    assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
    drop(reader);
}
