use super::*;
use std::{io::Write, path::Path};

fn collect(root: &Path, options: AdvancedSearchOptions) -> (Vec<FileEntry>, Vec<String>) {
    let receiver = advanced_search(
        root.to_string_lossy().into_owned(),
        options,
        100,
        true,
        true,
        Arc::new(AtomicBool::new(false)),
    );
    let mut files = Vec::new();
    let mut warnings = Vec::new();
    loop {
        match receiver
            .recv_timeout(std::time::Duration::from_secs(15))
            .expect("search completion")
        {
            QuickSearchEvent::Batch(batch) => files.extend(batch),
            QuickSearchEvent::ArchiveSkipped { detail, .. } => warnings.push(detail),
            QuickSearchEvent::Finished { limited } => {
                assert!(!limited);
                return (files, warnings);
            }
            QuickSearchEvent::Failed(error) => panic!("{error}"),
        }
    }
}

fn fixture(root: &Path, extension: &str) {
    std::fs::write(root.join("needle.txt"), b"no match here").unwrap();
    std::fs::create_dir(root.join("needle_folder")).unwrap();
    let path = root.join(format!("container.{extension}"));
    if extension == "zip" {
        let mut zip = zip::ZipWriter::new(File::create(path).unwrap());
        zip.start_file("needle.txt", zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(b"no match here").unwrap();
        zip.add_directory("needle_folder/", zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.finish().unwrap();
    } else {
        let mut tar = tar::Builder::new(Vec::new());
        for (name, kind, bytes) in [
            ("needle.txt", tar::EntryType::Regular, &b"no match here"[..]),
            ("needle_folder", tar::EntryType::Directory, &b""[..]),
        ] {
            let mut header = tar::Header::new_gnu();
            header.set_entry_type(kind);
            header.set_size(bytes.len() as u64);
            header.set_mode(0o755);
            header.set_mtime(1_234_567_890);
            header.set_cksum();
            tar.append_data(&mut header, name, bytes).unwrap();
        }
        let bytes = tar.into_inner().unwrap();
        let encoded = match extension {
            "tar" => bytes,
            "tar.gz" => {
                let mut w = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
                w.write_all(&bytes).unwrap();
                w.finish().unwrap()
            }
            "tar.xz" => {
                let mut w = xz2::write::XzEncoder::new(Vec::new(), 1);
                w.write_all(&bytes).unwrap();
                w.finish().unwrap()
            }
            "tar.bz2" => {
                let mut w = bzip2::write::BzEncoder::new(Vec::new(), bzip2::Compression::fast());
                w.write_all(&bytes).unwrap();
                w.finish().unwrap()
            }
            "tar.zst" => zstd::stream::encode_all(&bytes[..], 1).unwrap(),
            _ => unreachable!(),
        };
        std::fs::write(path, encoded).unwrap();
    }
}

#[test]
fn archive_content_criteria_never_return_unverified_members_and_warn() {
    for extension in ["zip", "tar", "tar.gz", "tar.xz", "tar.bz2", "tar.zst"] {
        let temp = tempfile::tempdir().unwrap();
        fixture(temp.path(), extension);
        for (content, regex, expected) in [
            ("ABSENT_MARKER", false, 0),
            ("no match here", false, 1),
            (r"^no\s+match", true, 1),
        ] {
            let (files, warnings) = collect(
                temp.path(),
                AdvancedSearchOptions {
                    query: "needle".into(),
                    content: content.into(),
                    content_regex: regex,
                    archives: true,
                    min_size: Some(10),
                    ..Default::default()
                },
            );
            assert_eq!(files.len(), expected, "{extension}: {content}");
            assert!(files.iter().all(|f| f.archive_path.is_none() && !f.is_dir));
            assert_eq!(
                warnings.len(),
                1,
                "missing content limitation for {extension}"
            );
            assert_eq!(
                ArchiveSearchLimitation::from_message(&warnings[0]),
                Some(ArchiveSearchLimitation::ContentNotSearched)
            );
        }
    }
}

#[test]
fn real_and_archived_directories_obey_the_same_size_contract() {
    for extension in ["zip", "tar", "tar.gz", "tar.xz", "tar.bz2", "tar.zst"] {
        let temp = tempfile::tempdir().unwrap();
        fixture(temp.path(), extension);
        for (min_size, max_size, expected) in [
            (None, None, 4),
            (Some(0), None, 2),
            (None, Some(13), 2),
            (Some(13), Some(13), 2),
            (Some(14), None, 0),
            (None, Some(12), 0),
        ] {
            let (files, warnings) = collect(
                temp.path(),
                AdvancedSearchOptions {
                    query: "needle".into(),
                    archives: true,
                    min_size,
                    max_size,
                    ..Default::default()
                },
            );
            assert!(warnings.is_empty(), "{extension}: {warnings:?}");
            assert_eq!(
                files.len(),
                expected,
                "{extension}: {min_size:?}..{max_size:?}"
            );
            if min_size.is_some() || max_size.is_some() {
                assert!(files.iter().all(|f| !f.is_dir));
            }
        }
    }
}

#[test]
fn missing_iso_and_seven_dates_do_not_satisfy_date_filters() {
    let temp = tempfile::tempdir().unwrap();
    crate::commands::archive::test_iso_image(&temp.path().join("disc.iso"));
    crate::commands::archive_seven_open::block_crc_fixture_with_method(
        &temp.path().join("data.7z"),
        false,
        0,
    );
    for (min_date, max_date) in [(Some(0), None), (None, Some(u64::MAX))] {
        let (files, warnings) = collect(
            temp.path(),
            AdvancedSearchOptions {
                archives: true,
                min_date,
                max_date,
                ..Default::default()
            },
        );
        assert!(files.iter().all(|f| f.archive_path.is_none()));
        assert_eq!(warnings.len(), 2);
        assert!(warnings
            .iter()
            .all(|warning| ArchiveSearchLimitation::from_message(warning)
                == Some(ArchiveSearchLimitation::DatesUnavailable)));
    }
    let (files, warnings) = collect(
        temp.path(),
        AdvancedSearchOptions {
            archives: true,
            min_size: Some(0),
            ..Default::default()
        },
    );
    assert!(warnings.is_empty());
    assert_eq!(files.iter().filter(|f| f.archive_path.is_some()).count(), 4);
    assert!(files.iter().all(|f| !f.is_dir));
    let (files, warnings) = collect(
        temp.path(),
        AdvancedSearchOptions {
            archives: true,
            content: "unverified".into(),
            ..Default::default()
        },
    );
    assert!(files.iter().all(|f| f.archive_path.is_none()));
    assert_eq!(warnings.len(), 2);
    assert!(warnings
        .iter()
        .all(|warning| ArchiveSearchLimitation::from_message(warning)
            == Some(ArchiveSearchLimitation::ContentNotSearched)));
    let (files, warnings) = collect(
        temp.path(),
        AdvancedSearchOptions {
            archives: true,
            ..Default::default()
        },
    );
    assert_eq!(files.iter().filter(|f| f.archive_path.is_some()).count(), 5);
    assert!(warnings.is_empty());
}
