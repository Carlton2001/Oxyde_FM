//! The Viewer's audio and video player: playbin3 in a self-worker plays the sound
//! itself and sends video frames on its clock, which the UI paints, as Wayland gives
//! GPUI no way to place a foreign surface. This module is the parent side.
use super::linux_worker::Worker;
use super::viewer_media::{decode_media_report, ViewerMediaControl, MEDIA_FAILED, MEDIA_REPORT_BYTES};
use std::io;
use std::path::Path;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

mod worker;

pub(super) use worker::run;

/// Private memory of the player, the Windows job's ceiling.
const MEMORY_BYTES: u64 = 1024 * 1024 * 1024;
/// Largest frame side, whatever the Viewer's size.
const MAX_SIDE: u32 = 3840;
/// The worker reports at least this often while it lives.
const HEARTBEAT: Duration = Duration::from_secs(15);
const COMMAND_POLL: Duration = Duration::from_millis(20);
const REPORT: u8 = b'R';
const FRAME: u8 = b'F';

/// Parent to worker: pause, seek and the displayed size in physical pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Command {
    paused: bool,
    seek_id: u32,
    seek_ms: u64,
    width: u32,
    height: u32,
}

impl Command {
    const BYTES: usize = 21;

    fn encode(self) -> [u8; Self::BYTES] {
        let mut bytes = [0; Self::BYTES];
        bytes[0] = u8::from(self.paused);
        bytes[1..5].copy_from_slice(&self.seek_id.to_le_bytes());
        bytes[5..13].copy_from_slice(&self.seek_ms.to_le_bytes());
        bytes[13..17].copy_from_slice(&self.width.to_le_bytes());
        bytes[17..21].copy_from_slice(&self.height.to_le_bytes());
        bytes
    }

    fn decode(bytes: &[u8; Self::BYTES]) -> Self {
        let number = |range: std::ops::Range<usize>| u32::from_le_bytes(bytes[range].try_into().unwrap());
        Self {
            paused: bytes[0] != 0,
            seek_id: number(1..5),
            seek_ms: u64::from_le_bytes(bytes[5..13].try_into().unwrap()),
            width: number(13..17).min(MAX_SIDE),
            height: number(17..21).min(MAX_SIDE),
        }
    }
}

/// Blocking session on a background thread, until cancelled or the worker fails.
pub(super) fn supervise(path: &Path, control: &ViewerMediaControl) -> io::Result<()> {
    supervise_in(&std::env::current_exe()?, path, control)
}

fn supervise_in(executable: &Path, path: &Path, control: &ViewerMediaControl) -> io::Result<()> {
    let mut worker = Worker::start(executable, "viewer-media", Some(path), MEMORY_BYTES)?;
    let mut sent = None;
    let mut heard = Instant::now();
    while !control.is_cancelled() {
        let command = control.command.lock().map(|command| Command {
            paused: command.paused,
            seek_id: command.seek_id,
            seek_ms: command.seek_ms,
            width: command.rect[2].max(0) as u32,
            height: command.rect[3].max(0) as u32,
        }).map_err(|_| io::Error::other("Media command lock poisoned"))?;
        if sent != Some(command) {
            worker.send(&command.encode())?;
            sent = Some(command);
        }
        if !worker.readable(COMMAND_POLL)? {
            if heard.elapsed() > HEARTBEAT {
                return Err(io::Error::new(io::ErrorKind::TimedOut, "Media worker stopped reporting"));
            }
            continue;
        }
        heard = Instant::now();
        receive(&mut worker, control, command.seek_id)?;
    }
    Ok(())
}

/// One packet: a report, or a frame for the UI.
fn receive(worker: &mut Worker, control: &ViewerMediaControl, seek_id: u32) -> io::Result<()> {
    let deadline = Instant::now() + HEARTBEAT;
    let mut header = [0; 5];
    worker.read_exact(&mut header, deadline)?;
    let length = u32::from_le_bytes(header[..4].try_into().unwrap()) as usize;
    match header[4] {
        REPORT if length == MEDIA_REPORT_BYTES + 4 => {
            let mut packet = [0; MEDIA_REPORT_BYTES + 4];
            worker.read_exact(&mut packet, deadline)?;
            let report: &[u8; MEDIA_REPORT_BYTES] = packet[..MEDIA_REPORT_BYTES].try_into().unwrap();
            let (status, position, duration, [width, height]) =
                decode_media_report(report).ok_or_else(|| io::Error::other("Invalid media report"))?;
            control.status.store(status, Ordering::Release);
            // A position from before the latest seek would pull the progress bar back.
            if u32::from_le_bytes(packet[MEDIA_REPORT_BYTES..].try_into().unwrap()) == seek_id {
                control.position_ms.store(position, Ordering::Release);
            }
            control.duration_ms.store(duration, Ordering::Release);
            control.video_dimensions.store(u64::from(width) << 32 | u64::from(height), Ordering::Release);
            if status == MEDIA_FAILED {
                return Err(io::Error::other("Media playback failed"));
            }
        }
        FRAME if length >= 8 => {
            let mut size = [0; 8];
            worker.read_exact(&mut size, deadline)?;
            let width = u32::from_le_bytes(size[..4].try_into().unwrap());
            let height = u32::from_le_bytes(size[4..].try_into().unwrap());
            if !(1..=MAX_SIDE).contains(&width) || !(1..=MAX_SIDE).contains(&height)
                || length != 8 + width as usize * height as usize * 4 {
                return Err(io::Error::other("Invalid media frame"));
            }
            let mut pixels = vec![0; length - 8];
            worker.read_exact(&mut pixels, deadline)?;
            let frame = image::RgbaImage::from_raw(width, height, pixels).ok_or_else(|| io::Error::other("Invalid media frame"))?;
            if let Ok(mut slot) = control.frame.lock() {
                *slot = Some(frame);
            }
            control.frame_ready.notify_one();
        }
        _ => return Err(io::Error::other("Invalid media worker packet")),
    }
    Ok(())
}

#[cfg(test)]
#[path = "linux_media/tests/media.rs"]
mod tests;
