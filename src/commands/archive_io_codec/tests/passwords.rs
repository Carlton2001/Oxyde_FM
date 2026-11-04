//! Encrypted ZIP and 7z through the framed codec: a missing or wrong password is a
//! typed refusal, the right one decodes.
use super::*;
use crate::models::error::ArchivePasswordFailure;
use crate::utils::archive_password::ArchiveSecret;

const CONTENT: &[u8] = b"secret payload";

fn aes_zip() -> Vec<u8> {
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default()
        .with_aes_encryption(zip::AesMode::Aes256, "correct horse");
    writer.add_directory("docs/", zip::write::SimpleFileOptions::default()).unwrap();
    writer.start_file("docs/note.txt", options).unwrap();
    writer.write_all(CONTENT).unwrap();
    writer.finish().unwrap().into_inner()
}

/// sevenz encrypts the header too, so even listing needs the password.
fn encrypted_seven() -> Vec<u8> {
    let folder = tempfile::tempdir().unwrap();
    std::fs::create_dir(folder.path().join("docs")).unwrap();
    std::fs::write(folder.path().join("docs/note.txt"), CONTENT).unwrap();
    sevenz_rust2::compress_encrypted(folder.path(), Cursor::new(Vec::new()), "correct horse".into())
        .unwrap()
        .into_inner()
}

fn decode_with(
    format: Format,
    bytes: Vec<u8>,
    selected: Option<&str>,
    password: Option<&str>,
) -> (Result<(), CommandError>, Broker) {
    let mut broker = Broker::new();
    let size = bytes.len() as u64;
    broker.sources.insert(0, bytes);
    broker.state = Some(ExtractionState::new(limits(1024), selected, true).unwrap());
    let connection = Arc::new(Mutex::new(broker));
    let result = run(
        connection.clone(),
        Operation::Decode {
            format,
            source_size: size,
            selected: selected.map(str::to_owned),
            stream_name: None,
            password: password.map(|value| ArchiveSecret::new(value.into()).unwrap()),
            limits: limits(1024),
        },
    );
    let broker = Arc::try_unwrap(connection).ok().unwrap().into_inner().unwrap();
    (result, broker)
}

fn refusal(result: Result<(), CommandError>) -> Option<ArchivePasswordFailure> {
    ArchivePasswordFailure::from_message(&result.unwrap_err().to_string())
}

#[test]
fn encrypted_archives_need_their_password_and_refuse_a_wrong_one() {
    for (format, bytes) in [(Format::Zip, aes_zip()), (Format::SevenZip, encrypted_seven())] {
        for selected in [None, Some("docs/note.txt")] {
            let (result, broker) = decode_with(format, bytes.clone(), selected, None);
            assert_eq!(refusal(result), Some(ArchivePasswordFailure::Required), "{format:?}");
            assert!(!broker.finished);
            let (result, broker) = decode_with(format, bytes.clone(), selected, Some("wrong"));
            assert_eq!(refusal(result), Some(ArchivePasswordFailure::Wrong), "{format:?}");
            assert!(!broker.finished);
            let (result, broker) = decode_with(format, bytes.clone(), selected, Some("correct horse"));
            result.unwrap();
            assert!(broker.finished);
            assert!(broker.extracted.values().any(|bytes| bytes == CONTENT), "{format:?}");
        }
    }
}

#[test]
fn a_refusal_travels_as_its_own_failure_kind() {
    for failure in [ArchivePasswordFailure::Required, ArchivePasswordFailure::Wrong] {
        let frame = super::super::super::archive_io_worker::error_frame(&failure.into());
        let error = crate::utils::archive_io_failure::failure(&frame);
        assert_eq!(ArchivePasswordFailure::from_message(&error.to_string()), Some(failure));
    }
}

fn encrypted(format: Format, password: &str) -> Vec<u8> {
    let mut broker = Broker::new();
    broker.metadata = vec![
        empty_entry(0, "docs".into(), true, 0),
        empty_entry(1, "docs/note.txt".into(), false, CONTENT.len() as u64),
    ];
    broker.sources.insert(0, Vec::new());
    broker.sources.insert(1, CONTENT.to_vec());
    let connection = Arc::new(Mutex::new(broker));
    run(
        connection.clone(),
        Operation::Encode {
            format,
            quality: Quality::Normal,
            entries: 2,
            output_limit: 1024 * 1024,
            password: ArchiveSecret::new(password.into()),
            limits: limits(1024),
        },
    )
    .unwrap();
    let broker = connection.lock().unwrap();
    broker.output.get_ref().clone()
}

#[test]
fn protected_archives_are_written_encrypted_and_read_back_with_their_password() {
    for format in [Format::Zip, Format::SevenZip] {
        let bytes = encrypted(format, "correct horse");
        assert!(!bytes.windows(CONTENT.len()).any(|window| window == CONTENT), "{format:?}");
        let (result, _) = decode_with(format, bytes.clone(), None, None);
        assert_eq!(refusal(result), Some(ArchivePasswordFailure::Required), "{format:?}");
        let (result, _) = decode_with(format, bytes.clone(), None, Some("wrong"));
        assert_eq!(refusal(result), Some(ArchivePasswordFailure::Wrong), "{format:?}");
        let (result, broker) = decode_with(format, bytes, None, Some("correct horse"));
        result.unwrap();
        assert!(broker.extracted.values().any(|bytes| bytes == CONTENT), "{format:?}");
    }
    // The 7z header is encrypted too: its names do not appear.
    let seven = encrypted(Format::SevenZip, "correct horse");
    assert!(!seven.windows(8).any(|window| window == b"n\0o\0t\0e\0"));
}
