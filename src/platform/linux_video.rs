//! Muted hover videos through GStreamer, as Totem and the video thumbnailer Nautilus
//! runs decode them. Decoding stays in a self-worker; `stream` is the parent side.
use super::linux_gstreamer::{Player, PLAYING, READY};
use crate::models::CommandError;
use image::RgbaImage;
use std::ffi::CStr;
use std::path::Path;
use std::time::Duration;

mod stream;

pub(super) use stream::stream;

/// Longest side of hover frames, as on Windows.
const SIDE: u32 = 640;
/// Video only, scaled by playbin's own converter, then BGRA for the UI. The sink
/// does not wait for the clock: the UI paces frames by their timestamps. playbin3
/// selects the video stream alone, where playbin still queued the ignored audio:
/// 205 MiB instead of 770 MiB for a 4K HEVC clip.
const PLAYER: &CStr = c"playbin3 flags=video";
const OUTPUT: &CStr = c"videoconvert ! videoscale ! \
    video/x-raw,format=BGRA,pixel-aspect-ratio=1/1,width=[1,640],height=[1,640] ! \
    appsink name=frames sync=false max-buffers=2";
/// Pause between loops, as on Windows.
const LOOP_PAUSE: Duration = Duration::from_millis(200);
const PULL_TIMEOUT: Duration = Duration::from_millis(100);

/// Worker side: frames in a loop until `on_frame` declines one.
pub(super) fn decode(path: &Path, on_frame: &mut dyn FnMut(RgbaImage, Duration) -> bool) -> Result<(), CommandError> {
    let player = Player::open(path, PLAYER, OUTPUT, PLAYING)?;
    let mut pts = Duration::ZERO;
    let mut looped_frame = false;
    // The bus says when the stream ended: the sink also reports an end before playbin3
    // has added it, once it knows the streams. Frames still queued come first.
    let mut ended = false;
    loop {
        let events = player.events();
        if let Some(error) = events.error {
            return Err(CommandError::Other(error));
        }
        if events.video == Some(false) {
            return Err(CommandError::Other("No video stream".into()));
        }
        ended |= events.ended;
        if let Some(sample) = player.pull(if ended { Duration::ZERO } else { PULL_TIMEOUT }, false) {
            let (image, at) = sample.image(SIDE)?;
            pts = at.unwrap_or(pts);
            looped_frame = true;
            if !on_frame(image, pts) {
                return Ok(());
            }
        } else if ended {
            if !looped_frame {
                return Err(CommandError::Other("Video stream ended without a frame".into()));
            }
            std::thread::sleep(LOOP_PAUSE);
            // Like the Windows reader opened again; READY keeps the file.
            if !(player.set_state(READY) && player.set_state(PLAYING)) {
                return Err(CommandError::Other("Cannot loop the video".into()));
            }
            looped_frame = false;
            ended = false;
        } else {
            // A sink that playbin3 has not started yet answers at once.
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}

#[cfg(test)]
#[path = "linux_video/tests/video.rs"]
mod tests;
