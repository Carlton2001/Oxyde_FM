#![cfg(target_os = "windows")]
mod utils {
    pub use oxyde_lib::utils::{archive_io_protocol, preview_protocol};
}
mod models {
    pub use oxyde_lib::models::error;
}
mod commands {
    pub use oxyde_lib::commands::archive_io_worker;
}
#[allow(dead_code)]
#[path = "../src/platform/windows_archive_io_worker.rs"]
mod windows_archive_io_worker;
#[allow(dead_code)]
#[path = "../src/platform/windows_preview_worker.rs"]
mod windows_preview_worker;
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
        Path::new(env!("CARGO_BIN_EXE_oxyde")),
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
        Path::new(env!("CARGO_BIN_EXE_oxyde")),
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
        Path::new(env!("CARGO_BIN_EXE_oxyde")),
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

fn powershell(script: &str) -> Command {
    let executable = oxyde_lib::platform::platform()
        .system_executable(oxyde_lib::platform::SystemExecutable::PowerShell)
        .unwrap();
    let mut command = Command::new(executable);
    command.args([
        "-NoLogo",
        "-NoProfile",
        "-NonInteractive",
        "-Command",
        script,
    ]);
    command
}
fn noop(_: Frame) -> io::Result<Frame> {
    Ok(Frame::new(Message::Ack {
        pause: false,
        turbo: false,
    }))
}

#[test]
fn deadline_and_cancellation_remain_live_when_child_does_not_read_its_input() {
    let operation = Operation::Decode {
        format: Format::Zip,
        source_size: 0,
        selected: Some("x".repeat(32 * 1024)),
        limits: limits(),
    };
    for cancel in [false, true] {
        let started = Instant::now();
        let controls = || {
            Ok(Control {
                cancelled: cancel && started.elapsed() >= Duration::from_millis(250),
                ..Control::default()
            })
        };
        let error = windows_archive_io_worker::supervise(
            powershell(
                "$g=[Console]::OpenStandardInput().ReadByte(); [Threading.Thread]::Sleep(10000)",
            ),
            [7; 16],
            operation.clone(),
            &controls,
            &mut noop,
            if cancel {
                Duration::from_secs(10)
            } else {
                Duration::from_millis(250)
            },
            1024 * 1024 * 1024,
        )
        .err()
        .unwrap();
        assert_eq!(
            error.kind(),
            if cancel {
                io::ErrorKind::Interrupted
            } else {
                io::ErrorKind::TimedOut
            }
        );
        assert!(started.elapsed() < Duration::from_secs(4));
    }
    encoded(Format::Zip, Quality::Fast);
}

#[test]
fn crash_malformed_output_and_memory_failure_do_not_report_success() {
    for (script, memory) in [
        ("$g=[Console]::OpenStandardInput().ReadByte(); exit 9", 1024 * 1024 * 1024),
        ("$g=[Console]::OpenStandardInput().ReadByte(); [Console]::OpenStandardOutput().WriteByte(0)", 1024 * 1024 * 1024),
        ("$g=[Console]::OpenStandardInput().ReadByte(); $b=New-Object byte[] 134217728; [Threading.Thread]::Sleep(10000)", 64 * 1024 * 1024),
    ] {
        let started = Instant::now();
        assert!(windows_archive_io_worker::supervise(powershell(script), [7; 16], encode_operation(Format::Zip, Quality::Fast), &|| Ok(Control::default()), &mut noop,
            Duration::from_secs(3), memory).is_err());
        assert!(started.elapsed() < Duration::from_secs(5));
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
    let bytes = bytes
        .iter()
        .map(u8::to_string)
        .collect::<Vec<_>>()
        .join(",");
    powershell(&format!("$g=[Console]::OpenStandardInput().ReadByte(); $b=[byte[]](@({bytes})); $o=[Console]::OpenStandardOutput(); $o.Write($b,0,$b.Length); $o.Flush(); [Threading.Thread]::Sleep(10000)"))
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
    assert!(started.elapsed() < Duration::from_secs(4));
}

#[test]
fn data_after_terminal_result_is_rejected() {
    assert!(windows_archive_io_worker::supervise(
        fake_frames(&[Message::Finished, Message::Checkpoint]),
        [7; 16],
        encode_operation(Format::Zip, Quality::Fast),
        &|| Ok(Control::default()),
        &mut noop,
        Duration::from_secs(3),
        1024 * 1024 * 1024
    )
    .is_err());
}

#[test]
fn cancellation_remains_live_during_a_confirmed_pause() {
    let bytes = encoded(Format::Zip, Quality::Fast);
    let started = Instant::now();
    let error = windows_archive_io_worker::run_in(
        Path::new(env!("CARGO_BIN_EXE_oxyde")),
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
    assert!(started.elapsed() < Duration::from_secs(4));
}
