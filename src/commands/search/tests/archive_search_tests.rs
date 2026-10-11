    use super::*;

    #[test]
    fn archive_search_warns_and_keeps_results_after_multiple_skipped_archives() {
        let temp = tempfile::tempdir().unwrap();
        let extensions = [
            "zip", "tar", "tar.gz", "tar.xz", "tar.zst", "tar.bz2", "iso", "7z", "rar",
        ];
        // More warnings than the channel capacity exercise backpressure too.
        for extension in extensions {
            std::fs::write(temp.path().join(format!("broken.{extension}")), b"bad").unwrap();
        }
        std::fs::write(temp.path().join("match.txt"), b"valid result").unwrap();
        let receiver = advanced_search(
            temp.path().to_string_lossy().into_owned(),
            AdvancedSearchOptions {
                query: "match".into(),
                archives: true,
                ..Default::default()
            },
            100,
            true,
            true,
            Arc::new(AtomicBool::new(false)),
        );
        let mut skipped = std::collections::HashSet::new();
        let mut results = Vec::new();
        loop {
            let event = receiver
                .recv_timeout(std::time::Duration::from_secs(5))
                .unwrap();
            let terminal = event.is_terminal();
            match event {
                QuickSearchEvent::ArchiveSkipped { path, detail } => {
                    assert!(!terminal);
                    assert!(std::path::Path::new(&path).starts_with(temp.path()));
                    assert!(!detail.is_empty());
                    skipped.insert(path);
                }
                QuickSearchEvent::Batch(batch) => {
                    assert!(!terminal);
                    results.extend(batch);
                }
                QuickSearchEvent::Finished { limited } => {
                    assert!(terminal);
                    assert!(!limited);
                    break;
                }
                QuickSearchEvent::Failed(error) => panic!("{error}"),
            }
        }
        assert_eq!(skipped.len(), extensions.len());
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "match.txt");
    }

    #[test]
    fn archive_search_warning_preserves_the_limit_kind_and_threshold() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("deep.zip");
        let name = format!("{}match.txt", "n/".repeat(64));
        metadata_fixture(&path, &[(&name, tar::EntryType::Regular)], 123);
        let receiver = advanced_search(
            temp.path().to_string_lossy().into_owned(),
            AdvancedSearchOptions {
                query: "match".into(),
                archives: true,
                ..Default::default()
            },
            100,
            true,
            true,
            Arc::new(AtomicBool::new(false)),
        );
        let mut warning = None;
        loop {
            match receiver
                .recv_timeout(std::time::Duration::from_secs(5))
                .unwrap()
            {
                QuickSearchEvent::ArchiveSkipped {
                    path: skipped,
                    detail,
                } => {
                    assert_eq!(std::path::Path::new(&skipped), path);
                    warning = crate::models::error::ArchiveLimitFailure::from_message(&detail);
                }
                QuickSearchEvent::Finished { limited } => {
                    assert!(!limited);
                    break;
                }
                QuickSearchEvent::Batch(batch) => assert!(batch.is_empty()),
                QuickSearchEvent::Failed(error) => panic!("{error}"),
            }
        }
        assert_eq!(
            warning,
            Some(crate::models::error::ArchiveLimitFailure {
                kind: crate::models::error::ArchiveLimitKind::Depth,
                limit: 64,
            })
        );
    }

    #[test]
    fn archive_search_cancellation_is_not_reported_as_a_skipped_archive() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(temp.path().join("broken.zip"), b"bad").unwrap();
        let receiver = advanced_search(
            temp.path().to_string_lossy().into_owned(),
            AdvancedSearchOptions {
                archives: true,
                ..Default::default()
            },
            100,
            true,
            true,
            Arc::new(AtomicBool::new(true)),
        );
        assert!(matches!(
            receiver.recv_timeout(std::time::Duration::from_secs(5)),
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected)
        ));
    }

    fn metadata_fixture(path: &std::path::Path, names: &[(&str, tar::EntryType)], mtime: u64) {
        use std::io::Write;
        if path.extension().is_some_and(|ext| ext == "zip") {
            let mut writer = zip::ZipWriter::new(File::create(path).unwrap());
            for (name, kind) in names {
                if kind.is_symlink() {
                    writer
                        .add_symlink(*name, "safe.txt", zip::write::SimpleFileOptions::default())
                        .unwrap();
                } else {
                    writer
                        .start_file(
                            *name,
                            zip::write::SimpleFileOptions::default()
                                .last_modified_time(
                                    zip::DateTime::from_date_and_time(2009, 2, 13, 23, 31, 30)
                                        .unwrap(),
                                )
                                .compression_method(zip::CompressionMethod::Stored),
                        )
                        .unwrap();
                    writer.write_all(b"safe").unwrap();
                }
            }
            writer.finish().unwrap();
        } else {
            let mut builder = tar::Builder::new(Vec::new());
            for (name, kind) in names {
                let mut header = tar::Header::new_gnu();
                header.as_mut_bytes()[..name.len()].copy_from_slice(name.as_bytes());
                header.set_entry_type(*kind);
                header.set_mode(0o644);
                header.set_size(4);
                header.set_mtime(mtime);
                header.set_cksum();
                builder.append(&header, &b"safe"[..]).unwrap();
            }
            let bytes = builder.into_inner().unwrap();
            let format = ArchiveFormat::from_path(path).unwrap();
            let mut contents = Vec::new();
            match format {
                ArchiveFormat::Tar => contents = bytes,
                ArchiveFormat::TarZst => {
                    contents = zstd::stream::encode_all(&bytes[..], 1).unwrap()
                }
                format => {
                    let mut encoder: Box<dyn Read + '_> = match format {
                        ArchiveFormat::TarGz => Box::new(flate2::read::GzEncoder::new(
                            &bytes[..],
                            flate2::Compression::fast(),
                        )),
                        ArchiveFormat::TarXz => Box::new(xz2::read::XzEncoder::new(&bytes[..], 1)),
                        ArchiveFormat::TarBz2 => Box::new(bzip2::read::BzEncoder::new(
                            &bytes[..],
                            bzip2::Compression::fast(),
                        )),
                        _ => panic!("unsupported metadata fixture"),
                    };
                    encoder.read_to_end(&mut contents).unwrap();
                }
            }
            std::fs::write(path, contents).unwrap();
        }
    }

    #[test]
    fn zip_tar_metadata_search_rejects_unsafe_names_before_returning_results() {
        let temp = tempfile::tempdir().unwrap();
        let mut rejected = Vec::new();
        for extension in ["zip", "tar", "tar.gz", "tar.xz", "tar.zst", "tar.bz2"] {
            let path = temp.path().join(format!("unsafe.{extension}"));
            metadata_fixture(
                &path,
                &[
                    ("safe.txt", tar::EntryType::Regular),
                    ("../outside.txt", tar::EntryType::Regular),
                ],
                123,
            );
            let result = search_in_archive(&path, &params(), &Arc::new(AtomicBool::new(false)));
            rejected.push((extension, result.is_empty()));
        }
        assert!(
            rejected.iter().all(|(_, empty)| *empty),
            "unsafe index rejection: {rejected:?}"
        );
    }

    #[test]
    fn zip_tar_metadata_rejects_special_entries_in_search_and_navigation() {
        let temp = tempfile::tempdir().unwrap();
        let mut rejected = Vec::new();
        for extension in ["zip", "tar", "tar.gz", "tar.xz", "tar.zst", "tar.bz2"] {
            let path = temp.path().join(format!("link.{extension}"));
            metadata_fixture(
                &path,
                &[
                    ("safe.txt", tar::EntryType::Regular),
                    ("link.txt", tar::EntryType::Symlink),
                ],
                123,
            );
            let navigation = super::super::archive::list_archive_contents(
                path.to_string_lossy().into_owned(),
                String::new(),
            )
            .is_err();
            let search =
                search_in_archive(&path, &params(), &Arc::new(AtomicBool::new(false))).is_empty();
            rejected.push((extension, navigation, search));
        }
        assert!(
            rejected
                .iter()
                .all(|(_, navigation, search)| *navigation && *search),
            "special index rejection: {rejected:?}"
        );
    }

    #[test]
    fn zip_tar_metadata_rejects_overflowing_tar_dates_without_panicking() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("date.tar");
        metadata_fixture(&path, &[("safe.txt", tar::EntryType::Regular)], u64::MAX);
        let result = std::panic::catch_unwind(|| {
            search_in_archive(&path, &params(), &Arc::new(AtomicBool::new(false)))
        });
        let listing = std::panic::catch_unwind(|| {
            super::super::archive::list_archive_contents(
                path.to_string_lossy().into_owned(),
                String::new(),
            )
        });
        assert!(
            result.is_ok() && listing.is_ok(),
            "TAR timestamp panicked: search={}, navigation={}",
            result.is_err(),
            listing.is_err()
        );
        assert!(result.unwrap().is_empty());
        assert!(listing.unwrap().is_err());
    }

    #[test]
    fn zip_tar_metadata_search_preserves_name_size_date_filters_and_cancellation() {
        let temp = tempfile::tempdir().unwrap();
        for extension in ["zip", "tar", "tar.gz", "tar.xz", "tar.zst", "tar.bz2"] {
            let path = temp.path().join(format!("filters.{extension}"));
            metadata_fixture(
                &path,
                &[("folder/Été.txt", tar::EntryType::Regular)],
                1_234_567_890,
            );
            let cancellation = Arc::new(AtomicBool::new(false));
            let mut options = params();
            options.pattern = SearchPattern::Literal("ETE".into(), false, true);
            options.min_size = Some(4);
            options.max_size = Some(4);
            options.min_date = Some(1_234_567_890_000);
            options.max_date = Some(1_234_567_890_000);
            let result = search_in_archive(&path, &options, &cancellation);
            assert_eq!(result.len(), 1, "{extension}");
            assert_eq!(
                std::path::Path::new(&result[0].path),
                path.join("folder/Été.txt")
            );
            options.max_size = Some(3);
            assert!(search_in_archive(&path, &options, &cancellation).is_empty());
            options.max_size = Some(4);
            options.max_date = Some(1_234_567_889_999);
            assert!(search_in_archive(&path, &options, &cancellation).is_empty());
            options.max_date = None;
            cancellation.store(true, Ordering::Relaxed);
            assert!(search_in_archive(&path, &options, &cancellation).is_empty());
        }
    }

    #[test]
    fn zip_tar_metadata_zip_names_do_not_require_a_supported_content_codec() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("codec.zip");
        metadata_fixture(&path, &[("safe.txt", tar::EntryType::Regular)], 123);
        let mut bytes = std::fs::read(&path).unwrap();
        let local = bytes.windows(4).position(|b| b == b"PK\x03\x04").unwrap();
        let central = bytes.windows(4).position(|b| b == b"PK\x01\x02").unwrap();
        bytes[local + 8..local + 10].copy_from_slice(&0x7fu16.to_le_bytes());
        bytes[central + 10..central + 12].copy_from_slice(&0x7fu16.to_le_bytes());
        std::fs::write(&path, bytes).unwrap();
        assert!(zip::ZipArchive::new(File::open(&path).unwrap())
            .unwrap()
            .by_index(0)
            .is_err());
        let results = search_in_archive(&path, &params(), &Arc::new(AtomicBool::new(false)));
        assert_eq!(
            results.len(),
            1,
            "name search constructed a ZIP content decoder"
        );
        assert_eq!(
            super::super::archive::list_archive_contents(
                path.to_string_lossy().into_owned(),
                String::new()
            )
            .unwrap()
            .len(),
            1
        );
    }

    fn params() -> SearchParams {
        SearchParams {
            pattern: SearchPattern::Literal(String::new(), false, false),
            min_size: None,
            max_size: None,
            min_date: None,
            max_date: None,
        }
    }

    #[test]
    fn seven_search_lists_header_names_without_constructing_a_content_decoder() {
        let temp = tempfile::tempdir().unwrap();
        let archive = temp.path().join("unsupported-codec.7z");
        super::super::archive_seven_open::block_crc_fixture_with_method(&archive, false, 0x7f);
        let mut reader = sevenz_rust2::ArchiveReader::new(
            File::open(&archive).unwrap(),
            sevenz_rust2::Password::empty(),
        )
        .unwrap();
        assert_eq!(reader.archive().files.len(), 2);
        assert!(reader.for_each_entries(|_, _| Ok(true)).is_err());
        let results = search_in_archive(&archive, &params(), &Arc::new(AtomicBool::new(false)));
        assert_eq!(results.len(), 2, "name search depends on a content decoder");
        assert_eq!(results[0].name, "selected.txt");
        assert_eq!(results[1].name, "tail.txt");
    }

    #[test]
    fn seven_search_preserves_size_filters_and_cancellation() {
        let temp = tempfile::tempdir().unwrap();
        let archive = temp.path().join("filters.7z");
        super::super::archive_seven_open::block_crc_fixture_with_method(&archive, false, 0);
        let cancellation = Arc::new(AtomicBool::new(false));
        let mut params = params();
        params.pattern = SearchPattern::Literal("SELECTED".into(), false, false);
        params.min_size = Some(4);
        params.max_size = Some(4);
        let results = search_in_archive(&archive, &params, &cancellation);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].size, 4);
        assert_eq!(
            std::path::Path::new(&results[0].path),
            archive.join("selected.txt")
        );
        params.min_size = Some(5);
        params.max_size = None;
        assert!(search_in_archive(&archive, &params, &cancellation).is_empty());
        params.min_size = None;
        cancellation.store(true, Ordering::Relaxed);
        assert!(search_in_archive(&archive, &params, &cancellation).is_empty());
    }

    #[test]
    fn iso_search_keeps_nested_paths_size_filters_and_cancellation() {
        let temp = tempfile::tempdir().unwrap();
        let image = temp.path().join("disc.img");
        crate::commands::archive::test_iso_image(&image);
        let cancellation = Arc::new(AtomicBool::new(false));
        let mut params = params();
        let results = search_in_archive(&image, &params, &cancellation);
        assert_eq!(results.len(), 3);
        assert!(results
            .iter()
            .any(|entry| entry.is_dir && entry.name == "DOCS"));
        params.min_size = Some(5);
        params.max_size = Some(11);
        params.pattern = SearchPattern::Literal("readme".into(), false, false);
        let results = search_in_archive(&image, &params, &cancellation);
        assert_eq!(results.len(), 1);
        assert_eq!(
            std::path::Path::new(&results[0].path),
            image.join("DOCS/README.TXT")
        );
        assert_eq!(results[0].size, 11);
        cancellation.store(true, Ordering::Relaxed);
        assert!(search_in_archive(&image, &params, &cancellation).is_empty());
    }

    #[test]
    fn iso_search_rejects_malformed_identifiers_incomplete_records_and_cycles() {
        let temp = tempfile::tempdir().unwrap();
        let image = temp.path().join("invalid.iso");
        for variant in 0..3 {
            crate::commands::archive::test_iso_image(&image);
            let mut bytes = std::fs::read(&image).unwrap();
            let directory = 20 * 2048 + 2 * 34;
            let file = directory + bytes[directory] as usize;
            match variant {
                0 => bytes[file + 32] = 255,
                1 => bytes[file] = 20,
                2 => {
                    bytes[directory + 2..directory + 6].copy_from_slice(&20u32.to_le_bytes());
                    bytes[directory + 6..directory + 10].copy_from_slice(&20u32.to_be_bytes());
                }
                _ => unreachable!(),
            }
            std::fs::write(&image, bytes).unwrap();
            assert!(
                search_in_archive(&image, &params(), &Arc::new(AtomicBool::new(false))).is_empty()
            );
        }
    }
