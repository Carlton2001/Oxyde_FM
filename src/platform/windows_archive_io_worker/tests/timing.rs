use super::*;
use crate::utils::archive_io_protocol::{Format, Limits, Quality};

fn timing() -> Timing {
    Timing::new(&Operation::Encode {
        format: Format::Zip,
        quality: Quality::Fast,
        entries: 1,
        output_limit: 1024,
        limits: Limits {
            entries: 1,
            bytes: 1024,
            depth: 10,
            seconds: 10,
        },
    })
}

#[test]
fn traffic_separates_checkpoints_source_reads_and_output_rewrites() {
    let mut value = timing();
    value.frame(&Frame::new(Message::Checkpoint));
    value.frame(&Frame::new(Message::Read {
        source: 0,
        offset: 0,
        length: 64,
    }));
    value.frame(&Frame::data(Message::Write { offset: 0 }, vec![0; 32]));
    value.frame(&Frame::data(Message::Write { offset: 0 }, vec![0; 4]));
    assert_eq!(
        (value.frames, value.reads, value.writes, value.checkpoints),
        (4, 1, 2, 1)
    );
    assert_eq!((value.read_bytes, value.write_bytes), (64, 36));
}

#[test]
fn measured_handler_keeps_the_original_error_and_receives_the_frame_once() {
    let mut value = timing();
    let mut calls = 0;
    let error = value
        .handle(Frame::new(Message::Flush), &mut |frame| {
            calls += 1;
            assert_eq!(frame.message, Message::Flush);
            Err(io::Error::new(io::ErrorKind::PermissionDenied, "fixture"))
        })
        .unwrap_err();
    assert_eq!(calls, 1);
    assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
    assert_eq!(error.to_string(), "fixture");
}
