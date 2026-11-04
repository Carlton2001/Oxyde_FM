use super::*;

#[test]
fn frames_carry_a_password_or_none_and_refuse_oversized_ones() {
    let secret = ArchiveSecret::new("pässwörd".into()).unwrap();
    let mut frame = Vec::new();
    send(&mut frame, Some(&secret)).unwrap();
    assert_eq!(read_frame(&mut frame.as_slice()).unwrap(), Some(secret));
    let mut empty = Vec::new();
    send(&mut empty, None).unwrap();
    assert_eq!(empty, [0, 0]);
    assert_eq!(read_frame(&mut empty.as_slice()).unwrap(), None);
    let mut long = ((MAX_BYTES + 1) as u16).to_le_bytes().to_vec();
    long.resize(2 + MAX_BYTES + 1, b'a');
    assert!(read_frame(&mut long.as_slice()).is_err());
    assert!(read_frame(&mut [2, 0, 0xff, 0xfe].as_slice()).is_err());
    assert!(ArchiveSecret::new(String::new()).is_none());
}

#[test]
fn debug_and_logs_never_show_the_value() {
    let secret = ArchiveSecret::new("hunter2".into()).unwrap();
    assert!(!format!("{secret:?}").contains("hunter2"));
    let json = serde_json::to_string(&secret).unwrap();
    assert_eq!(serde_json::from_str::<ArchiveSecret>(&json).unwrap(), secret);
    assert!(serde_json::from_str::<ArchiveSecret>("\"\"").is_err());
}

#[test]
fn passwords_are_remembered_per_archive() {
    let archive = Path::new("/tmp/oxyde-password-test.zip");
    assert!(for_archive(archive).is_none());
    remember(archive, ArchiveSecret::new("one".into()).unwrap());
    assert_eq!(for_archive(archive).unwrap().expose(), "one");
    assert!(for_archive(Path::new("/tmp/other.zip")).is_none());
    forget(archive);
    assert!(for_archive(archive).is_none());
}
