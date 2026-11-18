use super::*;
use std::io::Cursor;

/// Whether the member's local header carries a ZIP64 extra field (id 1).
fn local_zip64(entry: &Entry) -> bool {
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    writer.start_file(entry.name.clone(), zip_options(entry, 6)).unwrap();
    writer.write_all(b"data").unwrap();
    let bytes = writer.finish().unwrap().into_inner();
    let name = u16::from_le_bytes([bytes[26], bytes[27]]) as usize;
    let extra = u16::from_le_bytes([bytes[28], bytes[29]]) as usize;
    let fields = &bytes[30 + name..30 + name + extra];
    fields.starts_with(&[1, 0])
}

#[test]
fn members_near_four_gibibytes_get_zip64_headers() {
    // Without them the writer stops at 4 GiB: "Large file option has not been set".
    assert!(local_zip64(&empty_entry(0, "video.mkv".into(), false, 5 << 30)));
    assert!(local_zip64(&empty_entry(0, "disk.vhdx".into(), false, u32::MAX as u64 - 1)));
    assert!(!local_zip64(&empty_entry(0, "notes.txt".into(), false, 4)));
}
