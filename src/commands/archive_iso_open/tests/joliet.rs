use super::*;
use crate::commands::archive::test_iso_image;

fn record(name: &[u8], sector: u32, length: u32, directory: bool) -> Vec<u8> {
    let mut bytes = vec![0; (33 + name.len() + 1) & !1];
    bytes[0] = bytes.len() as u8;
    bytes[2..6].copy_from_slice(&sector.to_le_bytes());
    bytes[6..10].copy_from_slice(&sector.to_be_bytes());
    bytes[10..14].copy_from_slice(&length.to_le_bytes());
    bytes[14..18].copy_from_slice(&length.to_be_bytes());
    bytes[25] = if directory { 2 } else { 0 };
    bytes[28..30].copy_from_slice(&1u16.to_le_bytes());
    bytes[30..32].copy_from_slice(&1u16.to_be_bytes());
    bytes[32] = name.len() as u8;
    bytes[33..33 + name.len()].copy_from_slice(name);
    bytes
}

fn ucs2(name: &str) -> Vec<u8> {
    name.encode_utf16().flat_map(u16::to_be_bytes).collect()
}

/// The fixture's ISO 9660 tree (`DOCS/README.TXT`, `ROOT.TXT`), plus a Joliet
/// descriptor in sector 17 whose tree names the same files in Unicode. The
/// terminator moves to sector 18, over a path table the reader does not use.
fn joliet_image(escape: &[u8; 3]) -> (tempfile::TempDir, PathBuf) {
    let folder = tempfile::tempdir().unwrap();
    let archive = folder.path().join("disc.iso");
    test_iso_image(&archive);
    let mut bytes = std::fs::read(&archive).unwrap();
    let primary = bytes[16 * 2048..17 * 2048].to_vec();
    let supplementary = &mut bytes[17 * 2048..18 * 2048];
    supplementary.copy_from_slice(&primary);
    supplementary[0] = 2;
    supplementary[88..91].copy_from_slice(escape);
    supplementary[156..190].copy_from_slice(&record(&[0], 24, 2048, true));
    bytes[18 * 2048..19 * 2048].fill(0);
    bytes[18 * 2048..18 * 2048 + 7].copy_from_slice(b"\xffCD001\x01");
    for (sector, records) in [
        (
            24,
            vec![
                record(&[0], 24, 2048, true),
                record(&[1], 24, 2048, true),
                record(&ucs2("Mes documents"), 25, 2048, true),
                record(&ucs2("Racine été.txt;1"), 22, 4, false),
            ],
        ),
        (
            25,
            vec![
                record(&[0], 25, 2048, true),
                record(&[1], 24, 2048, true),
                record(&ucs2("Lisez-moi.txt;1"), 23, 11, false),
            ],
        ),
    ] {
        let mut offset = sector * 2048;
        for record in records {
            bytes[offset..offset + record.len()].copy_from_slice(&record);
            offset += record.len();
        }
    }
    std::fs::write(&archive, bytes).unwrap();
    (folder, archive)
}

fn names(archive: &Path, internal: &str) -> Vec<String> {
    let mut names: Vec<_> =
        list_iso(archive, internal).unwrap().into_iter().map(|entry| entry.name).collect();
    names.sort();
    names
}

#[test]
fn joliet_names_replace_the_short_iso_9660_names() {
    let (_folder, archive) = joliet_image(b"%/E");
    assert_eq!(names(&archive, ""), ["Mes documents", "Racine été.txt"]);
    assert_eq!(names(&archive, "Mes documents"), ["Lisez-moi.txt"]);
    let full = tempfile::tempdir().unwrap();
    let budget = ArchiveBudget::new(ArchiveLimits::EXTRACTION);
    extract_iso(
        File::open(&archive).unwrap(),
        &ExtractionRoot::new(full.path()).unwrap(),
        None,
        &|| Ok(()),
        &budget,
    )
    .unwrap();
    assert_eq!(
        std::fs::read(full.path().join("Mes documents").join("Lisez-moi.txt")).unwrap(),
        b"ISO content"
    );
}

#[test]
fn a_supplementary_descriptor_without_ucs2_escape_keeps_the_iso_9660_tree() {
    let (_folder, archive) = joliet_image(b"   ");
    assert_eq!(names(&archive, ""), ["DOCS", "ROOT.TXT"]);
}
