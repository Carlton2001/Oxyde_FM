use super::*;

const SAMPLE: &str = "# generated\n\
50:image/png:*.png\n\
50:application/gzip:*.gz\n\
50:application/x-compressed-tar:*.tar.gz\n\
50:text/x-genie:*.gs:cs\n\
10:text/x-readme:README*\n\
20:text/x-makefile:makefile\n";

#[test]
fn names_follow_the_specification_order() {
    let globs = parse(SAMPLE);
    assert_eq!(by_name(&globs, "photo.PNG"), Some("image/png"));
    assert_eq!(by_name(&globs, "archive.tar.gz"), Some("application/x-compressed-tar"));
    assert_eq!(by_name(&globs, "notes.gz"), Some("application/gzip"));
    assert_eq!(by_name(&globs, "script.gs"), Some("text/x-genie"));
    assert_eq!(by_name(&globs, "script.GS"), None);
    assert_eq!(by_name(&globs, "README.md"), Some("text/x-readme"));
    assert_eq!(by_name(&globs, "Makefile"), Some("text/x-makefile"));
    assert_eq!(by_name(&globs, "notes"), None);
}

#[test]
fn a_file_without_known_name_is_described_by_its_content() {
    let root = tempfile::tempdir().unwrap();
    let text = root.path().join("notes");
    std::fs::write(&text, "Café\n".repeat(1000)).unwrap();
    let binary = root.path().join("blob");
    std::fs::write(&binary, [0x7f, b'E', b'L', b'F', 0, 1]).unwrap();
    assert_eq!(by_content(&text), TEXT);
    assert_eq!(by_content(&binary), UNKNOWN);
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

#[test]
fn a_pipe_is_named_without_being_opened() {
    let root = tempfile::tempdir().unwrap();
    let pipe = root.path().join("pipe");
    rustix::fs::mknodat(rustix::fs::CWD, &pipe, rustix::fs::FileType::Fifo, rustix::fs::Mode::from_raw_mode(0o600), 0).unwrap();
    assert_eq!(mime_type(&pipe, false, "en").name, "inode/fifo");
}
