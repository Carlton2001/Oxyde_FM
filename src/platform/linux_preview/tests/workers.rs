use super::*;
use std::io::Write;
use std::path::PathBuf;

/// The real worker: `cargo build --offline --bins` comes first.
fn oxyde() -> PathBuf {
    let executable = std::env::current_exe().unwrap().parent().unwrap().parent().unwrap().join("oxyde");
    assert!(executable.is_file(), "Build oxyde before worker tests: cargo build --offline --bins");
    executable
}

fn png(path: &Path, side: u32) {
    image::RgbaImage::from_pixel(side, side, image::Rgba([255, 0, 0, 255])).save(path).unwrap();
}

fn docx(path: &Path, text: &str) {
    let mut archive = zip::ZipWriter::new(File::create(path).unwrap());
    let options = zip::write::SimpleFileOptions::default();
    archive.start_file("word/document.xml", options).unwrap();
    write!(archive, r#"<?xml version="1.0"?><w:document xmlns:w="w"><w:body><w:p><w:r><w:t>{text}</w:t></w:r></w:p></w:body></w:document>"#).unwrap();
    archive.finish().unwrap();
}

fn wav(path: &Path) {
    let mut bytes = b"RIFF".to_vec();
    bytes.extend_from_slice(&(36u32 + 8).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16u32.to_le_bytes());
    for value in [1u16, 1] { bytes.extend_from_slice(&value.to_le_bytes()); }
    bytes.extend_from_slice(&8000u32.to_le_bytes());
    bytes.extend_from_slice(&16000u32.to_le_bytes());
    for value in [2u16, 16] { bytes.extend_from_slice(&value.to_le_bytes()); }
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&8u32.to_le_bytes());
    bytes.extend_from_slice(&[0; 8]);
    std::fs::write(path, bytes).unwrap();
}

#[test]
fn images_are_decoded_in_the_worker_at_each_mode_size() {
    let oxyde = oxyde();
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("rouge très long.png");
    png(&source, 300);
    for (mode, limits, side) in [
        ("image", preview_protocol::PREVIEW, 300),
        ("image-thumbnail", preview_protocol::PREVIEW, 128),
        ("image-viewer", preview_protocol::VIEWER, 300),
    ] {
        let bytes = run_in(&oxyde, mode, &source).unwrap();
        let Reply::Images { frames, .. } = preview_protocol::decode_with(&bytes, limits).unwrap() else { panic!("{mode}") };
        assert_eq!(frames[0].buffer().dimensions(), (side, side), "{mode}");
    }
}

#[test]
fn an_svg_gets_the_fonts_of_its_profile() {
    let oxyde = oxyde();
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("dessin.svg");
    std::fs::write(&source, r##"<svg xmlns="http://www.w3.org/2000/svg" width="64" height="32"><rect width="64" height="32" fill="#00f"/><text y="20">Oxyde</text></svg>"##).unwrap();
    let bytes = run_in(&oxyde, "image", &source).unwrap();
    let Reply::Images { natural_size, .. } = preview_protocol::decode(&bytes).unwrap() else { panic!("no image") };
    assert_eq!(natural_size, Some((64, 32)));
}

#[test]
fn a_damaged_image_only_fails_its_worker() {
    let oxyde = oxyde();
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("cassé.png");
    std::fs::write(&source, b"\x89PNG\r\n\x1a\nnot an image").unwrap();
    let bytes = run_in(&oxyde, "image", &source).unwrap();
    assert!(matches!(preview_protocol::decode(&bytes).unwrap(), Reply::Error { .. }));
}

#[test]
fn office_documents_reach_the_worker_with_their_extension() {
    let oxyde = oxyde();
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("rapport.docx");
    docx(&source, "contenu cherché");
    for mode in ["office-text", "office-document"] {
        let bytes = run_in(&oxyde, mode, &source).unwrap();
        let Reply::Text(text) = preview_protocol::decode(&bytes).unwrap() else { panic!("{mode}") };
        assert!(text.contains("contenu cherché"), "{mode}: {text}");
    }
    let found = office_search_in(&oxyde, &source, DEADLINE, &|| Ok(())).unwrap();
    assert!(found.contains("contenu cherché"), "{found}");
}

#[test]
fn a_cancelled_office_search_stops_before_its_worker() {
    let oxyde = oxyde();
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("rapport.docx");
    docx(&source, "texte");
    let error = office_search_in(&oxyde, &source, DEADLINE, &|| Err(io::ErrorKind::Interrupted.into())).unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::Interrupted);
}

#[test]
fn audio_tags_and_a_missing_cover_come_from_the_worker() {
    let oxyde = oxyde();
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("son.wav");
    wav(&source);
    let bytes = run_in(&oxyde, "audio-metadata", &source).unwrap();
    let metadata: audio_metadata::AudioMetadata = serde_json::from_slice(&bytes).unwrap();
    assert!(metadata.cover.is_none());
    let bytes = run_in(&oxyde, "audio-cover", &source).unwrap();
    assert!(matches!(preview_protocol::decode(&bytes).unwrap(), Reply::Text(text) if text.is_empty()));
}

#[test]
fn a_named_pipe_is_refused_without_blocking() {
    let oxyde = oxyde();
    let directory = tempfile::tempdir().unwrap();
    let pipe = directory.path().join("tube.png");
    rustix::fs::mknodat(rustix::fs::CWD, &pipe, rustix::fs::FileType::Fifo, rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR, 0).unwrap();
    assert_eq!(run_in(&oxyde, "image", &pipe).unwrap_err().kind(), io::ErrorKind::InvalidInput);
}
