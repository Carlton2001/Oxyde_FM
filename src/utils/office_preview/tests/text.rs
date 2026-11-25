use super::*;
use std::io::Write;

fn docx(xml: &str) -> (tempfile::TempDir, File) {
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("note.docx");
    let mut writer = zip::ZipWriter::new(File::create(&path).unwrap());
    writer.start_file("word/document.xml", zip::write::SimpleFileOptions::default()).unwrap();
    writer.write_all(xml.as_bytes()).unwrap();
    writer.finish().unwrap();
    let file = File::open(&path).unwrap();
    (folder, file)
}

#[test]
fn tooltip_text_decodes_entities_and_ignores_paragraph_properties() {
    let (_folder, file) = docx(
        r#"<w:document><w:body><w:p><w:pPr><w:pStyle w:val="Title"/></w:pPr><w:r><w:t>A &amp; B</w:t></w:r></w:p><w:p><w:r><w:t>C</w:t></w:r></w:p></w:body></w:document>"#,
    );
    assert_eq!(text(file, "docx").unwrap(), "A & B\nC");
}
