use super::*;

fn reply(result: Result<Vec<FileEntry>, CommandError>) -> Result<Vec<FileEntry>, CommandError> {
    let mut bytes = Vec::new();
    encode(result, &mut bytes).unwrap();
    decode(&bytes, Path::new("/archive.zip"), "")
}

#[test]
fn a_refusal_travels_as_its_own_status() {
    for failure in [ArchivePasswordFailure::Required, ArchivePasswordFailure::Wrong] {
        let error = reply(Err(failure.into())).unwrap_err();
        assert!(matches!(error, CommandError::ArchivePassword(received) if received == failure));
        assert_eq!(ArchivePasswordFailure::from_message(&error.to_string()), Some(failure));
    }
}

#[test]
fn a_refusal_quoted_in_a_worker_text_is_not_one() {
    let name = format!("../{}", ArchivePasswordFailure::Required);
    let error = reply(Err(CommandError::ArchiveError(format!("Unsafe archive entry path: {name}")))).unwrap_err();
    assert_eq!(ArchivePasswordFailure::from_message(&error.to_string()), None);
    assert!(error.to_string().contains("Unsafe archive entry path: ../Archive password required (archive-password-required]"));
}

#[test]
fn a_refusal_status_carries_no_text() {
    let mut bytes = Vec::new();
    encode(Err(ArchivePasswordFailure::Wrong.into()), &mut bytes).unwrap();
    bytes[9..13].copy_from_slice(&1u32.to_le_bytes());
    bytes.push(b'x');
    assert!(matches!(decode(&bytes, Path::new("/archive.zip"), ""), Err(CommandError::ArchiveError(_))));
}
