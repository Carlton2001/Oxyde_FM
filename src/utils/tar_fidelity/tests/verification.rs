use super::*;
use std::io::Write;

fn archive(entries: &[(&str, &[u8], bool)]) -> tempfile::NamedTempFile {
    let mut file = tempfile::NamedTempFile::new().unwrap();
    {
        let mut builder = tar::Builder::new(file.as_file_mut());
        for (name, bytes, directory) in entries {
            let mut header = tar::Header::new_gnu();
            header.set_entry_type(if *directory {
                tar::EntryType::Directory
            } else {
                tar::EntryType::Regular
            });
            header.set_size(bytes.len() as u64);
            header.set_mode(0o644);
            header.set_mtime(123);
            header.set_cksum();
            builder.append_data(&mut header, name, *bytes).unwrap();
        }
        builder.finish().unwrap();
    }
    file
}
fn records(file: &mut File, selected: Option<&str>) -> io::Result<Vec<Record>> {
    scan(file, ArchiveLimits::WORKSPACE, selected, true, &|| Ok(()))
}
fn entry(record: &Record, id: u64) -> Entry {
    Entry {
        id,
        name: record.name.to_string_lossy().into_owned(),
        directory: record.directory,
        size: record.size,
        mode: record.mode,
        modified: record
            .modified
            .map(|seconds| super::super::archive_io_protocol::Timestamp { seconds, nanos: 0 }),
        created: None,
        accessed: None,
    }
}

#[test]
fn independent_tar_parser_verifies_long_names_empty_members_and_exact_selected_payloads() {
    let long = format!("docs/{}/école", "a".repeat(150));
    let mut file = archive(&[
        (".", b"", true),
        ("empty", b"", false),
        (&long, b"safe", false),
        ("neighbor", b"unselected", false),
    ]);
    let found = records(file.as_file_mut(), Some(&long)).unwrap();
    assert_eq!(found.len(), 4);
    assert_eq!(found[2].hash, Some(blake3::hash(b"safe")));
    assert_eq!(found[3].hash, None);
    let mut verifier = Verifier::new(found);
    for id in 0..4 {
        verifier
            .entry(&entry(&verifier.records[id], id as u64))
            .unwrap();
    }
    verifier.begin(2).unwrap();
    verifier.chunk(2, b"sa").unwrap();
    verifier.chunk(2, b"fe").unwrap();
    verifier.end(2).unwrap();
    verifier.finish().unwrap();
}

#[test]
fn valid_worker_protocol_cannot_hide_forged_payloads_metadata_or_missing_inventory() {
    for attack in ["bytes", "name", "mode", "time", "omit", "directory"] {
        let mut file = archive(&[("folder", b"", true), ("file", b"safe", false)]);
        let found = records(file.as_file_mut(), None).unwrap();
        let mut first = entry(&found[0], 0);
        let second = entry(&found[1], 1);
        let mut verifier = Verifier::new(found);
        match attack {
            "name" => first.name = "foreign".into(),
            "mode" => first.mode ^= 1,
            "time" => first.modified.as_mut().unwrap().seconds += 1,
            _ => {}
        }
        if matches!(attack, "name" | "mode" | "time") {
            assert!(verifier.entry(&first).is_err());
            continue;
        }
        verifier.entry(&first).unwrap();
        if attack == "omit" {
            assert!(verifier.finish().is_err());
            continue;
        }
        verifier.entry(&second).unwrap();
        if attack != "directory" {
            verifier.begin(0).unwrap();
            verifier.end(0).unwrap();
        }
        verifier.begin(1).unwrap();
        verifier
            .chunk(1, if attack == "bytes" { b"evil" } else { b"safe" })
            .unwrap();
        if attack == "bytes" {
            assert!(verifier.end(1).is_err());
        } else {
            verifier.end(1).unwrap();
            assert!(verifier.finish().is_err());
        }
    }
}

#[test]
fn checksum_truncation_trailing_data_and_duplicate_or_unsafe_names_are_refused() {
    for attack in ["checksum", "truncated", "trailing", "duplicate", "unsafe"] {
        let mut file = archive(&[("file", b"safe", false)]);
        match attack {
            "checksum" => {
                file.as_file_mut().seek(SeekFrom::Start(0)).unwrap();
                file.write_all(b"X").unwrap();
            }
            "truncated" => file.as_file_mut().set_len(515).unwrap(),
            "trailing" => {
                file.as_file_mut().seek(SeekFrom::End(0)).unwrap();
                file.write_all(b"evil").unwrap();
            }
            "duplicate" => file = archive(&[("file", b"safe", false), ("FILE", b"evil", false)]),
            _ => {
                let mut header = tar::Header::new_gnu();
                header.as_mut_bytes()[..10].copy_from_slice(b"../outside");
                header.set_size(0);
                header.set_cksum();
                file.as_file_mut().seek(SeekFrom::Start(0)).unwrap();
                file.write_all(header.as_bytes()).unwrap();
            }
        }
        assert!(records(file.as_file_mut(), None).is_err(), "{attack}");
    }
}

#[test]
fn canonical_encoder_output_rejects_hidden_ownership_link_and_padding_changes() {
    for attack in ["uid", "link", "padding"] {
        let mut file = archive(&[("file", b"safe", false)]);
        let mut header = tar::Header::new_gnu();
        file.as_file_mut().seek(SeekFrom::Start(0)).unwrap();
        file.read_exact(header.as_mut_bytes()).unwrap();
        match attack {
            "uid" => header.set_uid(123),
            "link" => {
                header.as_mut_bytes()[157] = b'X';
            }
            _ => {}
        }
        header.set_cksum();
        file.as_file_mut().seek(SeekFrom::Start(0)).unwrap();
        file.write_all(header.as_bytes()).unwrap();
        if attack == "padding" {
            file.as_file_mut().seek(SeekFrom::Start(516)).unwrap();
            file.write_all(b"X").unwrap();
        }
        assert!(
            scan_canonical(file.as_file_mut(), ArchiveLimits::WORKSPACE, &|| Ok(())).is_err(),
            "{attack}"
        );
    }
}

#[test]
fn posix_prefix_and_pax_path_are_verified_without_trusting_extension_payloads() {
    for pax_path in [false, true] {
        let mut file = tempfile::NamedTempFile::new().unwrap();
        {
            let mut builder = tar::Builder::new(file.as_file_mut());
            if pax_path {
                builder
                    .append_pax_extensions([("path", &b"docs/chosen"[..])])
                    .unwrap();
            }
            let mut header = tar::Header::new_ustar();
            header.set_size(4);
            header.set_mode(0o644);
            header.set_mtime(123);
            header.set_cksum();
            builder
                .append_data(
                    &mut header,
                    if pax_path { "ignored" } else { "docs/chosen" },
                    &b"safe"[..],
                )
                .unwrap();
            builder.finish().unwrap();
        }
        let found = records(file.as_file_mut(), Some("docs/chosen")).unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].hash, Some(blake3::hash(b"safe")));
    }
}
