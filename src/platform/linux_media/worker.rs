//! Worker side: plays the file, applies commands, reports and sends frames.
use super::super::linux_gstreamer::{Player, PAUSED, PLAYING};
use super::super::viewer_media::{encode_media_report, MEDIA_ENDED, MEDIA_FAILED, MEDIA_LOADING, MEDIA_PAUSED, MEDIA_PLAYING};
use super::{Command, FRAME, MAX_SIDE, REPORT};
use std::ffi::{CStr, CString};
use std::io::{self, Read, Write};
use std::path::Path;
use std::sync::mpsc::{self, TryRecvError};
use std::time::{Duration, Instant};

/// Sound and picture, without subtitles; the sound goes to the desktop's default output.
/// Native video leaves the picture unscaled up to the output, which then knows the
/// source size and converts and scales in one pass.
const PLAYER: &CStr = c"playbin3 flags=video+audio+soft-volume+deinterlace+native-video";
const PULL_TIMEOUT: Duration = Duration::from_millis(20);
const REPORT_INTERVAL: Duration = Duration::from_millis(100);
/// Resizing renegotiates the scaler: wait until the Viewer's size settles.
const RESIZE_SETTLE: Duration = Duration::from_millis(150);

/// Frames scaled to the displayed size, without enlarging; the sink follows the clock,
/// dropping a frame rather than delaying the sound when Oxyde falls behind.
fn video_output(width: u32, height: u32) -> CString {
    CString::new(format!("videoconvertscale name=source ! capsfilter name=size caps=\"{}\" ! \
        appsink name=frames max-buffers=1 drop=true", caps(width, height))).expect("no NUL in caps")
}

fn caps(width: u32, height: u32) -> String {
    let bound = |side: u32| if side == 0 { MAX_SIDE } else { side.clamp(16, MAX_SIDE) };
    format!("video/x-raw,format=BGRA,pixel-aspect-ratio=1/1,width=[1,{}],height=[1,{}]", bound(width), bound(height))
}

fn packet(kind: u8, parts: &[&[u8]], output: &mut impl Write) -> io::Result<()> {
    let length: usize = parts.iter().map(|part| part.len()).sum();
    output.write_all(&(length as u32).to_le_bytes())?;
    output.write_all(&[kind])?;
    for part in parts {
        output.write_all(part)?;
    }
    output.flush()
}

pub(in crate::platform) fn run(path: &Path) -> io::Result<()> {
    let (sender, commands) = mpsc::channel();
    // Commands arrive on their own thread; Oxyde closing the pipe ends the player.
    std::thread::spawn(move || {
        let mut input = io::stdin().lock();
        let mut bytes = [0; Command::BYTES];
        while input.read_exact(&mut bytes).is_ok() && sender.send(Command::decode(&bytes)).is_ok() {}
    });
    let Ok(mut command) = commands.recv() else { return Ok(()) };
    let mut output = io::stdout().lock();
    let report = |status: u8, position: u64, duration: u64, size: [u32; 2], seek_id: u32, output: &mut io::StdoutLock| {
        packet(REPORT, &[&encode_media_report(status, position, duration, size), &seek_id.to_le_bytes()], output)
    };
    let player = match Player::open(path, PLAYER, &video_output(command.width, command.height), if command.paused { PAUSED } else { PLAYING }) {
        Ok(player) => player,
        Err(error) => {
            log::warn!("Viewer media worker: {error}");
            return report(MEDIA_FAILED, 0, 0, [0; 2], 0, &mut output);
        }
    };
    let mut loaded = false;
    let mut ended = false;
    let mut applied_seek = 0;
    let mut size = [0; 2];
    let mut shown = (0, 0);
    let mut last_report = Instant::now() - REPORT_INTERVAL;
    let mut resized: Option<Instant> = None;
    let mut position = Duration::from_millis(command.seek_ms);
    loop {
        loop {
            match commands.try_recv() {
                Ok(next) => {
                    if (next.width, next.height) != (command.width, command.height) {
                        resized = Some(Instant::now());
                    }
                    if next.paused != command.paused && !ended {
                        player.set_state(if next.paused { PAUSED } else { PLAYING });
                    }
                    command = next;
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => return Ok(()),
            }
        }
        let events = player.events();
        if let Some(error) = events.error {
            log::warn!("Viewer media worker: {error}");
            return report(MEDIA_FAILED, position.as_millis() as u64, 0, size, applied_seek, &mut output);
        }
        loaded |= events.settled;
        ended |= events.ended;
        // A seek waits for the media to load; a new one also leaves the end.
        if loaded && command.seek_id != applied_seek {
            applied_seek = command.seek_id;
            position = Duration::from_millis(command.seek_ms);
            if player.seek(position) && ended {
                ended = false;
                player.set_state(if command.paused { PAUSED } else { PLAYING });
            }
        }
        if resized.is_some_and(|at| at.elapsed() >= RESIZE_SETTLE) {
            resized = None;
            player.set_output_caps(&CString::new(caps(command.width, command.height)).expect("no NUL in caps"));
        }
        let started = Instant::now();
        // A paused sink hands its frame over again: send it once.
        let sample = player.pull(PULL_TIMEOUT, command.paused || ended)
            .filter(|sample| sample.identity() != shown);
        if let Some(sample) = sample {
            shown = sample.identity();
            let (image, _) = sample.image(MAX_SIDE).map_err(io::Error::other)?;
            size = player.source_size().map_or([image.width(), image.height()], |(width, height)| [width, height]);
            packet(FRAME, &[&image.width().to_le_bytes(), &image.height().to_le_bytes(), image.as_raw()], &mut output)?;
        } else if started.elapsed() < PULL_TIMEOUT / 4 {
            // Nothing new at once: paused, sound alone, or a sink not started yet.
            std::thread::sleep(PULL_TIMEOUT);
        }
        if last_report.elapsed() >= REPORT_INTERVAL {
            last_report = Instant::now();
            let duration = player.duration();
            position = if ended { duration.unwrap_or(position) } else { player.position().unwrap_or(position) };
            let status = match () {
                _ if ended => MEDIA_ENDED,
                _ if !loaded => MEDIA_LOADING,
                _ if command.paused => MEDIA_PAUSED,
                _ => MEDIA_PLAYING,
            };
            let millis = |time: Duration| time.as_millis() as u64;
            report(status, millis(position), duration.map_or(0, millis), size, applied_seek, &mut output)?;
        }
    }
}
