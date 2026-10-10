use std::{fs, path::Path};
pub fn block_crc_fixture_with_method(path: &Path, corrupt_tail: bool, method: u8) {
    fn crc32(bytes: &[u8]) -> u32 {
        let mut crc = u32::MAX;
        for byte in bytes {
            crc ^= *byte as u32;
            for _ in 0..8 {
                crc = (crc >> 1) ^ (0xedb88320 & 0u32.wrapping_sub(crc & 1));
            }
        }
        !crc
    }
    assert_eq!(crc32(b"123456789"), 0xcbf43926);
    let content = b"safetail";
    let mut header = vec![
        1, 4, // Header and MainStreamsInfo.
        6, 0, 1, 9, 8, 0, // One COPY pack stream of eight bytes.
        7, 11, 1, 0, 1, 1, method, 12, 8, 10, 1, // One block, with a CRC.
    ];
    header.extend(crc32(content).to_le_bytes());
    header.extend([0, 8, 13, 2, 9, 4, 10, 0, 0, 0, 0]); // Two sub-streams, neither with a CRC.
    let mut names = vec![0];
    for name in ["selected.txt", "tail.txt"] {
        for unit in name.encode_utf16().chain([0]) {
            names.extend(unit.to_le_bytes());
        }
    }
    header.extend([5, 2, 17, names.len() as u8]);
    header.extend(names);
    header.extend([0, 0]);
    let mut bytes = vec![0u8; 32];
    bytes[..8].copy_from_slice(b"7z\xbc\xaf\x27\x1c\x00\x04");
    bytes[12..20].copy_from_slice(&8u64.to_le_bytes());
    bytes[20..28].copy_from_slice(&(header.len() as u64).to_le_bytes());
    bytes[28..32].copy_from_slice(&crc32(&header).to_le_bytes());
    let start_crc = crc32(&bytes[12..32]);
    bytes[8..12].copy_from_slice(&start_crc.to_le_bytes());
    bytes.extend(content);
    if corrupt_tail {
        bytes[36] ^= 0x55;
    }
    bytes.extend(header);
    fs::write(path, bytes).unwrap();
}
