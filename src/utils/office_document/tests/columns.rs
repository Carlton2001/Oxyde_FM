use super::*;

const WIDE: usize = MAX_CELLS + 6;

/// The row every format must show: the first `MAX_CELLS` columns, each with its own value.
fn kept() -> String {
    (1..=MAX_CELLS).map(|column| column.to_string()).collect::<Vec<_>>().join("\u{1f}")
}

fn letters(mut index: usize) -> String {
    let mut name = String::new();
    loop {
        name.insert(0, (b'A' + (index % 26) as u8) as char);
        if index < 26 {
            return name;
        }
        index = index / 26 - 1;
    }
}

#[test]
fn wide_spreadsheet_rows_keep_each_kept_column_value() {
    use std::io::Write;
    let cells: String = (0..WIDE)
        .map(|index| format!(r#"<c r="{}1"><v>{}</v></c>"#, letters(index), index + 1))
        .collect();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("wide.xlsx");
    let mut writer = zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
    for (name, content) in [
        ("xl/workbook.xml", r#"<workbook><sheets><sheet name="Wide" r:id="rId1"/></sheets></workbook>"#.to_owned()),
        ("xl/_rels/workbook.xml.rels", r#"<Relationships><Relationship Id="rId1" Target="worksheets/sheet1.xml"/></Relationships>"#.to_owned()),
        ("xl/worksheets/sheet1.xml", format!("<worksheet><sheetData><row>{cells}</row></sheetData></worksheet>")),
    ] {
        writer.start_file(name, zip::write::SimpleFileOptions::default()).unwrap();
        writer.write_all(content.as_bytes()).unwrap();
    }
    writer.finish().unwrap();
    let output = document(std::fs::File::open(&path).unwrap(), "xlsx").unwrap();
    assert_eq!(output, format!("sheet\tWide\ntr\t{}\n", kept()));
}

#[test]
fn wide_word_and_open_document_tables_drop_the_extra_columns() {
    let word: String = (1..=WIDE)
        .map(|value| format!("<w:tc><w:p><w:r><w:t>{value}</w:t></w:r></w:p></w:tc>"))
        .collect();
    let word = parse(&format!("<w:document><w:body><w:tbl><w:tr>{word}</w:tr></w:tbl></w:body></w:document>"), None);
    assert_eq!(word, format!("tr\t{}\n", kept()));
    let odf: String = (1..=WIDE)
        .map(|value| format!("<table:table-cell><text:p>{value}</text:p></table:table-cell>"))
        .collect();
    let odf = parse(
        &format!(r#"<office:spreadsheet><table:table table:name="Wide"><table:table-row>{odf}</table:table-row></table:table></office:spreadsheet>"#),
        Some(Odf::Sheet),
    );
    assert_eq!(odf, format!("sheet\tWide\ntr\t{}\n", kept()));
}
