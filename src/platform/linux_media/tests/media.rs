use super::super::viewer_media::{ViewerMediaCommand, MEDIA_ENDED, MEDIA_PAUSED, MEDIA_PLAYING};
use super::*;
use std::path::PathBuf;
use std::sync::Arc;

/// The application binary, which serves as its own worker.
fn oxyde() -> PathBuf {
    let executable = std::env::current_exe().unwrap().parent().unwrap().parent().unwrap().join("oxyde");
    assert!(executable.is_file(), "Build oxyde before worker tests: cargo build --offline --bins");
    executable
}

/// A clip encoded by GStreamer itself, or None where its encoders are missing.
fn encode(directory: &Path, name: &str, pipeline: &str) -> Option<PathBuf> {
    let path = directory.join(name);
    let launched = std::process::Command::new("gst-launch-1.0")
        .arg("-q")
        .args(pipeline.replace("{out}", &path.display().to_string()).split(' '))
        .status();
    if !launched.is_ok_and(|status| status.success()) {
        eprintln!("skipped: gst-launch-1.0 cannot encode {name}");
        return None;
    }
    Some(path)
}

/// Two seconds of silent 16:9 video at 10 fps, without a sound track.
fn clip(directory: &Path) -> Option<PathBuf> {
    encode(directory, "clip.mp4", "videotestsrc num-buffers=20 ! video/x-raw,width=1280,height=720,framerate=10/1 \
        ! videoconvert ! openh264enc ! h264parse ! mp4mux ! filesink location={out}")
}

/// Plays `path` on its own thread, as the Viewer's media queue does.
fn play(path: PathBuf, command: ViewerMediaCommand) -> (Arc<ViewerMediaControl>, std::thread::JoinHandle<io::Result<()>>) {
    let control = Arc::new(ViewerMediaControl::default());
    *control.command.lock().unwrap() = command;
    let session = control.clone();
    (control, std::thread::spawn(move || supervise_in(&oxyde(), &path, &session)))
}

fn wait_for(control: &ViewerMediaControl, what: &str, done: impl Fn(&ViewerMediaControl) -> bool) {
    let started = Instant::now();
    while !done(control) {
        assert!(started.elapsed() < Duration::from_secs(10), "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn commands_survive_the_pipe_and_sizes_stay_bounded() {
    let command = Command { paused: true, seek_id: 7, seek_ms: 90_000, width: 800, height: 450 };
    assert_eq!(Command::decode(&command.encode()), command);
    let huge = Command { width: u32::MAX, height: 5000, ..command };
    assert_eq!(Command::decode(&huge.encode()).width, MAX_SIDE);
    assert_eq!(Command::decode(&huge.encode()).height, MAX_SIDE);
}

#[test]
fn video_plays_at_the_viewer_size_then_pauses_seeks_and_ends() {
    let directory = tempfile::tempdir().unwrap();
    let Some(path) = clip(directory.path()) else { return };
    let (control, session) = play(path, ViewerMediaCommand { rect: [0, 0, 320, 320], ..Default::default() });
    wait_for(&control, "a frame", |control| control.frame.lock().unwrap().is_some());
    let frame = control.frame.lock().unwrap().take().unwrap();
    assert_eq!(frame.dimensions(), (320, 180), "Scaled to the Viewer, keeping 16:9");
    wait_for(&control, "playing", |control| control.status() == MEDIA_PLAYING && control.duration_ms() == 2000);
    assert_eq!(control.video_dimensions.load(Ordering::Acquire), 1280 << 32 | 720, "The source size, not the scaled one");

    control.command.lock().unwrap().paused = true;
    wait_for(&control, "paused", |control| control.status() == MEDIA_PAUSED);
    control.seek(1500);
    wait_for(&control, "the seek", |control| (1400..=1600).contains(&control.position_ms()));
    control.command.lock().unwrap().paused = false;
    wait_for(&control, "the end", |control| control.status() == MEDIA_ENDED);
    assert_eq!(control.position_ms(), 2000);

    control.cancel();
    session.join().unwrap().unwrap();
}

#[test]
fn sound_alone_plays_without_frames() {
    let directory = tempfile::tempdir().unwrap();
    let Some(path) = encode(directory.path(), "silence.ogg", "audiotestsrc wave=silence num-buffers=20 ! vorbisenc ! oggmux ! filesink location={out}") else { return };
    let (control, session) = play(path, ViewerMediaCommand { paused: true, ..Default::default() });
    wait_for(&control, "paused", |control| control.status() == MEDIA_PAUSED && control.duration_ms() > 0);
    assert!(control.frame.lock().unwrap().is_none());
    assert_eq!(control.video_dimensions.load(Ordering::Acquire), 0);
    control.cancel();
    session.join().unwrap().unwrap();
}

#[test]
fn a_damaged_file_fails_the_session() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("damaged.mp4");
    std::fs::write(&path, b"\0\0\0\x18ftypmp42\0\0\0\0truncated").unwrap();
    let (control, session) = play(path, ViewerMediaCommand::default());
    assert!(session.join().unwrap().is_err());
    assert_eq!(control.status(), MEDIA_FAILED);
}
