use super::*;
use crate::utils::archive_io_protocol::{Entry, Limits, Quality};
use std::cell::{Cell, RefCell};
use std::path::PathBuf;

fn worker() -> PathBuf {
    let executable = std::env::current_exe().unwrap().parent().unwrap().parent().unwrap().join("oxyde-archive-io-worker");
    assert!(executable.is_file(), "Build the workers before these tests: cargo build --offline --bins");
    executable
}

fn limits() -> Limits {
    Limits { entries: 10, bytes: 16 * 1024 * 1024, depth: 64, seconds: 120 }
}

fn entry(size: u64) -> Entry {
    Entry { id: 0, name: "école.txt".into(), directory: false, size, mode: 0o644, modified: None, created: None, accessed: None }
}

/// Runs the real encoder over one file and returns the archive it wrote.
fn encoded(format: Format, content: &[u8], controls: &dyn Fn() -> io::Result<Control>) -> io::Result<Vec<u8>> {
    let operation = Operation::Encode { format, quality: Quality::Fast, entries: 1, output_limit: 64 * 1024 * 1024, limits: limits() };
    let output = RefCell::new(Vec::new());
    let mut handler = |frame: Frame| {
        frame.validate()?;
        let ack = Frame::new(Message::Ack { pause: false, turbo: false });
        Ok(match frame.message {
            Message::Source { .. } => Frame::new(Message::Entry { entry: entry(content.len() as u64) }),
            Message::Read { source, offset, length } => {
                let start = offset as usize;
                Frame::data(Message::Bytes { source, offset }, content[start..start + length as usize].to_vec())
            }
            Message::Write { offset } => {
                let mut output = output.borrow_mut();
                let end = offset as usize + frame.data.len();
                if output.len() < end { output.resize(end, 0); }
                output[offset as usize..end].copy_from_slice(&frame.data);
                ack
            }
            _ => ack,
        })
    };
    run_in(&worker(), operation, controls, &mut handler, Duration::from_secs(60))?;
    Ok(output.into_inner())
}

fn free() -> io::Result<Control> {
    Ok(Control::default())
}

#[test]
fn the_worker_encodes_and_decodes_through_the_parent_alone() {
    let content: Vec<u8> = (0..1_500_000u32).map(|value| (value % 251) as u8).collect();
    for format in [Format::Zip, Format::SevenZip, Format::Tar, Format::TarZst] {
        let archive = encoded(format, &content, &free).unwrap();
        let operation = Operation::Decode { format, source_size: archive.len() as u64, selected: None, limits: limits() };
        let mut restored = Vec::new();
        let finished = Cell::new(false);
        let mut handler = |frame: Frame| {
            frame.validate()?;
            match frame.message {
                Message::Read { source, offset, length } => {
                    let start = offset as usize;
                    return Ok(Frame::data(Message::Bytes { source, offset }, archive[start..start + length as usize].to_vec()));
                }
                Message::Chunk { .. } => restored.extend_from_slice(&frame.data),
                Message::Finished => finished.set(true),
                _ => {}
            }
            Ok(Frame::new(Message::Ack { pause: false, turbo: false }))
        };
        run_in(&worker(), operation, &free, &mut handler, Duration::from_secs(60)).unwrap();
        assert!(finished.get(), "{format:?}");
        assert!(restored == content, "{format:?}");
    }
}

#[test]
fn a_confirmed_pause_stops_effects_and_does_not_count_against_the_deadline() {
    let started = Instant::now();
    let controls = || Ok(Control { paused: started.elapsed() < Duration::from_millis(400), ..Control::default() });
    let archive = encoded(Format::Tar, b"contenu", &controls).unwrap();
    assert!(started.elapsed() >= Duration::from_millis(400));
    assert!(!archive.is_empty());
}

#[test]
fn cancellation_ends_the_session_as_interrupted() {
    let error = encoded(Format::Tar, b"contenu", &|| Ok(Control { cancelled: true, ..Control::default() })).unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::Interrupted);
    let calls = Cell::new(0);
    let late = || {
        calls.set(calls.get() + 1);
        Ok(Control { cancelled: calls.get() > 3, ..Control::default() })
    };
    assert_eq!(encoded(Format::Zip, &[7; 1024], &late).unwrap_err().kind(), io::ErrorKind::Interrupted);
}

/// A worker that does what the parent would never allow it to.
fn hostile(script: &str) -> PathBuf {
    let directory = std::env::temp_dir().join(format!("oxyde-hostile-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join("worker");
    std::fs::write(&path, format!("#!/bin/sh\n{script}\n")).unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    path
}

fn decode_with(executable: &Path, deadline: Duration) -> io::Result<Duration> {
    let operation = Operation::Decode { format: Format::Tar, source_size: 0, selected: None, limits: limits() };
    run_in(executable, operation, &free, &mut |_| Ok(Frame::new(Message::Ack { pause: false, turbo: false })), deadline)
}

#[test]
fn a_silent_worker_runs_out_of_time_and_garbage_or_a_crash_is_refused() {
    let silent = hostile("exec sleep 30");
    let error = decode_with(&silent, Duration::from_millis(300)).unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::TimedOut);
    let garbage = hostile("head -c 4096 /dev/urandom");
    assert!(decode_with(&garbage, Duration::from_secs(10)).is_err());
    let crash = hostile("exit 3");
    assert!(decode_with(&crash, Duration::from_secs(10)).is_err());
    for path in [silent, garbage, crash] {
        std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }
}
