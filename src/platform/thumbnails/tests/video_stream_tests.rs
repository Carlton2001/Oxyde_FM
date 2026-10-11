    use super::*;
    use std::cell::Cell;
    fn frame() -> VideoSample {
        VideoSample::Frame(RgbaImage::new(1, 1), Duration::ZERO)
    }
    #[test]
    fn opening_and_read_errors_are_returned_without_restart() {
        let opens = Cell::new(0);
        let error = play_video(
            &|| false,
            || -> Result<(), CommandError> {
                opens.set(opens.get() + 1);
                Err(CommandError::Other("open failure".into()))
            },
            |_| panic!("read after failed open"),
            &mut |_, _| panic!("frame after failed open"),
            || panic!("pause after failed open"),
        );
        assert!(error.unwrap_err().to_string().contains("open failure"));
        assert_eq!(opens.get(), 1);
        let error = play_video(
            &|| false,
            || {
                opens.set(opens.get() + 1);
                Ok(())
            },
            |_| Err(CommandError::Other("decode failure".into())),
            &mut |_, _| panic!("frame after failed decode"),
            || panic!("pause after failed decode"),
        );
        assert!(error.unwrap_err().to_string().contains("decode failure"));
        assert_eq!(opens.get(), 2);
    }
    #[test]
    fn an_empty_stream_ends_with_error_instead_of_reopening_forever() {
        let error = play_video(
            &|| false,
            || Ok(()),
            |_| Ok(VideoSample::End),
            &mut |_, _| panic!("empty stream produced frame"),
            || panic!("empty stream restarted"),
        );
        assert!(error.unwrap_err().to_string().contains("without a frame"));
    }
    #[test]
    fn cancellation_stops_gaps_and_suppresses_a_frame_returned_after_it() {
        for with_frame in [false, true] {
            let cancelled = Cell::new(false);
            play_video(
                &|| cancelled.get(),
                || Ok(()),
                |_| {
                    cancelled.set(true);
                    Ok(if with_frame {
                        frame()
                    } else {
                        VideoSample::Gap
                    })
                },
                &mut |_, _| panic!("published after cancellation"),
                || panic!("restarted after cancellation"),
            )
            .unwrap();
        }
        play_video(
            &|| true,
            || -> Result<(), CommandError> { panic!("opened after cancellation") },
            |_| panic!(),
            &mut |_, _| panic!(),
            || panic!(),
        )
        .unwrap();
    }
    #[test]
    fn only_natural_end_restarts_and_callback_refusal_stops_both_loops() {
        let opens = Cell::new(0);
        let frames = Cell::new(0);
        let pauses = Cell::new(0);
        play_video(
            &|| false,
            || {
                opens.set(opens.get() + 1);
                Ok(0)
            },
            |index| {
                *index += 1;
                Ok(if *index <= 2 {
                    frame()
                } else {
                    VideoSample::End
                })
            },
            &mut |_, _| {
                frames.set(frames.get() + 1);
                frames.get() < 3
            },
            || pauses.set(pauses.get() + 1),
        )
        .unwrap();
        assert_eq!(opens.get(), 2);
        assert_eq!(frames.get(), 3);
        assert_eq!(pauses.get(), 20);
    }
    #[test]
    fn cancellation_during_restart_pause_prevents_a_new_reader() {
        let cancelled = Cell::new(false);
        let reads = Cell::new(0);
        let opens = Cell::new(0);
        play_video(
            &|| cancelled.get(),
            || {
                opens.set(opens.get() + 1);
                Ok(())
            },
            |_| {
                reads.set(reads.get() + 1);
                Ok(if reads.get() == 1 {
                    frame()
                } else {
                    VideoSample::End
                })
            },
            &mut |_, _| true,
            || cancelled.set(true),
        )
        .unwrap();
        assert_eq!(opens.get(), 1);
    }
    #[test]
    fn video_pixels_are_bounded_and_signed_stride_is_respected() {
        assert!(VideoLayout::new(0, 2, 8).is_err());
        assert!(VideoLayout::new(2, 2, 4).is_err());
        assert!(VideoLayout::new(2, 2, i32::MIN).is_err());
        assert!(VideoLayout::new(u32::MAX, 2, 8).is_err());
        let mut layout = VideoLayout::new(2, 2, 12).unwrap();
        layout.out_w = 2;
        layout.out_h = 2;
        assert!(resize_video_frame(&[0; 19], layout).is_err());
        let pixels = [
            1, 2, 3, 0, 4, 5, 6, 0, 99, 99, 99, 99, 7, 8, 9, 0, 10, 11, 12, 0,
        ];
        let image = resize_video_frame(&pixels, layout).unwrap();
        assert_eq!(
            image.into_raw(),
            [1, 2, 3, 255, 4, 5, 6, 255, 7, 8, 9, 255, 10, 11, 12, 255]
        );
        layout.stride = -12;
        let image = resize_video_frame(&pixels, layout).unwrap();
        assert_eq!(
            image.into_raw(),
            [7, 8, 9, 255, 10, 11, 12, 255, 1, 2, 3, 255, 4, 5, 6, 255]
        );
    }
    #[test]
    fn video_timestamps_preserve_units_without_overflow() {
        assert_eq!(video_timestamp(10_000_005), Duration::new(1, 500));
        assert_eq!(video_timestamp(-1), Duration::ZERO);
        let large = video_timestamp(i64::MAX);
        assert_eq!(large.as_secs(), i64::MAX as u64 / 10_000_000);
    }
    #[test]
    fn native_video_layout_and_truncated_buffer_are_validated() {
        use windows::Win32::Media::MediaFoundation::*;
        let _com = crate::platform::windows_com::ComApartment::mta_or_existing().unwrap();
        let _media = MediaFoundationGuard::new().unwrap();
        unsafe {
            let media = MFCreateMediaType().unwrap();
            media.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_NV12).unwrap();
            media
                .SetUINT64(&MF_MT_FRAME_SIZE, (2u64 << 32) | 2)
                .unwrap();
            assert!(VideoLayout::from_media_type(&media).is_err());
            media.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_RGB32).unwrap();
            media.SetUINT32(&MF_MT_DEFAULT_STRIDE, 8).unwrap();
            let layout = VideoLayout::from_media_type(&media).unwrap();
            let sample = MFCreateSample().unwrap();
            let buffer = MFCreateMemoryBuffer(16).unwrap();
            let mut ptr = std::ptr::null_mut();
            buffer.Lock(&mut ptr, None, None).unwrap();
            std::slice::from_raw_parts_mut(ptr, 16).copy_from_slice(&[30, 60, 90, 0].repeat(4));
            buffer.Unlock().unwrap();
            buffer.SetCurrentLength(16).unwrap();
            sample.AddBuffer(&buffer).unwrap();
            let frame = subsample_frame(sample.clone(), layout).unwrap();
            assert_eq!(frame.dimensions(), (640, 640));
            assert_eq!(frame.get_pixel(0, 0).0, [30, 60, 90, 255]);
            buffer.SetCurrentLength(15).unwrap();
            assert!(subsample_frame(sample, layout).is_err());
            buffer.Lock(&mut ptr, None, None).unwrap();
            buffer.Unlock().unwrap();
        }
    }

    #[test]
    fn native_video_open_failure_is_reported() {
        let root = tempfile::tempdir().unwrap();
        let path = root
            .path()
            .join("missing.mp4")
            .to_string_lossy()
            .into_owned();
        let result =
            stream_video_frames_local(path, |_, _| panic!("missing file produced image"), || false);
        assert!(result.is_err());
    }
    #[test]
    fn native_scaled_frames_keep_colors_alpha_padding_and_signed_stride() {
        let mut pixels = Vec::new();
        for color in [[17, 43, 201, 0], [99, 21, 7, 0]] {
            pixels.extend(color.repeat(640));
            pixels.extend([0; 4]);
        }
        for (stride, first) in [(2564, [17, 43, 201, 255]), (-2564, [99, 21, 7, 255])] {
            let layout = VideoLayout::new(640, 2, stride).unwrap();
            let image = resize_video_frame(&pixels, layout).unwrap();
            assert_eq!(image.dimensions(), (640, 2));
            assert_eq!(image.get_pixel(0, 0).0, first);
            assert_eq!(image.get_pixel(639, 0).0, first);
            assert_eq!(image.get_pixel(639, 1).0[3], 255);
        }
    }
