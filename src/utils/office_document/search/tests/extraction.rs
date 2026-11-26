use super::*;
use std::io::{Seek, SeekFrom, Write};

fn package(parts: &[(&str, &str)]) -> File {
    let mut file = tempfile::tempfile().unwrap();
    let mut zip = zip::ZipWriter::new(&mut file);
    for (name, xml) in parts {
        zip.start_file(
            *name,
            zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated),
        )
        .unwrap();
        zip.write_all(xml.as_bytes()).unwrap();
    }
    zip.finish().unwrap();
    file.seek(SeekFrom::Start(0)).unwrap();
    file
}

#[test]
fn word_entities_and_split_runs_match_visible_text() {
    let text = extract(
        package(&[(
            "word/document.xml",
            "<w:p><w:r><w:t>A&amp;</w:t></w:r><w:r><w:t>B &#xE9;cole</w:t></w:r></w:p>",
        )]),
        "docx",
        &|| Ok(()),
    )
    .unwrap();
    assert_eq!(text, "A&B école\n");
}

#[test]
fn worksheet_attribute_variants_and_boolean_cells_keep_their_visible_values() {
    let file = package(&[
        (
            "xl/sharedStrings.xml",
            "<sst><si><t>selected</t></si></sst>",
        ),
        (
            "xl/worksheets/sheet1.xml",
            "<row><c t = 's'><v>0</v></c><c t='b'><v>1</v></c></row>",
        ),
    ]);
    assert_eq!(
        extract(file, "xlsx", &|| Ok(())).unwrap(),
        "selected\nTRUE\n"
    );
}

#[test]
fn search_visits_later_slides_and_all_worksheet_columns_with_inline_and_shared_text() {
    let slides = extract(
        package(&[
            ("ppt/slides/slide1.xml", "<a:p><a:t>first</a:t></a:p>"),
            ("ppt/slides/slide4.xml", "<a:p><a:t>fourth</a:t></a:p>"),
        ]),
        "pptx",
        &|| Ok(()),
    )
    .unwrap();
    assert_eq!(slides, "first\nfourth\n");
    let sheet = extract(package(&[
        ("xl/sharedStrings.xml", "<sst><si><r><t>sha</t></r><r><t>red &amp; text</t></r></si><si><t>unused</t></si></sst>"),
        ("xl/worksheets/sheet1.xml", "<worksheet><row><c t=\"s\"><v>0</v></c></row></worksheet>"),
        ("xl/worksheets/sheet2.xml", "<worksheet><row><c r=\"ZZ999\" t=\"inlineStr\"><is><t>A&amp;amp;B</t></is></c></row></worksheet>"),
    ]), "xlsx", &|| Ok(())).unwrap();
    assert_eq!(sheet, "shared & text\nA&amp;B\n");
    assert!(!sheet.contains("unused"));
}

#[test]
fn open_document_preserves_spans_and_entities_without_speaker_notes() {
    let text = extract(package(&[("content.xml", "<text:p>A <text:span>B &amp; C</text:span></text:p><presentation:notes><text:p>hidden</text:p></presentation:notes>")]), "odp", &|| Ok(())).unwrap();
    assert_eq!(text, "A B & C\n");
}

#[test]
fn xml_and_output_limits_are_errors_instead_of_successful_truncations() {
    for count in [MAX_TEXT + 1, MAX_PART + 1] {
        let xml = format!("<w:p><w:t>{}</w:t></w:p>", "x".repeat(count));
        let file = package(&[("word/document.xml", &xml)]);
        assert_eq!(
            extract(file, "docx", &|| Ok(())).unwrap_err().kind(),
            io::ErrorKind::FileTooLarge
        );
    }
    let mut zip = zip::ZipArchive::new(package(&[("a", "aaa"), ("b", "bbb")])).unwrap();
    let mut remaining = 5;
    assert_eq!(
        part(&mut zip, "a", &mut remaining, &|| Ok(())).unwrap(),
        "aaa"
    );
    assert_eq!(remaining, 2);
    assert_eq!(
        part(&mut zip, "b", &mut remaining, &|| Ok(()))
            .unwrap_err()
            .kind(),
        io::ErrorKind::FileTooLarge
    );
}

#[test]
fn cancellation_is_checked_inside_decompression_and_missing_parts_are_not_empty_successes() {
    let mut zip = zip::ZipArchive::new(package(&[("a", &"x".repeat(1024 * 1024))])).unwrap();
    let checks = std::cell::Cell::new(0);
    let mut remaining = MAX_XML;
    let error = part(&mut zip, "a", &mut remaining, &|| {
        checks.set(checks.get() + 1);
        if checks.get() >= 4 {
            Err(io::ErrorKind::Interrupted.into())
        } else {
            Ok(())
        }
    })
    .unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::Interrupted);
    assert_eq!(checks.get(), 4);
    assert!(
        extract(package(&[("other.xml", "not a document")]), "docx", &|| Ok(
            ()
        ))
        .is_err()
    );
    let invalid = package(&[("xl/worksheets/sheet1.xml", "<c t=\"s\"><v>10</v></c>")]);
    assert!(extract(invalid, "xlsx", &|| Ok(())).is_err());
}

#[test]
fn open_document_text_after_an_anchored_text_box_is_still_searched() {
    let text = extract(package(&[("content.xml", "<text:p>Before <draw:frame><draw:text-box><text:p>Boxed</text:p></draw:text-box></draw:frame> after</text:p>")]), "odt", &|| Ok(())).unwrap();
    assert!(text.contains("Before") && text.contains("Boxed") && text.contains("after"), "{text:?}");
}

#[test]
fn workbook_notes_and_chart_text_are_searched_besides_the_cells() {
    let file = package(&[
        ("xl/worksheets/sheet1.xml", "<row><c t='inlineStr'><is><t>cell</t></is></c></row>"),
        (
            "xl/comments1.xml",
            "<comments><commentList><comment ref='A1'><text><r><t>Author:</t></r><r><t xml:space='preserve'> check totals</t></r></text></comment></commentList></comments>",
        ),
        (
            "xl/charts/chart1.xml",
            "<c:chartSpace><c:chart><c:title><c:tx><c:rich><a:p><a:r><a:t>Quarterly sales</a:t></a:r></a:p></c:rich></c:tx></c:title>\
             <c:ser><c:tx><c:strRef><c:strCache><c:pt idx='0'><c:v>Revenue</c:v></c:pt></c:strCache></c:strRef></c:tx>\
             <c:val><c:numRef><c:numCache><c:pt idx='0'><c:v>42</c:v></c:pt></c:numCache></c:numRef></c:val></c:ser></c:chart></c:chartSpace>",
        ),
        ("xl/charts/style1.xml", "<cs:chartStyle><a:t>not a chart</a:t></cs:chartStyle>"),
    ]);
    assert_eq!(
        extract(file, "xlsx", &|| Ok(())).unwrap(),
        "cell\nAuthor: check totals\nQuarterly sales\nRevenue\n"
    );
}
