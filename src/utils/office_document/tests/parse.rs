use super::*;

#[test]
fn attributes_respect_boundaries_spacing_and_both_xml_quotes() {
    assert_eq!(attribute("c t = 's' r=\"ZZ999\"", "t"), Some("s"));
    assert_eq!(attribute("sheet name='école budget' r:id = 'r1'", "r:id"), Some("r1"));
    assert_eq!(attribute("c other-t=\"s\"", "t"), None);
    assert_eq!(attribute("c t=\"unterminated", "t"), None);
}

#[test]
fn word_paragraphs_headings_lists_and_tables_keep_their_order() {
    let xml = r#"<w:document><w:body>
<w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:t>Intro &amp; plan</w:t></w:r></w:p>
<w:p><w:r><w:t xml:space="preserve">Hello </w:t></w:r><w:r><w:t>world</w:t></w:r></w:p>
<w:p><w:pPr><w:numPr><w:ilvl w:val="0"/></w:numPr></w:pPr><w:r><w:t>item</w:t></w:r></w:p>
<w:tbl><w:tr><w:tc><w:p><w:r><w:t>a</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>b</w:t></w:r></w:p></w:tc></w:tr></w:tbl>
<w:p><w:r><w:t>end</w:t></w:r></w:p>
</w:body></w:document>"#;
    assert_eq!(
        parse(xml, None),
        "h1\tIntro & plan\np\tHello world\nli\titem\ntr\ta\u{1f}b\np\tend\n"
    );
}

#[test]
fn odf_headings_and_paragraphs_ignore_markup_between_blocks() {
    let xml = r#"<office:document-content><office:automatic-styles><style:style/></office:automatic-styles>
<office:body><office:text>
<text:h text:outline-level="2">Title</text:h>
<text:p>Some <text:span>styled</text:span> text</text:p>
</office:text></office:body></office:document-content>"#;
    assert_eq!(parse(xml, Some(Odf::Text)), "h2\tTitle\np\tSome styled text\n");
}

fn archive(parts: &[(&str, &str)], extension: &str) -> String {
    use std::io::Write;
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join(format!("document.{extension}"));
    let mut writer = zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
    for (name, content) in parts {
        writer.start_file(*name, zip::write::SimpleFileOptions::default()).unwrap();
        writer.write_all(content.as_bytes()).unwrap();
    }
    writer.finish().unwrap();
    document(std::fs::File::open(&path).unwrap(), extension).unwrap()
}

#[test]
fn spreadsheets_list_every_sheet_with_shared_strings_and_column_gaps() {
    let output = archive(
        &[
            ("xl/workbook.xml", r#"<workbook><sheets><sheet name="Budget &amp; plan" r:id="rId1"/><sheet name="Two" r:id="rId2"/></sheets></workbook>"#),
            ("xl/_rels/workbook.xml.rels", r#"<Relationships><Relationship Id="rId1" Target="worksheets/sheet1.xml"/><Relationship Id="rId2" Target="/xl/worksheets/sheet2.xml"/></Relationships>"#),
            ("xl/sharedStrings.xml", r#"<sst><si><t>Name</t></si><si><r><t>Va</t></r><r><t>lue</t></r></si></sst>"#),
            ("xl/worksheets/sheet1.xml", r#"<worksheet><sheetData><row r="1"><c r="A1" t="s"><v>0</v></c><c r="C1" t="s"><v>1</v></c></row><row r="2"><c r="A2"><v>42</v></c><c r="B2" t="b"><v>1</v></c></row></sheetData></worksheet>"#),
            ("xl/worksheets/sheet2.xml", r#"<worksheet><sheetData><row><c t="inlineStr"><is><t>inline</t></is></c></row></sheetData></worksheet>"#),
        ],
        "xlsx",
    );
    assert_eq!(
        output,
        "sheet\tBudget & plan\ntr\tName\u{1f}\u{1f}Value\ntr\t42\u{1f}TRUE\nsheet\tTwo\ntr\tinline\n"
    );
}

#[test]
fn presentations_keep_slide_order_and_skip_empty_paragraphs() {
    let output = archive(
        &[
            ("ppt/presentation.xml", r#"<p:presentation><p:sldIdLst><p:sldId r:id="rId3"/><p:sldId r:id="rId2"/></p:sldIdLst></p:presentation>"#),
            ("ppt/_rels/presentation.xml.rels", r#"<Relationships><Relationship Id="rId2" Target="slides/slide1.xml"/><Relationship Id="rId3" Target="slides/slide2.xml"/></Relationships>"#),
            ("ppt/slides/slide1.xml", r#"<p:sld><a:p><a:r><a:t>Second</a:t></a:r></a:p></p:sld>"#),
            ("ppt/slides/slide2.xml", r#"<p:sld><a:p><a:r><a:t>First</a:t></a:r></a:p><a:p></a:p></p:sld>"#),
        ],
        "pptx",
    );
    assert_eq!(output, "slide\t1\np\tFirst\nslide\t2\np\tSecond\n");
}

#[test]
fn open_document_sheets_and_slides_have_headers_and_bounded_cells() {
    let sheet = parse(
        r#"<office:spreadsheet><table:table table:name="Data"><table:table-row><table:table-cell><text:p>a</text:p></table:table-cell><table:table-cell table:number-columns-repeated="16384"/><table:table-cell><text:p>b</text:p></table:table-cell></table:table-row></table:table></office:spreadsheet>"#,
        Some(Odf::Sheet),
    );
    assert!(sheet.starts_with("sheet\tData\ntr\ta\u{1f}"));
    assert_eq!(sheet.lines().nth(1).unwrap().split('\u{1f}').count(), MAX_CELLS);
    let slides = parse(
        r#"<office:presentation><draw:page draw:name="One"><draw:frame><draw:text-box><text:p>Hello</text:p></draw:text-box></draw:frame><presentation:notes><text:p>secret</text:p></presentation:notes></draw:page></office:presentation>"#,
        Some(Odf::Slides),
    );
    assert_eq!(slides, "slide\tOne\np\tHello\n");
}
