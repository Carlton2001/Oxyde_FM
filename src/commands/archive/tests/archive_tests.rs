use super::*;
use crate::systems::archive_workspace::mutation_operation;
use crate::systems::file_ops::FileOperationManager;

#[test]
fn seven_full_extract_preserves_typed_io_limits_from_decoder_callbacks() {
    use crate::models::error::{ArchiveLimitFailure, ArchiveLimitKind};
    let temp = tempfile::tempdir().unwrap();
    let archive = temp.path().join("deadline.7z");
    let mut writer = sevenz_rust2::ArchiveWriter::create(&archive).unwrap();
    writer
        .push_archive_entry(
            sevenz_rust2::ArchiveEntry::new_file("safe.txt"),
            Some(&b"safe"[..]),
        )
        .unwrap();
    writer.finish().unwrap();
    let target = tempfile::tempdir().unwrap();
    let expired = ArchiveBudget::new(ArchiveLimits {
        max_duration: std::time::Duration::ZERO,
        ..ArchiveLimits::EXTRACTION
    });
    let error = extract_archive_with_checkpoint(
        archive.to_string_lossy().into_owned(),
        target.path().to_string_lossy().into_owned(),
        &|| {
            if target.path().join("safe.txt").exists() {
                expired.check_time()
            } else {
                Ok(())
            }
        },
    )
    .unwrap_err();
    let failure = ArchiveLimitFailure::from_message(&error.to_string())
        .expect("7z erased the structured deadline error");
    assert_eq!(failure.kind, ArchiveLimitKind::Seconds);
    assert_eq!(failure.limit, 0);
}

#[test]
fn listings_refuse_paths_exceeding_the_shared_depth_limit() {
    use std::io::Write;
    let dir = tempfile::tempdir().unwrap();
    let name = format!("{}file.txt", "a/".repeat(65));
    for extension in ["zip", "7z", "tar", "zst"] {
        let path = dir.path().join(format!("deep.{extension}"));
        match extension {
            "zip" => {
                let mut writer = zip::ZipWriter::new(File::create(&path).unwrap());
                writer
                    .start_file(&name, zip::write::SimpleFileOptions::default())
                    .unwrap();
                writer.write_all(b"safe").unwrap();
                writer.finish().unwrap();
            }
            "7z" => {
                let mut writer = sevenz_rust2::ArchiveWriter::create(&path).unwrap();
                writer
                    .push_archive_entry(
                        sevenz_rust2::ArchiveEntry::new_file(&name),
                        Some(&b"safe"[..]),
                    )
                    .unwrap();
                writer.finish().unwrap();
            }
            _ => {
                let output: Box<dyn Write> = if extension == "zst" {
                    Box::new(
                        zstd::stream::write::Encoder::new(File::create(&path).unwrap(), 1)
                            .unwrap()
                            .auto_finish(),
                    )
                } else {
                    Box::new(File::create(&path).unwrap())
                };
                let mut builder = tar::Builder::new(output);
                let mut header = tar::Header::new_gnu();
                header.set_size(4);
                header.set_mode(0o644);
                header.set_cksum();
                builder
                    .append_data(&mut header, &name, &b"safe"[..])
                    .unwrap();
                builder.finish().unwrap();
            }
        }
        assert!(
            list_archive_contents(path.to_string_lossy().into_owned(), String::new()).is_err(),
            "unbounded listing for {extension}"
        );
    }
}

#[test]
fn targeted_zip_open_does_not_extract_other_files() {
    use std::io::Write;
    let dir = tempfile::tempdir().unwrap();
    let archive = dir.path().join("select.zip");
    let mut writer = zip::ZipWriter::new(File::create(&archive).unwrap());
    for (name, data) in [
        ("selected.txt", &b"safe"[..]),
        ("other.txt", &b"unselected and over the limit"[..]),
    ] {
        writer
            .start_file(name, zip::write::SimpleFileOptions::default())
            .unwrap();
        writer.write_all(data).unwrap();
    }
    writer.finish().unwrap();
    let output = tempfile::tempdir().unwrap();
    let limits = ArchiveLimits {
        max_bytes: 4,
        ..ArchiveLimits::OPEN_FILE
    };
    let selected =
        extract_zip_file_for_open(&archive, "selected.txt", output.path(), limits).unwrap();
    assert_eq!(fs::read(selected).unwrap(), b"safe");
    assert!(!output.path().join("other.txt").exists());
    let second = tempfile::tempdir().unwrap();
    assert!(extract_zip_file_for_open(&archive, "other.txt", second.path(), limits).is_err());
    assert!(fs::read_dir(second.path()).unwrap().next().is_none());
}

#[test]
fn targeted_zip_preflight_does_not_construct_unselected_decoders() {
    use std::io::Write;
    let temp = tempfile::tempdir().unwrap();
    let archive = temp.path().join("metadata-only.zip");
    let mut zip = zip::ZipWriter::new(File::create(&archive).unwrap());
    for name in ["selected.txt", "other.txt"] {
        zip.start_file(name, zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored)).unwrap();
        zip.write_all(b"safe").unwrap();
    }
    zip.finish().unwrap();
    let mut bytes = fs::read(&archive).unwrap();
    // A valid index whose unselected member uses an unsupported codec.
    // Change matching local/central headers only, without decoding data.
    let mut patched = 0;
    for offset in 0..bytes.len().saturating_sub(46) {
        let (name, method) = match &bytes[offset..offset + 4] {
            b"PK\x03\x04" => (offset + 30, offset + 8),
            b"PK\x01\x02" => (offset + 46, offset + 10),
            _ => continue,
        };
        if bytes.get(name..name + 9) == Some(&b"other.txt"[..]) {
            bytes[method..method + 2].copy_from_slice(&200u16.to_le_bytes());
            patched += 1;
        }
    }
    assert_eq!(patched, 2);
    fs::write(&archive, bytes).unwrap();
    let output = tempfile::tempdir().unwrap();
    let selected = extract_zip_file_for_open(&archive, "selected.txt", output.path(), ArchiveLimits::OPEN_FILE).unwrap();
    assert_eq!(fs::read(selected).unwrap(), b"safe");
    let refused = tempfile::tempdir().unwrap();
    assert!(extract_zip_file_for_open(&archive, "other.txt", refused.path(), ArchiveLimits::OPEN_FILE).is_err());
}

#[test]
fn targeted_zip_cancellation_precedes_destination_creation() {
    use std::io::Write;
    let temp = tempfile::tempdir().unwrap();
    let archive = temp.path().join("cancel.zip");
    let mut zip = zip::ZipWriter::new(File::create(&archive).unwrap());
    zip.start_file("safe.txt", zip::write::SimpleFileOptions::default()).unwrap();
    zip.write_all(b"safe").unwrap();
    zip.finish().unwrap();
    let output = tempfile::tempdir().unwrap();
    let error = extract_zip_file_with_checkpoint(&archive, "safe.txt", output.path(), ArchiveLimits::OPEN_FILE, &|| {
        Err(io::Error::new(io::ErrorKind::Interrupted, "cancelled before ZIP output"))
    }).unwrap_err();
    assert!(error.to_string().contains("cancelled before ZIP output"));
    assert_eq!(fs::read_dir(output.path()).unwrap().count(), 0);
}

#[cfg(target_os = "windows")]
#[test]
fn targeted_zip_retains_source_and_provenance_after_parent_retargeting() {
    use crate::platform::windows_reparse_tests::{junction, Retarget};
    use std::io::Write;
    let temp = tempfile::tempdir().unwrap();
    let chosen = temp.path().join("chosen");
    let outside = temp.path().join("outside");
    fs::create_dir(&chosen).unwrap();
    fs::create_dir(&outside).unwrap();
    for (folder, content) in [(&chosen, &b"safe"[..]), (&outside, &b"evil"[..])] {
        let mut zip = zip::ZipWriter::new(File::create(folder.join("source.zip")).unwrap());
        zip.start_file("safe.txt", zip::write::SimpleFileOptions::default()).unwrap();
        zip.write_all(content).unwrap();
        zip.finish().unwrap();
    }
    let archive = chosen.join("source.zip");
    let provenance = b"[ZoneTransfer]\r\nZoneId=3\r\n";
    platform().write_file_provenance(&archive, provenance).unwrap();
    let link = temp.path().join("link");
    let template = temp.path().join("template");
    junction(&link, &chosen);
    junction(&template, &outside);
    let retarget = Retarget::new(&link, &template);
    let changed = std::cell::Cell::new(false);
    let output = tempfile::tempdir().unwrap();
    let result = extract_zip_file_with_checkpoint(&link.join("source.zip"), "safe.txt", output.path(), ArchiveLimits::OPEN_FILE, &|| {
        if !changed.replace(true) {
            assert!(File::options().write(true).open(&archive).is_err());
            assert!(fs::rename(&archive, chosen.join("moved.zip")).is_err());
            retarget.apply();
        }
        Ok(())
    }).unwrap();
    assert!(changed.get());
    assert_eq!(fs::read(&result).unwrap(), b"safe");
    assert_eq!(platform().read_file_provenance(&result).unwrap().as_deref(), Some(&provenance[..]));
    drop(retarget);
    fs::remove_dir(link).unwrap();
    fs::remove_dir(template).unwrap();
    assert!(File::options().write(true).open(&archive).is_ok());
}

#[test]
fn extraction_limits_apply_to_every_supported_decoder() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("file.txt");
    fs::write(&source, b"bounded test content").unwrap();
    let limits = ArchiveLimits {
        max_bytes: 2,
        ..ArchiveLimits::EXTRACTION
    };
    for extension in ["zip", "7z", "tar", "zst", "iso"] {
        let archive = dir.path().join(format!("bounded.{extension}"));
        if extension == "iso" {
            test_iso_image(&archive);
        } else {
            FileOperationManager::perform_compress(
                &mutation_operation("fast"),
                vec![source.clone()],
                Some(archive.clone()),
            )
            .unwrap();
        }
        let output = tempfile::tempdir().unwrap();
        let result = extract_archive_with_limits(
            archive.to_string_lossy().into_owned(),
            output.path().to_string_lossy().into_owned(),
            &|| Ok(()),
            limits,
        );
        assert!(result.is_err(), "unbounded {extension}");
        assert_eq!(fs::metadata(&source).unwrap().len(), 20);
    }
}

#[test]
fn zip_preflight_limits_counts_and_depth_and_rejects_ambiguous_selection() {
    use std::io::Write;
    let dir = tempfile::tempdir().unwrap();
    let archive = dir.path().join("paths.zip");
    let mut writer = zip::ZipWriter::new(File::create(&archive).unwrap());
    for name in ["a/b/file.txt", "a\\b\\file.txt"] {
        writer
            .start_file(name, zip::write::SimpleFileOptions::default())
            .unwrap();
        writer.write_all(b"safe").unwrap();
    }
    writer.finish().unwrap();
    for limits in [
        ArchiveLimits {
            max_depth: 2,
            ..ArchiveLimits::OPEN_FILE
        },
        ArchiveLimits {
            max_entries: 1,
            ..ArchiveLimits::OPEN_FILE
        },
        ArchiveLimits::OPEN_FILE,
    ] {
        let output = tempfile::tempdir().unwrap();
        assert!(
            extract_zip_file_for_open(&archive, "a/b/file.txt", output.path(), limits).is_err()
        );
        assert!(fs::read_dir(output.path()).unwrap().next().is_none());
    }
}

#[test]
fn zip_listing_and_workspace_reject_windows_paths_and_streams() {
    use std::io::Write;
    for name in [
        "a/C:victim.txt",
        "a/../C:victim.txt",
        "C:sentinel.txt",
        "file.txt:extra",
        "dir./file",
        "NUL.txt",
    ] {
        let dir = tempfile::tempdir().unwrap();
        let archive = dir.path().join("unsafe.zip");
        let mut writer = zip::ZipWriter::new(File::create(&archive).unwrap());
        writer
            .start_file("readme.txt", zip::write::SimpleFileOptions::default())
            .unwrap();
        writer.write_all(b"safe entry").unwrap();
        writer
            .start_file(name, zip::write::SimpleFileOptions::default())
            .unwrap();
        writer.write_all(b"untrusted entry").unwrap();
        writer.finish().unwrap();
        assert!(
            list_archive_contents(archive.to_string_lossy().into_owned(), String::new())
                .is_err(),
            "listed {name}"
        );
        assert!(
            with_archive_cache_base_for_tests(dir.path(), || {
                prepare_archive_file_for_open(&archive.join("readme.txt"))
            })
            .is_err(),
            "opened {name}"
        );
    }
}

#[cfg(target_os = "windows")]
#[test]
fn archive_outputs_and_external_copies_preserve_internet_provenance() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("readme.txt");
    fs::write(&source, b"safe document").unwrap();
    let data = b"[ZoneTransfer]\r\nZoneId=3\r\n";
    for extension in ["zip", "7z", "tar", "zst"] {
        let archive = dir.path().join(format!("marked.{extension}"));
        FileOperationManager::perform_compress(
            &mutation_operation("fast"),
            vec![source.clone()],
            Some(archive.clone()),
        )
        .unwrap();
        platform().write_file_provenance(&archive, data).unwrap();
        let target = dir.path().join(extension);
        extract_archive_with_checkpoint(
            archive.to_string_lossy().into_owned(),
            target.to_string_lossy().into_owned(),
            &|| Ok(()),
        )
        .unwrap();
        assert_eq!(
            platform()
                .read_file_provenance(&target.join("readme.txt"))
                .unwrap()
                .as_deref(),
            Some(data.as_slice())
        );
        let cached = with_archive_cache_base_for_tests(dir.path(), || {
            prepare_archive_file_for_open(&archive.join("readme.txt"))
        })
        .unwrap();
        assert_eq!(
            platform().read_file_provenance(&cached).unwrap().as_deref(),
            Some(data.as_slice())
        );
        fs::remove_file(&cached).unwrap();
        fs::remove_dir(cached.parent().unwrap()).unwrap();
    }
}

#[test]
fn seven_zip_extraction_rejects_absolute_and_parent_paths() {
    let dir = tempfile::tempdir().unwrap();
    let root = fs::canonicalize(dir.path()).unwrap();

    for name in [
        "../outside.txt",
        "nested/../../outside.txt",
        "..\\outside.txt",
        "C:/outside.txt",
        "\\\\server\\share\\outside.txt",
        "/outside.txt",
    ] {
        assert!(
            safe_seven_zip_destination(&root, name).is_err(),
            "unsafe archive path was accepted: {name}"
        );
    }
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 0);
}

#[test]
fn seven_zip_extraction_resolves_nested_paths_under_the_root() {
    let dir = tempfile::tempdir().unwrap();
    let root = fs::canonicalize(dir.path()).unwrap();
    let destination = safe_seven_zip_destination(&root, "nested\\folder\\file.txt").unwrap();

    assert_eq!(
        destination,
        root.join("nested").join("folder").join("file.txt")
    );
    assert!(destination.parent().unwrap().is_dir());
}

#[test]
fn seven_zip_extraction_rejects_malicious_archive_entries() {
    let dir = tempfile::tempdir().unwrap();
    let archive_path = dir.path().join("malicious.7z");
    let target = dir.path().join("extract");
    fs::create_dir(&target).unwrap();

    let mut writer = sevenz_rust2::ArchiveWriter::create(&archive_path).unwrap();
    writer
        .push_archive_entry(
            sevenz_rust2::ArchiveEntry::new_file("../escaped.txt"),
            Some(&b"must not be written outside the target"[..]),
        )
        .unwrap();
    writer.finish().unwrap();

    assert!(extract_archive_with_checkpoint(
        archive_path.to_string_lossy().into_owned(),
        target.to_string_lossy().into_owned(),
        &|| Ok(()),
    )
    .is_err());
    assert!(!dir.path().join("escaped.txt").exists());
}

#[test]
fn seven_zip_compression_and_extraction_round_trip() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("source.txt");
    let archive_path = dir.path().join("roundtrip.7z");
    let target = dir.path().join("extract");
    fs::write(&source, b"round-trip payload").unwrap();
    fs::create_dir(&target).unwrap();
    FileOperationManager::perform_compress(
        &mutation_operation("fast"),
        vec![source],
        Some(archive_path.clone()),
    )
    .unwrap();
    extract_archive_with_checkpoint(
        archive_path.to_string_lossy().into_owned(),
        target.to_string_lossy().into_owned(),
        &|| Ok(()),
    )
    .unwrap();

    assert_eq!(
        fs::read(target.join("source.txt")).unwrap(),
        b"round-trip payload"
    );
}

#[test]
fn iso_extraction_rejects_path_components() {
    for name in ["", ".", "..", "../outside", "C:", "\\outside"] {
        assert!(
            safe_iso_component(name).is_err(),
            "accepted ISO name: {name}"
        );
    }
    assert_eq!(
        safe_iso_component("document.txt;1").unwrap(),
        "document.txt;1"
    );
}
