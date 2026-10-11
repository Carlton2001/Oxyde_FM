    use super::*;

    fn generated_image() -> RgbaImage {
        RgbaImage::from_pixel(32, 16, image::Rgba([201, 43, 17, 255]))
    }

    #[test]
    fn unavailable_cache_directory_does_not_prevent_generation() {
        let root = tempfile::tempdir().unwrap();
        let blocked = root.path().join("regular_file");
        fs::write(&blocked, b"independent directory marker").unwrap();
        let calls = std::cell::Cell::new(0);
        let expected = generated_image();
        let result = cached_thumbnail_in(&blocked.join("entry.jpg"), || {
            calls.set(calls.get() + 1);
            Ok(expected.clone())
        });
        assert_eq!(calls.get(), 1, "Caching prevented thumbnail generation");
        assert_eq!(result.unwrap(), expected);
        assert_eq!(fs::read(&blocked).unwrap(), b"independent directory marker");
    }

    #[test]
    fn persistence_failure_keeps_the_generated_image() {
        let root = tempfile::tempdir().unwrap();
        let cache_dir = root.path().join("cache");
        fs::create_dir(&cache_dir).unwrap();
        let expected = generated_image();
        let result = cached_thumbnail_in(&cache_dir.join("entry.jpg"), || {
            // Change only this fixture after a valid image has been generated.
            fs::remove_dir(&cache_dir).unwrap();
            fs::write(&cache_dir, b"independent directory marker").unwrap();
            Ok(expected.clone())
        });
        assert_eq!(result.unwrap(), expected);
        assert_eq!(
            fs::read(&cache_dir).unwrap(),
            b"independent directory marker"
        );
    }

    #[test]
    fn exclusive_creation_collision_preserves_the_existing_file() {
        let root = tempfile::tempdir().unwrap();
        let temp = root.path().join("entry.tmp");
        let destination = root.path().join("entry.jpg");
        fs::write(&temp, b"independent staging marker").unwrap();
        assert!(write_thumbnail_cache(&destination, &temp, &generated_image()).is_err());
        assert_eq!(fs::read(&temp).unwrap(), b"independent staging marker");
        assert!(!destination.exists());
    }

    #[test]
    fn normal_cache_hit_avoids_generation_and_preserves_pixels() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("entry.jpg");
        let expected = generated_image();
        assert_eq!(
            cached_thumbnail_in(&path, || Ok(expected.clone())).unwrap(),
            expected
        );
        let bytes = fs::read(&path).unwrap();
        let image =
            cached_thumbnail_in(&path, || panic!("Valid cache hit generated again")).unwrap();
        assert_eq!(image.dimensions(), (32, 16));
        let pixel = image.get_pixel(0, 0).0;
        assert!(pixel[0] > 190 && pixel[1] < 55 && pixel[2] < 30);
        assert_eq!(fs::read(path).unwrap(), bytes);
    }

    #[test]
    fn generation_failure_preserves_invalid_cache_until_replacement() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("entry.jpg");
        fs::write(&path, b"invalid cache marker").unwrap();
        let error = cached_thumbnail_in(&path, || {
            Err(CommandError::Other("injected generation failure".into()))
        })
        .unwrap_err();
        assert!(error.to_string().contains("injected generation failure"));
        assert_eq!(fs::read(&path).unwrap(), b"invalid cache marker");
        assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
        let expected = generated_image();
        assert_eq!(
            cached_thumbnail_in(&path, || Ok(expected.clone())).unwrap(),
            expected
        );
        assert_eq!(
            load_cached_thumbnail(&path)
                .unwrap()
                .to_rgba8()
                .dimensions(),
            (32, 16)
        );
        assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
    }

    #[test]
    fn unavailable_cache_does_not_hide_a_generation_error() {
        let root = tempfile::tempdir().unwrap();
        let blocked = root.path().join("regular_file");
        fs::write(&blocked, b"independent directory marker").unwrap();
        let error = cached_thumbnail_in(&blocked.join("entry.jpg"), || {
            Err(CommandError::Other("injected generation failure".into()))
        })
        .unwrap_err();
        assert!(error.to_string().contains("injected generation failure"));
        assert_eq!(fs::read(&blocked).unwrap(), b"independent directory marker");
    }

    #[test]
    fn failed_publication_cleans_only_the_created_staging_file() {
        let root = tempfile::tempdir().unwrap();
        let destination = root.path().join("entry.jpg");
        fs::create_dir(&destination).unwrap();
        fs::write(
            destination.join("marker"),
            b"independent destination marker",
        )
        .unwrap();
        let unrelated = root.path().join("other.tmp");
        fs::write(&unrelated, b"independent staging marker").unwrap();
        let temp = root.path().join("entry.tmp");
        assert!(write_thumbnail_cache(&destination, &temp, &generated_image()).is_err());
        assert!(!temp.exists());
        assert_eq!(fs::read(unrelated).unwrap(), b"independent staging marker");
        assert_eq!(
            fs::read(destination.join("marker")).unwrap(),
            b"independent destination marker"
        );
    }

    #[test]
    fn failed_publication_still_returns_the_generated_thumbnail() {
        let root = tempfile::tempdir().unwrap();
        let destination = root.path().join("entry.jpg");
        fs::create_dir(&destination).unwrap();
        fs::write(
            destination.join("marker"),
            b"independent destination marker",
        )
        .unwrap();
        let expected = generated_image();
        assert_eq!(
            cached_thumbnail_in(&destination, || Ok(expected.clone())).unwrap(),
            expected
        );
        assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
        assert_eq!(
            fs::read(destination.join("marker")).unwrap(),
            b"independent destination marker"
        );
    }

    #[test]
    fn missing_source_metadata_still_reports_an_error() {
        let root = tempfile::tempdir().unwrap();
        assert!(cached_thumbnail(
            root.path()
                .join("missing.png")
                .to_string_lossy()
                .into_owned(),
            "image",
            |_| { panic!("Missing source must not generate a thumbnail or access the user cache") }
        )
        .is_err());
        assert_eq!(fs::read_dir(root.path()).unwrap().count(), 0);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn cache_junction_does_not_publish_into_its_target() {
        let root = tempfile::tempdir().unwrap();
        let outside = root.path().join("outside");
        fs::create_dir(&outside).unwrap();
        fs::write(outside.join("entry.jpg"), b"independent destination marker").unwrap();
        let cache = root.path().join("cache");
        crate::platform::windows_reparse_tests::junction(&cache, &outside);
        let result = write_thumbnail_cache(
            &cache.join("entry.jpg"),
            &cache.join("entry.tmp"),
            &generated_image(),
        );
        let bytes = fs::read(outside.join("entry.jpg")).unwrap();
        fs::remove_dir(&cache).unwrap();
        assert!(result.is_err(), "Cache publication followed a junction");
        assert_eq!(bytes, b"independent destination marker");
        assert_eq!(fs::read_dir(&outside).unwrap().count(), 1);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn cache_junction_does_not_supply_a_thumbnail() {
        let root = tempfile::tempdir().unwrap();
        let outside = root.path().join("outside");
        fs::create_dir(&outside).unwrap();
        write_thumbnail_cache(
            &outside.join("entry.jpg"),
            &outside.join("entry.tmp"),
            &generated_image(),
        )
        .unwrap();
        let cache = root.path().join("cache");
        crate::platform::windows_reparse_tests::junction(&cache, &outside);
        let result = load_cached_thumbnail(&cache.join("entry.jpg"));
        fs::remove_dir(&cache).unwrap();
        assert!(result.is_err(), "Cache read followed a junction");
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn occupied_disk_quota_lease_keeps_the_generated_thumbnail_usable() {
        let root = tempfile::tempdir().unwrap();
        let lease = root.path().join(".preview-cache-lock");
        fs::write(&lease, b"lease witness").unwrap();
        let reader = fs::File::open(&lease).unwrap();
        let destination = root.path().join(format!("{:064x}.jpg", 1));
        let expected = generated_image();
        assert_eq!(
            cached_thumbnail_in(&destination, || Ok(expected.clone())).unwrap(),
            expected
        );
        assert!(!destination.exists());
        assert_eq!(fs::read(&lease).unwrap(), b"lease witness");
        assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
        drop(reader);
    }
