    use super::*;

    #[cfg(target_os = "windows")]
    #[test]
    fn embedded_office_thumbnail_keeps_bgra_channels_for_native_ui() {
        let mut bytes = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(RgbaImage::from_pixel(2,1,image::Rgba([201,43,17,255])))
            .write_to(&mut bytes,image::ImageFormat::Png).unwrap();
        bytes.set_position(0);
        assert_eq!(decode_office_thumbnail(bytes).unwrap().get_pixel(0,0).0,[17,43,201,255]);
    }

    #[test]
    fn oversized_compressed_payload_is_rejected_with_bounded_reads() {
        let mut input = std::io::Cursor::new(vec![0; 9 * 1024 * 1024]);
        assert!(decode_office_thumbnail(&mut input).is_err());
        assert_eq!(input.position(), 8 * 1024 * 1024 + 1);
    }

    #[test]
    fn normal_images_decode_and_excessive_dimensions_are_rejected() {
        for (width, accepted) in [(2, true), (4097, false)] {
            let image = image::DynamicImage::ImageRgba8(RgbaImage::new(width, 1));
            let mut data = std::io::Cursor::new(Vec::new());
            image.write_to(&mut data, image::ImageFormat::Png).unwrap();
            data.set_position(0);
            assert_eq!(decode_office_thumbnail(data).is_ok(), accepted);
        }
    }
