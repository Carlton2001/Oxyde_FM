use super::*;
use crate::utils::archive_io_protocol::{Message, HEADER};

fn encoded() -> Vec<u8> {
    let mut bytes = Vec::new();
    Wire::new([9; 16])
        .write(&mut bytes, &Frame::new(Message::Checkpoint))
        .unwrap();
    bytes
}

struct Fragmented(io::Cursor<Vec<u8>>);
impl Read for Fragmented {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        let count = bytes.len().min(1);
        self.0.read(&mut bytes[..count])
    }
}

#[test]
fn fragmented_frame_is_validated_and_clean_eof_is_distinct_from_truncation() {
    let bytes = encoded();
    let mut input = Fragmented(io::Cursor::new(bytes.clone()));
    let mut wire = Wire::new([9; 16]);
    assert_eq!(
        next_frame(&mut input, &mut wire).unwrap(),
        Some(Frame::new(Message::Checkpoint))
    );
    assert_eq!(next_frame(&mut input, &mut wire).unwrap(), None);
    for end in [1, HEADER - 1, HEADER, bytes.len() - 1] {
        let error =
            next_frame(&mut io::Cursor::new(&bytes[..end]), &mut Wire::new([9; 16])).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::UnexpectedEof);
    }
}

#[test]
fn reader_rejects_wrong_session_replayed_sequence_and_oversized_header() {
    let bytes = encoded();
    assert!(next_frame(&mut io::Cursor::new(&bytes), &mut Wire::new([8; 16])).is_err());
    let mut repeated = bytes.clone();
    repeated.extend_from_slice(&bytes);
    let mut reader = io::Cursor::new(repeated);
    let mut wire = Wire::new([9; 16]);
    next_frame(&mut reader, &mut wire).unwrap();
    assert!(next_frame(&mut reader, &mut wire).is_err());
    let mut header = bytes[..HEADER].to_vec();
    header[36..40].copy_from_slice(&u32::MAX.to_le_bytes());
    let error = next_frame(&mut io::Cursor::new(header), &mut Wire::new([9; 16])).unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::InvalidData);
}

#[test]
fn frames_and_completion_wake_the_same_queue_and_disconnect_releases_helpers() {
    let pipe = Pipe::start(io::Cursor::new(encoded()), io::sink(), [9; 16]).unwrap();
    assert!(matches!(
        pipe.wait(Duration::from_secs(2)).unwrap(),
        Event::Frame(_)
    ));
    pipe.read_next().unwrap();
    assert!(matches!(
        pipe.wait(Duration::from_secs(2)).unwrap(),
        Event::Closed(Ok(()))
    ));
    pipe.send(vec![1, 2, 3]).unwrap();
    assert!(matches!(
        pipe.wait(Duration::from_secs(2)).unwrap(),
        Event::Written(Ok(()))
    ));
    for helper in pipe.close() {
        helper.join().unwrap();
    }
}

#[test]
fn prefetched_frames_cannot_starve_write_completion_without_read_credit() {
    let mut bytes = Vec::new();
    let mut wire = Wire::new([9; 16]);
    for _ in 0..64 {
        wire.write(&mut bytes, &Frame::new(Message::Checkpoint))
            .unwrap();
    }
    let pipe = Pipe::start(io::Cursor::new(bytes), io::sink(), [9; 16]).unwrap();
    assert!(matches!(
        pipe.wait(Duration::from_secs(2)).unwrap(),
        Event::Frame(_)
    ));
    pipe.send(vec![0; 32]).unwrap();
    assert!(matches!(
        pipe.wait(Duration::from_secs(2)).unwrap(),
        Event::Written(Ok(()))
    ));
    assert!(matches!(
        pipe.wait(Duration::from_millis(1)),
        Err(RecvTimeoutError::Timeout)
    ));
    pipe.read_next().unwrap();
    assert!(matches!(
        pipe.wait(Duration::from_secs(2)).unwrap(),
        Event::Frame(_)
    ));
    for helper in pipe.close() {
        helper.join().unwrap();
    }
}
