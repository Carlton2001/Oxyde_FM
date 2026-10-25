    use super::*;

    fn advanced(root: &std::path::Path, options: AdvancedSearchOptions) -> Vec<FileEntry> {
        let receiver = advanced_search(
            root.to_string_lossy().into_owned(),
            options,
            100,
            true,
            true,
            Arc::new(AtomicBool::new(false)),
        );
        let mut results = Vec::new();
        loop {
            match receiver
                .recv_timeout(std::time::Duration::from_secs(5))
                .unwrap()
            {
                QuickSearchEvent::Batch(batch) => results.extend(batch),
                QuickSearchEvent::ArchiveSkipped { .. } => {},
                QuickSearchEvent::Finished { .. } => return results,
                QuickSearchEvent::Failed(error) | QuickSearchEvent::ContentSkipped { detail: error, .. } => panic!("{error}"),
            }
        }
    }

    #[test]
    fn advanced_filters_content_accents_size_and_recursion() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("nested")).unwrap();
        std::fs::write(dir.path().join("Été.txt"), "Résumé").unwrap();
        std::fs::write(dir.path().join("nested/Été.txt"), "Résumé").unwrap();
        std::fs::write(dir.path().join("Other.txt"), "none").unwrap();
        let options = AdvancedSearchOptions {
            query: "ete.*".into(),
            content: "resume".into(),
            ignore_accents: true,
            min_size: Some(3),
            max_size: Some(30),
            ..Default::default()
        };
        assert_eq!(advanced(dir.path(), options.clone()).len(), 1);
        assert_eq!(
            advanced(
                dir.path(),
                AdvancedSearchOptions {
                    recursive: true,
                    ..options.clone()
                }
            )
            .len(),
            2
        );
        assert!(advanced(
            dir.path(),
            AdvancedSearchOptions {
                min_size: Some(20),
                ..options
            }
        )
        .is_empty());
    }

    #[test]
    fn advanced_regex_case_dates_and_invalid_bounds() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("Photo1.txt"), "example").unwrap();
        std::fs::write(dir.path().join("photo2.txt"), "example").unwrap();
        let options = AdvancedSearchOptions {
            query: r"^Photo\d".into(),
            regex: true,
            case_sensitive: true,
            ..Default::default()
        };
        assert_eq!(advanced(dir.path(), options.clone()).len(), 1);
        assert!(advanced(
            dir.path(),
            AdvancedSearchOptions {
                max_date: Some(1),
                ..options
            }
        )
        .is_empty());
        assert!(AdvancedSearchOptions {
            query: "[".into(),
            regex: true,
            ..Default::default()
        }
        .validate()
        .is_err());
        assert!(AdvancedSearchOptions {
            min_size: Some(3),
            max_size: Some(1),
            ..Default::default()
        }
        .validate()
        .is_err());
    }

    #[test]
    fn advanced_searches_zip_entry_names() {
        use std::io::Write;
        let dir = tempfile::tempdir().unwrap();
        let file = File::create(dir.path().join("archive.zip")).unwrap();
        let mut writer = zip::ZipWriter::new(file);
        writer
            .start_file(
                "folder/result.txt",
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
        writer.write_all(b"test").unwrap();
        writer.finish().unwrap();
        let options = AdvancedSearchOptions {
            query: "result".into(),
            ..Default::default()
        };
        assert!(advanced(dir.path(), options.clone()).is_empty());
        let results = advanced(
            dir.path(),
            AdvancedSearchOptions {
                archives: true,
                ..options
            },
        );
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "result.txt");
    }

    #[test]
    fn advanced_stream_respects_limit_and_content_regex() {
        let dir = tempfile::tempdir().unwrap();
        for index in 0..5 {
            std::fs::write(dir.path().join(format!("{index}.txt")), "photo42").unwrap();
        }
        let options = AdvancedSearchOptions {
            content: r"photo\d+".into(),
            content_regex: true,
            ..Default::default()
        };
        let receiver = advanced_search(
            dir.path().to_string_lossy().into_owned(),
            options,
            2,
            true,
            true,
            Arc::new(AtomicBool::new(false)),
        );
        let mut count = 0;
        loop {
            match receiver
                .recv_timeout(std::time::Duration::from_secs(5))
                .unwrap()
            {
                QuickSearchEvent::Batch(batch) => count += batch.len(),
                QuickSearchEvent::ArchiveSkipped { .. } => {},
                QuickSearchEvent::Finished { limited } => {
                    assert!(limited);
                    assert_eq!(count, 2);
                    break;
                }
                QuickSearchEvent::Failed(error) | QuickSearchEvent::ContentSkipped { detail: error, .. } => panic!("{error}"),
            }
        }
    }

    fn collect(
        root: &std::path::Path,
        query: &str,
        limit: usize,
        cancelled: bool,
    ) -> (Vec<FileEntry>, bool) {
        let receiver = quick_search(
            root.to_string_lossy().into_owned(),
            query.to_owned(),
            limit,
            true,
            true,
            Arc::new(AtomicBool::new(cancelled)),
        );
        let mut files = Vec::new();
        while let Ok(event) = receiver.recv_timeout(std::time::Duration::from_secs(5)) {
            match event {
                QuickSearchEvent::Batch(batch) => files.extend(batch),
                QuickSearchEvent::ArchiveSkipped { .. } => {},
                QuickSearchEvent::Finished { limited } => return (files, limited),
                QuickSearchEvent::Failed(error) | QuickSearchEvent::ContentSkipped { detail: error, .. } => panic!("{error}"),
            }
        }
        assert!(cancelled, "search did not finish");
        (files, false)
    }

    #[test]
    fn recursive_names_are_case_insensitive_and_keep_actual_paths() {
        let directory = tempfile::tempdir().unwrap();
        let nested = directory.path().join("UserFolder");
        std::fs::create_dir(&nested).unwrap();
        std::fs::write(nested.join("en-US.json"), "abc").unwrap();
        std::fs::write(nested.join("other.txt"), "us in content only").unwrap();
        let (files, limited) = collect(directory.path(), "us", 100, false);
        assert!(!limited);
        assert_eq!(files.len(), 2);
        assert!(files
            .iter()
            .any(|file| file.is_dir && file.name == "UserFolder"));
        let file = files.iter().find(|file| file.name == "en-US.json").unwrap();
        assert_eq!(std::path::Path::new(&file.path), nested.join("en-US.json"));
        assert_eq!(file.size, 3);
    }

    #[test]
    fn wildcard_limit_and_cancellation() {
        let directory = tempfile::tempdir().unwrap();
        for name in ["a.json", "b.JSON", "c.txt"] {
            std::fs::write(directory.path().join(name), "").unwrap();
        }
        let (files, limited) = collect(directory.path(), "*.json", 1, false);
        assert!(limited);
        assert_eq!(files.len(), 1);
        assert!(files[0].name.to_lowercase().ends_with(".json"));
        assert!(collect(directory.path(), "*", 100, true).0.is_empty());
        assert!(collect(directory.path(), "absent", 100, false).0.is_empty());
    }

    #[test]
    fn invalid_root_reports_failure() {
        let directory = tempfile::tempdir().unwrap();
        let receiver = quick_search(
            directory
                .path()
                .join("absent")
                .to_string_lossy()
                .into_owned(),
            "us".into(),
            100,
            true,
            true,
            Arc::new(AtomicBool::new(false)),
        );
        assert!(matches!(
            receiver
                .recv_timeout(std::time::Duration::from_secs(5))
                .unwrap(),
            QuickSearchEvent::Failed(_)
        ));
    }
