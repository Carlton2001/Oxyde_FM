use super::*;
use std::io::{Cursor, Write};

fn written(modified: Option<Timestamp>) -> Option<Timestamp> {
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let mut options = zip::write::SimpleFileOptions::default();
    if let Some(date) = to_zip(modified) {
        options = options.last_modified_time(date);
    }
    writer.start_file("file", options).unwrap();
    writer.write_all(b"data").unwrap();
    let mut archive = zip::ZipArchive::new(writer.finish().unwrap()).unwrap();
    let file = archive.by_index(0).unwrap();
    from_zip(&file)
}

#[test]
fn dates_survive_a_zip_to_the_even_second() {
    // 2021-06-15 10:20:31 UTC: DOS dates keep even seconds.
    let time = Timestamp { seconds: 1_623_752_431, nanos: 500 };
    assert_eq!(written(Some(time)).unwrap().seconds, 1_623_752_430);
    // Before 1980 a DOS date cannot hold it.
    assert!(to_zip(Some(Timestamp { seconds: 0, nanos: 0 })).is_none());
}

#[test]
fn utc_fields_take_precedence_over_the_local_dos_date() {
    let mut options = zip::write::FullFileOptions::default()
        .last_modified_time(zip::DateTime::from_date_and_time(1999, 1, 1, 0, 0, 0).unwrap());
    // Extended timestamp: flags (modified), then seconds since 1970.
    let mut extended = vec![1u8];
    extended.extend_from_slice(&1_623_752_431u32.to_le_bytes());
    options.add_extra_field(0x5455, extended, false).unwrap();
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    writer.start_file("file", options).unwrap();
    writer.write_all(b"data").unwrap();
    let mut archive = zip::ZipArchive::new(writer.finish().unwrap()).unwrap();
    let file = archive.by_index(0).unwrap();
    assert_eq!(from_zip(&file), Some(Timestamp { seconds: 1_623_752_431, nanos: 0 }));
}
