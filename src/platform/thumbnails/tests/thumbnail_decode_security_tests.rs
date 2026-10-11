    use super::*;

    fn write_image(path: &Path, width: u32, height: u32, format: image::ImageFormat) {
        image::DynamicImage::ImageRgba8(RgbaImage::new(width, height))
            .save_with_format(path, format)
            .unwrap();
    }

    #[test]
    fn cached_thumbnail_dimension_limit_accepts_boundary_and_rejects_next_pixel() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("cache.jpg");
        for (width, height, accepted) in [
            (1024, 1, true),
            (1, 1024, true),
            (1025, 1, false),
            (1, 1025, false),
        ] {
            write_image(&path, width, height, image::ImageFormat::Jpeg);
            assert_eq!(
                load_cached_thumbnail(&path).is_ok(),
                accepted,
                "width {width}"
            );
        }
    }

    #[test]
    fn source_thumbnail_dimension_limit_accepts_boundary_and_rejects_next_pixel() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("source.png");
        for (width, height, accepted) in [
            (32768, 1, true),
            (1, 32768, true),
            (32769, 1, false),
            (1, 32769, false),
        ] {
            write_image(&path, width, height, image::ImageFormat::Png);
            assert_eq!(
                load_source_thumbnail(&path).is_ok(),
                accepted,
                "width {width}"
            );
        }
    }

    #[test]
    fn cached_thumbnail_encoded_limit_accepts_boundary_and_rejects_next_byte() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("cache.jpg");
        write_image(&path, 2, 1, image::ImageFormat::Jpeg);
        fs::OpenOptions::new()
            .write(true)
            .open(&path)
            .unwrap()
            .set_len(CACHE_DECODE_LIMITS.encoded_bytes)
            .unwrap();
        assert!(load_cached_thumbnail(&path).is_ok());
        fs::OpenOptions::new()
            .write(true)
            .open(&path)
            .unwrap()
            .set_len(CACHE_DECODE_LIMITS.encoded_bytes + 1)
            .unwrap();
        let error = load_cached_thumbnail(&path).unwrap_err();
        assert!(error.to_string().contains("encoded size limit"));
    }

    #[test]
    fn source_thumbnail_rejects_oversized_open_file_before_format_detection() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("source.png");
        fs::File::create(&path)
            .unwrap()
            .set_len(SOURCE_DECODE_LIMITS.encoded_bytes + 1)
            .unwrap();
        // A file of zeros would fail format detection: this error comes from admission.
        let error = load_source_thumbnail(&path).unwrap_err();
        assert!(error.to_string().contains("encoded size limit"));
    }

    #[test]
    fn thumbnail_reader_stops_after_encoded_limit_plus_one() {
        use std::io::Read;
        let mut input = std::io::repeat(0).take(1000);
        let policy = ThumbnailDecodeLimits {
            encoded_bytes: 16,
            ..CACHE_DECODE_LIMITS
        };
        let error = decode_thumbnail_image(&mut input, policy, None).unwrap_err();
        assert!(error.to_string().contains("encoded size limit"));
        assert_eq!(input.limit(), 1000 - 17);
    }

    #[test]
    fn valid_thumbnail_reader_accepts_exact_budget_and_rejects_appended_byte() {
        let mut data = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(RgbaImage::new(2, 1))
            .write_to(&mut data, image::ImageFormat::Png)
            .unwrap();
        let mut data = data.into_inner();
        let policy = ThumbnailDecodeLimits {
            encoded_bytes: data.len() as u64,
            ..CACHE_DECODE_LIMITS
        };
        assert!(decode_thumbnail_image(std::io::Cursor::new(&data), policy, None).is_ok());
        data.push(0);
        assert!(decode_thumbnail_image(std::io::Cursor::new(&data), policy, None).is_err());
    }

    #[test]
    fn thumbnail_decode_rejects_output_over_allocation_budget() {
        let mut data = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(RgbaImage::new(128, 128))
            .write_to(&mut data, image::ImageFormat::Png)
            .unwrap();
        let data = data.into_inner();
        assert!(
            decode_thumbnail_image(std::io::Cursor::new(&data), CACHE_DECODE_LIMITS, None).is_ok()
        );
        let policy = ThumbnailDecodeLimits {
            alloc_bytes: 32 * 1024,
            ..CACHE_DECODE_LIMITS
        };
        assert!(decode_thumbnail_image(std::io::Cursor::new(&data), policy, None).is_err());
    }

    #[test]
    fn cache_and_source_do_not_decode_a_different_format_than_the_extension() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("cache.jpg");
        write_image(&path, 2, 1, image::ImageFormat::Png);
        assert!(load_cached_thumbnail(&path).is_err());
        assert!(load_source_thumbnail(&path).is_err());
    }

    #[test]
    fn normal_source_and_cache_images_preserve_dimensions_and_pixels() {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("source.png");
        let cache = directory.path().join("cache.jpg");
        let image = RgbaImage::from_pixel(512, 128, image::Rgba([201, 43, 17, 255]));
        image::DynamicImage::ImageRgba8(image.clone())
            .save(&source)
            .unwrap();
        let decoded = load_source_thumbnail(&source).unwrap().to_rgba8();
        assert_eq!(decoded, image);
        let thumbnail = image::DynamicImage::ImageRgba8(decoded).thumbnail(THUMB_SIZE, THUMB_SIZE);
        assert_eq!((thumbnail.width(), thumbnail.height()), (256, 64));
        thumbnail.save(&cache).unwrap();
        let decoded = load_cached_thumbnail(&cache).unwrap();
        assert_eq!((decoded.width(), decoded.height()), (256, 64));
        let pixel = decoded.to_rgba8().get_pixel(0, 0).0;
        assert!(pixel[0] > 190 && pixel[1] < 55 && pixel[2] < 30);
    }
