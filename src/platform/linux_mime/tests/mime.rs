use super::*;

const SAMPLE: &str = "# generated\n\
50:image/png:*.png\n\
50:application/gzip:*.gz\n\
50:application/x-compressed-tar:*.tar.gz\n\
50:text/x-genie:*.gs:cs\n\
50:text/x-a:*.amb\n\
50:text/x-b:*.amb\n\
10:text/x-readme:README*\n\
20:text/x-makefile:makefile\n";

#[test]
fn names_follow_the_specification_order() {
    let globs = parse(SAMPLE);
    assert_eq!(by_name(&globs, "photo.PNG"), ["image/png"]);
    assert_eq!(by_name(&globs, "archive.tar.gz"), ["application/x-compressed-tar"]);
    assert_eq!(by_name(&globs, "notes.gz"), ["application/gzip"]);
    assert_eq!(by_name(&globs, "script.gs"), ["text/x-genie"]);
    assert!(by_name(&globs, "script.GS").is_empty());
    assert_eq!(by_name(&globs, "README.md"), ["text/x-readme"]);
    assert_eq!(by_name(&globs, "Makefile"), ["text/x-makefile"]);
    assert_eq!(by_name(&globs, "both.amb"), ["text/x-a", "text/x-b"]);
    assert!(by_name(&globs, "notes").is_empty());
}

/// A magic file with two sections: PNG at offset 0, and a masked word searched in a
/// range under a parent rule.
fn sample_magic() -> Vec<u8> {
    let mut bytes = b"MIME-Magic\0\n[50:image/png]\n>0=".to_vec();
    bytes.extend([0, 4]);
    bytes.extend(b"\x89PNG\n[40:application/x-test]\n>0=");
    bytes.extend([0, 2]);
    bytes.extend(b"TT\n1>4=");
    bytes.extend([0, 2]);
    bytes.extend(b"\x0a\x0b&\xff\x0f~2+3\n");
    bytes
}

#[test]
fn content_rules_follow_their_tree() {
    let sections = magic::parse(&sample_magic());
    assert_eq!(sections.len(), 2);
    assert_eq!(magic::lookup(&sections, b"\x89PNG...."), Some(("image/png", 50)));
    // The word is stored big-endian and read in host order, masked, within offsets 4 to 6.
    let word = if cfg!(target_endian = "little") { [0xfb, 0x0a] } else { [0x0a, 0xfb] };
    let mut data = b"TT??..".to_vec();
    data.extend(word);
    assert_eq!(magic::lookup(&sections, &data), Some(("application/x-test", 40)));
    assert_eq!(magic::lookup(&sections, b"TT??......"), None);
    assert_eq!(magic::extent(&sections), 8);
}

#[test]
fn a_file_without_known_name_is_described_by_its_content() {
    let root = tempfile::tempdir().unwrap();
    let text = root.path().join("notes");
    std::fs::write(&text, "Café\n".repeat(1000)).unwrap();
    let binary = root.path().join("blob");
    std::fs::write(&binary, [0x01, 0x8b, 0, 0xff]).unwrap();
    let sections = magic::parse(&sample_magic());
    assert_eq!(by_content(&sections, &text, &[]), TEXT);
    assert_eq!(by_content(&sections, &binary, &[]), UNKNOWN);
    let png = root.path().join("image");
    std::fs::write(&png, b"\x89PNG\r\n\x1a\n").unwrap();
    assert_eq!(by_content(&sections, &png, &[]), "image/png");
    assert_eq!(by_content(&sections, &binary, &["text/x-a", "text/x-b"]), "text/x-a");
}

/// Reads the database installed on this system.
#[test]
fn system_types_are_described_in_the_interface_language() {
    if !Path::new("/usr/share/mime/image/png.xml").exists() { return }
    let png = mime_type(Path::new("/nonexistent/photo.png"), false, "fr");
    assert_eq!(png.name, "image/png");
    assert_eq!(png.description.as_deref(), Some("image PNG"));
    let folder = mime_type(Path::new("/tmp"), true, "en");
    assert_eq!((folder.name.as_str(), folder.description.as_deref()), ("inode/directory", Some("Folder")));
}

/// Same types as GIO for files whose name says nothing (`gio info` must be installed).
#[test]
fn content_types_match_gio() {
    if !Path::new("/usr/share/mime/magic").exists() { return }
    let root = tempfile::tempdir().unwrap();
    let samples: Vec<(&str, Vec<u8>)> = vec![
        ("program", std::fs::read("/usr/bin/true").unwrap()),
        ("image", b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec()),
        ("document", b"%PDF-1.7\n%\xe2\xe3\xcf\xd3\n".to_vec()),
        ("script", b"#!/bin/sh\necho bonjour\n".to_vec()),
        ("python", b"#!/usr/bin/env python3\nprint(1)\n".to_vec()),
        ("compressed", b"\x1f\x8b\x08\0\0\0\0\0\0\x03".to_vec()),
        ("archive", b"PK\x03\x04\x14\0\0\0\0\0".to_vec()),
        ("notes", b"just some words\n".to_vec()),
        ("page", b"<!DOCTYPE html>\n<html><body></body></html>\n".to_vec()),
        ("empty.png", Vec::new()),
        ("latin1-notes", b"caf\xe9 cr\xe8me\n".to_vec()),
        ("page.html", b"<?xml version=\"1.0\" ?>\n<!DOCTYPE html PUBLIC \"-//W3C//DTD XHTML 1.0 Strict//EN\" \"x\">\n<html xmlns=\"http://www.w3.org/1999/xhtml\">\n".to_vec()),
        ("unit.service", b"<?xml version=\"1.0\"?>\n<service-group></service-group>\n".to_vec()),
        ("archive.tar.gz", b"\x1f\x8b\x08\0\0\0\0\0\0\x03".to_vec()),
    ];
    for (name, bytes) in samples {
        let path = root.path().join(name);
        std::fs::write(&path, bytes).unwrap();
        let Ok(output) = std::process::Command::new("gio").args(["info", "-a", "standard::content-type"]).arg(&path).output() else { return };
        let gio = String::from_utf8_lossy(&output.stdout)
            .lines()
            .find_map(|line| line.trim().strip_prefix("standard::content-type: ").map(str::to_owned))
            .unwrap();
        assert_eq!(mime_type(&path, false, "en").name, gio, "{name}");
    }
}

#[test]
fn execution_is_offered_for_programs_and_text_only() {
    let mut parents = HashMap::new();
    parse_subclasses(
        "application/x-shellscript application/x-executable\n\
         application/x-csh application/x-shellscript\n\
         application/x-shellscript text/plain\n",
        &mut parents,
    );
    assert!(can_be_executable(&parents, "application/x-csh"));
    assert!(can_be_executable(&parents, "text/x-python"));
    assert!(can_be_executable(&parents, EXECUTABLE));
    assert!(!can_be_executable(&parents, "application/x-cd-image"));
    assert!(!can_be_executable(&parents, UNKNOWN));
    assert!(!can_be_executable(&parents, "image/png"));
}

#[test]
fn a_pipe_is_named_without_being_opened() {
    let root = tempfile::tempdir().unwrap();
    let pipe = root.path().join("pipe");
    rustix::fs::mknodat(rustix::fs::CWD, &pipe, rustix::fs::FileType::Fifo, rustix::fs::Mode::from_raw_mode(0o600), 0).unwrap();
    assert_eq!(mime_type(&pipe, false, "en").name, "inode/fifo");
}

/// Diagnostic: `OXYDE_TEST_MIME_DIR=<folder> cargo test -- --ignored gio_on_a_folder`
/// compares up to 3000 files of a folder with GIO and lists the differences.
#[test]
#[ignore = "compares a real folder with GIO"]
fn gio_on_a_folder() {
    let Some(root) = std::env::var_os("OXYDE_TEST_MIME_DIR") else { return };
    let files: Vec<_> = walkdir::WalkDir::new(root)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_file())
        .take(3000)
        .map(|entry| entry.into_path())
        .collect();
    let mut differences = Vec::new();
    for chunk in files.chunks(200) {
        let output = std::process::Command::new("gio").args(["info", "-a", "standard::content-type"]).args(chunk).output().unwrap();
        let types: Vec<_> = String::from_utf8_lossy(&output.stdout)
            .lines()
            .filter_map(|line| line.trim().strip_prefix("standard::content-type: ").map(str::to_owned))
            .collect();
        for (path, gio) in chunk.iter().zip(types) {
            let ours = mime_type(path, false, "en").name;
            if ours != gio {
                differences.push(format!("{}: {ours} / gio {gio}", path.display()));
            }
        }
    }
    println!("{} files, {} differences", files.len(), differences.len());
    for difference in differences.iter().take(40) {
        println!("{difference}");
    }
}
