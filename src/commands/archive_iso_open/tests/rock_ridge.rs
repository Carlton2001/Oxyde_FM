use super::*;
use crate::commands::archive::test_iso_image;

/// A directory record whose system use area holds `system_use`.
fn record(name: &[u8], sector: u32, length: u32, directory: bool, system_use: &[u8]) -> Vec<u8> {
    let identifier = (33 + name.len() + 1) & !1;
    let mut bytes = vec![0; (identifier + system_use.len() + 1) & !1];
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
    bytes[identifier..identifier + system_use.len()].copy_from_slice(system_use);
    bytes
}

fn both(value: u32) -> Vec<u8> {
    [value.to_le_bytes(), value.to_be_bytes()].concat()
}

fn entry(signature: &[u8; 2], data: &[u8]) -> Vec<u8> {
    [&signature[..], &[(4 + data.len()) as u8, 1], data].concat()
}

/// `name` in NM entries of at most `chunk` bytes, each but the last marked as continued.
fn names(name: &str, chunk: usize) -> Vec<u8> {
    let parts: Vec<_> = name.as_bytes().chunks(chunk).collect();
    parts.iter().enumerate()
        .flat_map(|(index, part)| entry(b"NM", &[&[u8::from(index + 1 < parts.len())][..], part].concat()))
        .collect()
}

/// The fixture's tree (`DOCS/README.TXT`, `ROOT.TXT`) with Rock Ridge names, the README's
/// in a continuation area, and a Joliet tree that names them otherwise.
fn image() -> (tempfile::TempDir, PathBuf) {
    let folder = tempfile::tempdir().unwrap();
    let archive = folder.path().join("disc.iso");
    test_iso_image(&archive);
    let mut bytes = std::fs::read(&archive).unwrap();
    let sp = entry(b"SP", &[0xBE, 0xEF, 0]);
    let continued = names("Lisez-moi, version longue.txt", 255);
    let ce = entry(b"CE", &[both(26), both(0), both(continued.len() as u32)].concat());
    let joliet: Vec<u8> = "Joliet".encode_utf16().flat_map(u16::to_be_bytes).collect();
    // Sector 17 becomes a Joliet descriptor whose root, in sector 24, holds one file.
    let primary = bytes[16 * 2048..17 * 2048].to_vec();
    bytes[17 * 2048..18 * 2048].copy_from_slice(&primary);
    bytes[17 * 2048] = 2;
    bytes[17 * 2048 + 88..17 * 2048 + 91].copy_from_slice(b"%/E");
    bytes[17 * 2048 + 156..17 * 2048 + 190].copy_from_slice(&record(&[0], 24, 2048, true, &[]));
    bytes[18 * 2048..19 * 2048].fill(0);
    bytes[18 * 2048..18 * 2048 + 7].copy_from_slice(b"\xffCD001\x01");
    for (sector, records) in [
        (
            20,
            vec![
                record(&[0], 20, 2048, true, &sp),
                record(&[1], 20, 2048, true, &[]),
                record(b"DOCS", 21, 2048, true, &names("Mes Documents", 5)),
                record(b"ROOT.TXT;1", 22, 4, false, &names("racine.txt;1", 255)),
            ],
        ),
        (
            21,
            vec![
                record(&[0], 21, 2048, true, &[]),
                record(&[1], 20, 2048, true, &[]),
                record(b"README.TXT;1", 23, 11, false, &ce),
            ],
        ),
        (
            24,
            vec![
                record(&[0], 24, 2048, true, &[]),
                record(&[1], 24, 2048, true, &[]),
                record(&joliet, 22, 4, false, &[]),
            ],
        ),
    ] {
        bytes[sector * 2048..(sector + 1) * 2048].fill(0);
        let mut offset = sector * 2048;
        for record in records {
            bytes[offset..offset + record.len()].copy_from_slice(&record);
            offset += record.len();
        }
    }
    bytes[26 * 2048..26 * 2048 + continued.len()].copy_from_slice(&continued);
    std::fs::write(&archive, bytes).unwrap();
    (folder, archive)
}

fn listed(archive: &Path, internal: &str) -> Vec<String> {
    let mut names: Vec<_> = list_iso(archive, internal).unwrap().into_iter().map(|entry| entry.name).collect();
    names.sort();
    names
}

#[test]
fn rock_ridge_names_come_before_joliet_and_keep_their_own_spelling() {
    let (_folder, archive) = image();
    assert_eq!(listed(&archive, ""), ["Mes Documents", "racine.txt;1"]);
    assert_eq!(listed(&archive, "Mes Documents"), ["Lisez-moi, version longue.txt"]);
    let full = tempfile::tempdir().unwrap();
    let budget = ArchiveBudget::new(ArchiveLimits::EXTRACTION);
    extract_iso(File::open(&archive).unwrap(), &ExtractionRoot::new(full.path()).unwrap(), None, &|| Ok(()), &budget)
        .unwrap();
    assert_eq!(
        std::fs::read(full.path().join("Mes Documents").join("Lisez-moi, version longue.txt")).unwrap(),
        b"ISO content"
    );
}
