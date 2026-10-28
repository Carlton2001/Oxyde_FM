//! Parent side: one video worker at a time, its packets checked before the UI sees them.
use super::super::linux_worker::Worker;
use super::SIDE;
use crate::utils::preview_protocol::{self, Limits, Reply};
use image::RgbaImage;
use std::io;
use std::path::Path;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Private memory of the decoder, the Windows job's ceiling.
const MEMORY_BYTES: u64 = 1024 * 1024 * 1024;
/// Longest wait for the next frame.
const DEADLINE: Duration = Duration::from_secs(15);
/// Packet: length, then seconds and nanoseconds of the timestamp, then the frame.
const MAX_PACKET: usize = 2 * 1024 * 1024;
const CANCEL_CHECK: Duration = Duration::from_millis(50);
static SLOT: Mutex<()> = Mutex::new(());

pub(in crate::platform) fn stream(
    path: &Path,
    on_frame: &mut dyn FnMut(RgbaImage, Duration) -> bool,
    cancelled: &dyn Fn() -> bool,
) -> io::Result<()> {
    if cancelled() {
        return Ok(());
    }
    let _slot = SLOT.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    stream_in(&std::env::current_exe()?, path, on_frame, cancelled, DEADLINE)
}

fn stream_in(
    executable: &Path,
    path: &Path,
    on_frame: &mut dyn FnMut(RgbaImage, Duration) -> bool,
    cancelled: &dyn Fn() -> bool,
    deadline: Duration,
) -> io::Result<()> {
    if cancelled() {
        return Ok(());
    }
    let mut worker = Worker::start(executable, "video", Some(path), MEMORY_BYTES)?;
    loop {
        let waiting = Instant::now();
        while !worker.readable(CANCEL_CHECK)? {
            if cancelled() {
                return Ok(());
            }
            if waiting.elapsed() >= deadline {
                return Err(io::Error::new(io::ErrorKind::TimedOut, "PreviewWorkerTimeout: Video produced no frame for 15 seconds"));
            }
        }
        let read_by = Instant::now() + deadline;
        let mut header = [0; 4];
        worker.read_exact(&mut header, read_by)?;
        let length = u32::from_le_bytes(header) as usize;
        if !(12..=MAX_PACKET).contains(&length) {
            return Err(io::Error::other("Invalid video worker packet size"));
        }
        let mut packet = vec![0; length];
        worker.read_exact(&mut packet, read_by)?;
        let seconds = u64::from_le_bytes(packet[..8].try_into().unwrap());
        let nanos = u32::from_le_bytes(packet[8..12].try_into().unwrap());
        if nanos >= 1_000_000_000 {
            return Err(io::Error::other("Invalid video worker timestamp"));
        }
        let limits = Limits { pixels: (SIDE * SIDE * 4) as usize, side: SIDE };
        match preview_protocol::decode_with(&packet[12..], limits)? {
            Reply::Images { mut frames, limited: false, .. } if frames.len() == 1 => {
                if !on_frame(frames.remove(0).into_buffer(), Duration::new(seconds, nanos)) {
                    return Ok(());
                }
            }
            Reply::Error { detail, .. } => return Err(io::Error::other(detail)),
            _ => return Err(io::Error::other("Unexpected video worker reply")),
        }
        if cancelled() {
            return Ok(());
        }
    }
}

#[cfg(test)]
#[path = "tests/stream.rs"]
mod tests;
