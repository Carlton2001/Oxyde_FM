use super::*;
use std::path::PathBuf;

/// A clip encoded by GStreamer itself, or None where its encoders are missing.
pub(super) fn encode(directory: &Path, name: &str, pipeline: &str) -> Option<PathBuf> {
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

/// Ten wide H.264 frames at 10 fps.
pub(super) fn wide(directory: &Path) -> Option<PathBuf> {
    encode(directory, "wide.mp4", "videotestsrc num-buffers=10 pattern=red ! video/x-raw,width=1280,height=720,framerate=10/1 \
        ! videoconvert ! openh264enc ! h264parse ! mp4mux ! filesink location={out}")
}

fn frames(path: &Path, count: usize) -> Result<Vec<(RgbaImage, Duration)>, CommandError> {
    let mut frames = Vec::new();
    decode(path, &mut |image, pts| {
        frames.push((image, pts));
        frames.len() < count
    })?;
    Ok(frames)
}

#[test]
fn frames_are_scaled_bgra_with_their_timestamps() {
    let directory = tempfile::tempdir().unwrap();
    let Some(path) = wide(directory.path()) else { return };
    let frames = frames(&path, 3).unwrap();
    assert_eq!(frames.len(), 3);
    let (first, _) = &frames[0];
    assert_eq!(first.dimensions(), (640, 360));
    let [blue, green, red, alpha] = first.get_pixel(320, 180).0;
    assert!(red > 200 && green < 50 && blue < 50 && alpha == 255, "red expected as BGRA, got {:?}", first.get_pixel(320, 180).0);
    assert_eq!(frames[1].1 - frames[0].1, Duration::from_millis(100));
}

#[test]
fn small_and_portrait_videos_keep_their_shape() {
    let directory = tempfile::tempdir().unwrap();
    let Some(path) = encode(directory.path(), "portrait.webm", "videotestsrc num-buffers=3 ! video/x-raw,width=320,height=480,framerate=10/1 \
        ! vp8enc ! webmmux ! filesink location={out}") else { return };
    assert_eq!(frames(&path, 1).unwrap()[0].0.dimensions(), (320, 480));
}

#[test]
fn a_finished_clip_starts_over() {
    let directory = tempfile::tempdir().unwrap();
    let Some(path) = wide(directory.path()) else { return };
    let frames = frames(&path, 15).unwrap();
    assert_eq!(frames.len(), 15);
    assert!(frames[10].1 < frames[9].1, "The eleventh frame restarts the clip");
}

#[test]
fn files_without_a_playable_video_are_errors() {
    let directory = tempfile::tempdir().unwrap();
    let fake = directory.path().join("fake.mp4");
    std::fs::write(&fake, b"not a video at all").unwrap();
    assert!(frames(&fake, 1).is_err());
    assert!(frames(&directory.path().join("missing.mp4"), 1).is_err());
    let Some(sound) = encode(directory.path(), "sound.ogg", "audiotestsrc num-buffers=10 ! vorbisenc ! oggmux ! filesink location={out}") else { return };
    assert!(frames(&sound, 1).is_err());
}
