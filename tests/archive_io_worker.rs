#![cfg(target_os = "windows")]
use oxyde_lib::utils;
use oxyde_lib::platform::worker_test_support::{
    archive_io as windows_archive_io_worker,
    archive_listing as windows_archive_worker,
    preview as windows_preview_worker,
};
#[path = "support/iso.rs"]
mod iso;
#[path = "support/native_process.rs"]
mod native_process;
#[path = "support/archive_transport.rs"]
mod transport_tests;
use native_process::native_probe;
use oxyde_lib::utils::archive_io_state::ExtractionState;
use std::io::{self, Cursor, Seek, SeekFrom, Write};
use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use utils::archive_io_protocol::{
    invalid, Control, Entry, Format, Frame, Limits, Message, Operation, Quality,
};

fn limits() -> Limits {
    Limits {
        entries: 10,
        bytes: 1024 * 1024,
        depth: 64,
        seconds: 120,
    }
}
fn entry(size: u64) -> Entry {
    Entry {
        id: 0,
        name: "école_日本語.txt".into(),
        directory: false,
        size,
        mode: 0o644,
        modified: None,
        created: None,
        accessed: None,
    }
}
fn encode_operation(format: Format, quality: Quality) -> Operation {
    Operation::Encode {
        format,
        quality,
        entries: 1,
        output_limit: 2 * 1024 * 1024,
        limits: limits(),
    }
}
fn encoded(format: Format, quality: Quality) -> Vec<u8> {
    let content: Vec<u8> = (0..(256 * 1024 + 13))
        .map(|i| ((i * 29 + i / 257) % 256) as u8)
        .collect();
    let mut output = Cursor::new(Vec::new());
    let stats = windows_archive_io_worker::run_in(
        Path::new(env!("CARGO_BIN_EXE_oxyde-archive-io-worker")),
        encode_operation(format, quality),
        &|| Ok(Control::default()),
        &mut |frame| match frame.message {
            Message::Source { id: 0 } => Ok(Frame::new(Message::Entry {
                entry: entry(content.len() as u64),
            })),
            Message::Read {
                source: 0,
                offset,
                length,
            } => {
                let end = offset
                    .checked_add(length as u64)
                    .filter(|end| *end <= content.len() as u64)
                    .ok_or_else(invalid)?;
                Ok(Frame::data(
                    Message::Bytes { source: 0, offset },
                    content[offset as usize..end as usize].to_vec(),
                ))
            }
            Message::Write { offset } => {
                if offset
                    .checked_add(frame.data.len() as u64)
                    .is_none_or(|end| end > 2 * 1024 * 1024)
                {
                    return Err(invalid());
                }
                output.seek(SeekFrom::Start(offset))?;
                output.write_all(&frame.data)?;
                Ok(Frame::new(Message::Ack {
                    pause: false,
                    turbo: false,
                }))
            }
            Message::Begin { id: 0 }
            | Message::End { id: 0 }
            | Message::Flush
            | Message::Finished => Ok(Frame::new(Message::Ack {
                pause: false,
                turbo: false,
            })),
            _ => Err(invalid()),
        },
        Duration::from_secs(30),
    )
    .unwrap();
    assert!(stats.peak_memory > 0 && stats.peak_memory <= 1024 * 1024 * 1024);
    eprintln!(
        "{format:?} {quality:?}: {} committed bytes",
        stats.peak_memory
    );
    let bytes = output.into_inner();
    assert!(bytes.len() as u64 <= content.len() as u64 + 64 * 1024 * 1024 + 4096);
    let mut state = ExtractionState::new(limits(), Some("école_日本語.txt"), true).unwrap();
    let mut extracted = Vec::new();
    let operation = Operation::Decode {
        format,
        source_size: bytes.len() as u64,
        selected: Some("école_日本語.txt".into()),
        limits: limits(),
    };
    windows_archive_io_worker::run_in(
        Path::new(env!("CARGO_BIN_EXE_oxyde-archive-io-worker")),
        operation,
        &|| Ok(Control::default()),
        &mut |frame| {
            if let Message::Read {
                source: 0,
                offset,
                length,
            } = frame.message
            {
                let end = offset
                    .checked_add(length as u64)
                    .filter(|end| *end <= bytes.len() as u64)
                    .ok_or_else(invalid)?;
                return Ok(Frame::data(
                    Message::Bytes { source: 0, offset },
                    bytes[offset as usize..end as usize].to_vec(),
                ));
            }
            if matches!(frame.message, Message::Decoded { .. }) {
                return Ok(Frame::new(Message::Ack {
                    pause: false,
                    turbo: false,
                }));
            }
            state.accept(&frame)?;
            if matches!(frame.message, Message::Chunk { .. }) {
                extracted.extend_from_slice(&frame.data);
            }
            Ok(Frame::new(Message::Ack {
                pause: false,
                turbo: false,
            }))
        },
        Duration::from_secs(30),
    )
    .unwrap();
    assert!(state.finished());
    assert_eq!(extracted, content);
    bytes
}

#[test]
fn real_worker_encodes_and_decodes_all_writable_formats_under_the_memory_job() {
    for format in [Format::Zip, Format::SevenZip, Format::Tar, Format::TarZst] {
        encoded(format, Quality::Best);
    }
}

fn decode_fixture(format: Format, bytes: Vec<u8>, selected: &str, expected: &[u8]) {
    let mut state = ExtractionState::new(limits(), Some(selected), true).unwrap();
    let mut extracted = Vec::new();
    windows_archive_io_worker::run_in(
        Path::new(env!("CARGO_BIN_EXE_oxyde-archive-io-worker")),
        Operation::Decode {
            format,
            source_size: bytes.len() as u64,
            selected: Some(selected.into()),
            limits: limits(),
        },
        &|| Ok(Control::default()),
        &mut |frame| {
            if let Message::Read {
                source: 0,
                offset,
                length,
            } = frame.message
            {
                let end = offset
                    .checked_add(length as u64)
                    .filter(|n| *n <= bytes.len() as u64)
                    .ok_or_else(invalid)?;
                return Ok(Frame::data(
                    Message::Bytes { source: 0, offset },
                    bytes[offset as usize..end as usize].to_vec(),
                ));
            }
            if !matches!(frame.message, Message::Decoded { .. }) {
                state.accept(&frame)?;
                if matches!(frame.message, Message::Chunk { .. }) {
                    extracted.extend_from_slice(&frame.data);
                }
            }
            Ok(Frame::new(Message::Ack {
                pause: false,
                turbo: false,
            }))
        },
        Duration::from_secs(30),
    )
    .unwrap();
    assert!(state.finished());
    assert_eq!(extracted, expected);
}

#[test]
fn real_worker_decodes_gzip_xz_bzip2_and_iso_under_lpac() {
    let tar = encoded(Format::Tar, Quality::Fast);
    let expected: Vec<u8> = (0..(256 * 1024 + 13))
        .map(|i| ((i * 29 + i / 257) % 256) as u8)
        .collect();
    let mut gzip = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
    gzip.write_all(&tar).unwrap();
    decode_fixture(
        Format::TarGz,
        gzip.finish().unwrap(),
        "école_日本語.txt",
        &expected,
    );
    let mut xz = xz2::write::XzEncoder::new(Vec::new(), 1);
    xz.write_all(&tar).unwrap();
    decode_fixture(
        Format::TarXz,
        xz.finish().unwrap(),
        "école_日本語.txt",
        &expected,
    );
    let mut bzip = bzip2::write::BzEncoder::new(Vec::new(), bzip2::Compression::fast());
    bzip.write_all(&tar).unwrap();
    decode_fixture(
        Format::TarBz2,
        bzip.finish().unwrap(),
        "école_日本語.txt",
        &expected,
    );
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("fixture.iso");
    iso::iso_fixture(&path);
    decode_fixture(
        Format::Iso,
        std::fs::read(path).unwrap(),
        "DOCS/README.TXT",
        b"ISO content",
    );
}

#[test]
fn confirmed_pause_excludes_elapsed_time_and_produces_no_effects() {
    let bytes = encoded(Format::Zip, Quality::Fast);
    let pause = AtomicBool::new(false);
    let started = Instant::now();
    let mut first = None;
    let mut effects = 0;
    let controls = || {
        Ok(Control {
            paused: pause.load(Ordering::Relaxed)
                && started.elapsed() < Duration::from_millis(1800),
            ..Control::default()
        })
    };
    let operation = Operation::Decode {
        format: Format::Zip,
        source_size: bytes.len() as u64,
        selected: Some("école_日本語.txt".into()),
        limits: limits(),
    };
    let mut state = ExtractionState::new(limits(), Some("école_日本語.txt"), true).unwrap();
    windows_archive_io_worker::run_in(
        Path::new(env!("CARGO_BIN_EXE_oxyde-archive-io-worker")),
        operation,
        &controls,
        &mut |frame| {
            effects += 1;
            if first.is_none() {
                first = Some(started.elapsed());
                pause.store(true, Ordering::Relaxed);
            } else if pause.load(Ordering::Relaxed) {
                assert!(started.elapsed() >= Duration::from_millis(1800));
            }
            if let Message::Read {
                source: 0,
                offset,
                length,
            } = frame.message
            {
                return Ok(Frame::data(
                    Message::Bytes { source: 0, offset },
                    bytes[offset as usize..offset as usize + length as usize].to_vec(),
                ));
            }
            state.accept(&frame)?;
            Ok(Frame::new(Message::Ack {
                pause: false,
                turbo: false,
            }))
        },
        Duration::from_millis(1500),
    )
    .unwrap();
    assert!(started.elapsed() >= Duration::from_millis(1800));
    assert!(effects > 1 && state.finished());
}

#[test]
fn listing_search_and_preview_advance_while_archive_io_is_confirmed_paused() {
    use std::sync::{atomic::AtomicUsize, mpsc, Arc};
    let directory = tempfile::tempdir().unwrap();
    let bytes = encoded(Format::Zip, Quality::Fast);
    let archive = directory.path().join("source.zip");
    std::fs::write(&archive, &bytes).unwrap();
    let image = directory.path().join("source.png");
    image::RgbaImage::from_pixel(2, 1, image::Rgba([11, 22, 33, 255]))
        .save(&image)
        .unwrap();
    let paused = Arc::new(AtomicBool::new(true));
    let effects = Arc::new(AtomicUsize::new(0));
    let pause_control = paused.clone();
    let observed = effects.clone();
    let (ready, waiting) = mpsc::channel();
    let child = std::thread::spawn(move || {
        let started = Instant::now();
        let sent = AtomicBool::new(false);
        let mut state = ExtractionState::new(limits(), Some("école_日本語.txt"), true).unwrap();
        let result = windows_archive_io_worker::run_in(
            Path::new(env!("CARGO_BIN_EXE_oxyde-archive-io-worker")),
            Operation::Decode {
                format: Format::Zip,
                source_size: bytes.len() as u64,
                selected: Some("école_日本語.txt".into()),
                limits: limits(),
            },
            &|| {
                // Surviving longer than the active deadline proves that pause was
                // acknowledged; a worker that never acknowledged would have exited.
                if started.elapsed() >= Duration::from_secs(3)
                    && !sent.swap(true, Ordering::Relaxed)
                {
                    let _ = ready.send(());
                }
                Ok(Control {
                    paused: pause_control.load(Ordering::Relaxed),
                    ..Control::default()
                })
            },
            &mut |frame| {
                assert!(!pause_control.load(Ordering::Relaxed));
                observed.fetch_add(1, Ordering::Relaxed);
                if let Message::Read {
                    source: 0,
                    offset,
                    length,
                } = frame.message
                {
                    return Ok(Frame::data(
                        Message::Bytes { source: 0, offset },
                        bytes[offset as usize..offset as usize + length as usize].to_vec(),
                    ));
                }
                state.accept(&frame)?;
                Ok(Frame::new(Message::Ack {
                    pause: false,
                    turbo: false,
                }))
            },
            Duration::from_secs(2),
        );
        result.unwrap();
        assert!(state.finished());
    });
    let run = || {
        waiting.recv_timeout(Duration::from_secs(12)).unwrap();
        assert_eq!(effects.load(Ordering::Relaxed), 0);
        let executable = Path::new(env!("CARGO_BIN_EXE_oxyde-archive-list-worker"));
        assert_eq!(
            windows_archive_worker::run_in(executable, &archive, "", Duration::from_secs(15))
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            windows_archive_worker::index_in(
                executable,
                &archive,
                Duration::from_secs(15),
                &|| Ok(())
            )
            .unwrap()
            .len(),
            1
        );
        let reply =
            windows_preview_worker::run_image_in(Path::new(env!("CARGO_BIN_EXE_oxyde-image-worker")), &image, Duration::from_secs(15))
                .unwrap();
        let utils::preview_protocol::Reply::Images { frames, limited, .. } =
            utils::preview_protocol::decode(&reply).unwrap()
        else {
            panic!("Expected image");
        };
        assert!(!limited);
        assert_eq!(
            frames[0].buffer().as_raw(),
            &[33, 22, 11, 255, 33, 22, 11, 255]
        );
        assert_eq!(effects.load(Ordering::Relaxed), 0);
    };
    // Release even when a separate worker fails, so this fixture owns no orphan.
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(run));
    paused.store(false, Ordering::Relaxed);
    child.join().unwrap();
    result.unwrap();
}

fn noop(_: Frame) -> io::Result<Frame> {
    Ok(Frame::new(Message::Ack {
        pause: false,
        turbo: false,
    }))
}

fn adverse_probe(mode: &str) -> Command {
    let mut bytes = Vec::new();
    utils::archive_io_protocol::Wire::new([7; 16])
        .write(&mut bytes, &Frame::new(Message::Source { id: 0 }))
        .unwrap();
    let mut command = native_probe::command(mode);
    command.arg(bytes.iter().map(|b| format!("{b:02x}")).collect::<String>());
    command
}

#[test]
fn deadline_and_cancellation_remain_live_when_child_does_not_read_its_input() {
    // Compilation is outside the timed cancellation window, otherwise a slow
    // compiler could cancel the session before the child was ever launched.
    native_probe::executable();
    let operation = Operation::Decode {
        format: Format::Zip,
        source_size: 0,
        selected: Some("x".repeat(32 * 1024)),
        limits: limits(),
    };
    for cancel in [false, true] {
        let mut ready = false;
        let mut replies = 0;
        let mut messages = vec![Message::Source { id: 0 }];
        messages.extend((0..64).map(|index| Message::Read {
            source: 0,
            offset: index * utils::archive_io_protocol::MAX_BLOCK as u64,
            length: utils::archive_io_protocol::MAX_BLOCK as u32,
        }));
        let started = Instant::now();
        let ready_at = std::cell::Cell::new(None::<Instant>);
        let controls = || {
            Ok(Control {
                cancelled: cancel && ready_at.get().is_some_and(|time| time.elapsed() >= Duration::from_millis(250)),
                ..Control::default()
            })
        };
        let error = windows_archive_io_worker::supervise(
            fake_frames(&messages),
            [7; 16],
            operation.clone(),
            &controls,
            &mut |frame| match frame.message {
                Message::Source { id: 0 } => {
                    ready = true;
                    ready_at.set(Some(Instant::now()));
                    noop(frame)
                }
                Message::Read {
                    source: 0,
                    offset,
                    length,
                } => {
                    replies += 1;
                    Ok(Frame::data(
                        Message::Bytes { source: 0, offset },
                        vec![0; length as usize],
                    ))
                }
                _ => panic!("unexpected native probe request"),
            },
            if cancel {
                Duration::from_secs(10)
            } else {
                Duration::from_millis(250)
            },
            1024 * 1024 * 1024,
        )
        .err()
        .unwrap();
        assert!(ready, "native adverse child never reported readiness");
        // More than the requested 2MiB pipe capacity was produced, yet the
        // child's finite request stream was not drained: a writer is blocked.
        assert!(
            (9..64).contains(&replies),
            "blocked-write witness missing: {replies}"
        );
        assert_eq!(
            error.kind(),
            if cancel {
                io::ErrorKind::Interrupted
            } else {
                io::ErrorKind::TimedOut
            }
        );
        assert!(started.elapsed() >= Duration::from_millis(200));
        assert!(started.elapsed() < Duration::from_secs(4));
    }
    encoded(Format::Zip, Quality::Fast);
}

#[test]
fn crash_malformed_output_and_memory_failure_do_not_report_success() {
    for (mode, memory) in [
        ("crash", 1024 * 1024 * 1024),
        ("malformed", 1024 * 1024 * 1024),
        ("memory", 64 * 1024 * 1024),
    ] {
        let mut ready = None;
        let command = adverse_probe(mode);
        assert!(windows_archive_io_worker::supervise(
            command,
            [7; 16],
            encode_operation(Format::Zip, Quality::Fast),
            &|| Ok(Control::default()),
            &mut |frame| {
                assert_eq!(frame.message, Message::Source { id: 0 });
                ready = Some(Instant::now());
                noop(frame)
            },
            Duration::from_secs(3),
            memory
        )
        .is_err());
        assert!(
            ready.is_some(),
            "native adverse child never reported readiness: {mode}"
        );
        assert!(ready.unwrap().elapsed() < Duration::from_secs(5));
    }
}

#[test]
fn cancelled_session_does_not_launch_even_with_a_missing_executable() {
    let error = windows_archive_io_worker::run_in(
        Path::new("C:/missing-archive-io-worker-test.exe"),
        encode_operation(Format::Zip, Quality::Normal),
        &|| {
            Ok(Control {
                cancelled: true,
                ..Control::default()
            })
        },
        &mut noop,
        Duration::from_secs(30),
    )
    .err()
    .unwrap();
    assert_eq!(error.kind(), io::ErrorKind::Interrupted);
}

fn fake_frames(messages: &[Message]) -> Command {
    let mut wire = utils::archive_io_protocol::Wire::new([7; 16]);
    let mut bytes = Vec::new();
    for message in messages {
        wire.write(&mut bytes, &Frame::new(message.clone()))
            .unwrap();
    }
    let bytes = bytes.iter().map(|b| format!("{b:02x}")).collect::<String>();
    let mut command = native_probe::command("frames");
    command.arg(bytes);
    command
}

#[test]
fn unacknowledged_pause_never_disables_the_deadline() {
    let started = Instant::now();
    let error = windows_archive_io_worker::supervise(
        fake_frames(&[Message::Checkpoint]),
        [7; 16],
        encode_operation(Format::Zip, Quality::Fast),
        &|| {
            Ok(Control {
                paused: true,
                ..Control::default()
            })
        },
        &mut |_| panic!("effects while pause requested"),
        Duration::from_millis(1200),
        1024 * 1024 * 1024,
    )
    .err()
    .unwrap();
    assert_eq!(error.kind(), io::ErrorKind::TimedOut);
    assert!(started.elapsed() >= Duration::from_millis(200));
    assert!(started.elapsed() < Duration::from_secs(4));
}

#[test]
fn data_after_terminal_result_is_rejected() {
    let mut terminal_seen = false;
    assert!(windows_archive_io_worker::supervise(
        fake_frames(&[Message::Finished, Message::Checkpoint]),
        [7; 16],
        encode_operation(Format::Zip, Quality::Fast),
        &|| Ok(Control::default()),
        &mut |frame| {
            assert_eq!(frame.message, Message::Finished);
            terminal_seen = true;
            noop(frame)
        },
        Duration::from_secs(3),
        1024 * 1024 * 1024
    )
    .is_err());
    assert!(
        terminal_seen,
        "terminal frame from native child was never observed"
    );
}

#[test]
fn forged_sessions_and_replayed_sequences_are_rejected_after_native_readiness() {
    for replay in [false, true] {
        let mut bytes = Vec::new();
        let mut wire = utils::archive_io_protocol::Wire::new([7; 16]);
        wire.write(&mut bytes, &Frame::new(Message::Source { id: 0 }))
            .unwrap();
        let mut bad = Vec::new();
        if replay {
            utils::archive_io_protocol::Wire::new([7; 16])
                .write(&mut bad, &Frame::new(Message::Checkpoint))
                .unwrap();
        } else {
            wire.write(&mut bad, &Frame::new(Message::Checkpoint))
                .unwrap();
            bad[8] ^= 1;
        }
        bytes.extend_from_slice(&bad);
        let mut command = native_probe::command("frames");
        command.arg(bytes.iter().map(|b| format!("{b:02x}")).collect::<String>());
        let mut ready = false;
        let error = windows_archive_io_worker::supervise(
            command,
            [7; 16],
            encode_operation(Format::Zip, Quality::Fast),
            &|| Ok(Control::default()),
            &mut |frame| {
                assert_eq!(frame.message, Message::Source { id: 0 });
                ready = true;
                noop(frame)
            },
            Duration::from_secs(3),
            1024 * 1024 * 1024,
        )
        .err()
        .unwrap();
        assert!(ready, "native child readiness missing");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    }
}

#[test]
fn cancellation_remains_live_during_a_confirmed_pause() {
    let bytes = encoded(Format::Zip, Quality::Fast);
    let started = Instant::now();
    let error = windows_archive_io_worker::run_in(
        Path::new(env!("CARGO_BIN_EXE_oxyde-archive-io-worker")),
        Operation::Decode {
            format: Format::Zip,
            source_size: bytes.len() as u64,
            selected: None,
            limits: limits(),
        },
        &|| {
            Ok(Control {
                paused: true,
                cancelled: started.elapsed() >= Duration::from_millis(350),
                ..Control::default()
            })
        },
        &mut |_| panic!("effect while paused"),
        Duration::from_secs(10),
    )
    .err()
    .unwrap();
    assert_eq!(error.kind(), io::ErrorKind::Interrupted);
    assert!(started.elapsed() >= Duration::from_millis(200));
    assert!(started.elapsed() < Duration::from_secs(4));
}
