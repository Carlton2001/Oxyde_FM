use super::super::tests::wide;
use super::*;
use std::path::PathBuf;

/// The application binary, which serves as its own worker.
fn oxyde() -> PathBuf {
    let executable = std::env::current_exe().unwrap().parent().unwrap().parent().unwrap().join("oxyde");
    assert!(executable.is_file(), "Build oxyde before worker tests: cargo build --offline --bins");
    executable
}

#[test]
fn frames_arrive_from_the_worker_until_declined() {
    let directory = tempfile::tempdir().unwrap();
    let Some(path) = wide(directory.path()) else { return };
    let mut frames = Vec::new();
    stream_in(&oxyde(), &path, &mut |image, pts| {
        frames.push((image.dimensions(), pts));
        frames.len() < 12
    }, &|| false, DEADLINE).unwrap();
    assert_eq!(frames.len(), 12);
    assert_eq!(frames[0].0, (640, 360));
    assert!(frames[10].1 < frames[9].1, "The worker loops the clip");
}

#[test]
fn cancelling_ends_the_stream_promptly() {
    let directory = tempfile::tempdir().unwrap();
    let Some(path) = wide(directory.path()) else { return };
    let started = Instant::now();
    let cancelled = || started.elapsed() > Duration::from_millis(300);
    stream_in(&oxyde(), &path, &mut |_, _| { std::thread::sleep(Duration::from_millis(50)); true }, &cancelled, DEADLINE).unwrap();
    assert!(started.elapsed() < Duration::from_secs(2));
}

#[test]
fn a_damaged_video_is_an_error_from_the_worker() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("damaged.mp4");
    std::fs::write(&path, b"\0\0\0\x18ftypmp42\0\0\0\0truncated").unwrap();
    assert!(stream_in(&oxyde(), &path, &mut |_, _| true, &|| false, DEADLINE).is_err());
}
