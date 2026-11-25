use super::*;

fn bytes(files: &mut VolumeFiles, source: u64, length: u32) -> Vec<u8> {
    let frame = files.serve(source, 0, length).unwrap();
    assert!(matches!(frame.message, Message::Bytes { .. }));
    frame.data
}

#[test]
fn the_checked_volume_is_read_even_once_replaced_by_name() {
    let directory = tempfile::tempdir().unwrap();
    let first = directory.path().join("film.part1.rar");
    std::fs::write(&first, b"checked").unwrap();
    std::fs::write(directory.path().join("film.part2.rar"), b"second").unwrap();
    let source = File::open(&first).unwrap();
    let replacement = directory.path().join("other");
    std::fs::write(&replacement, b"swapped").unwrap();
    std::fs::rename(&replacement, &first).unwrap();
    let mut files = VolumeFiles::open(&first, source).unwrap();
    assert_eq!(bytes(&mut files, 0, 7), b"checked");
    assert_eq!(bytes(&mut files, 1, 6), b"second");
}
