//! Cross-platform contract. Native decoding and IPC run outside the UI thread.
use std::sync::{
    atomic::{AtomicBool, AtomicU64, AtomicU8, Ordering},
    Mutex,
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ViewerMediaCommand {
    pub parent: u64,
    pub rect: [i32; 4],
    pub paused: bool,
    pub visible: bool,
    /// Each new `seek_id` moves playback to `seek_ms` once the media is loaded.
    pub seek_id: u32,
    pub seek_ms: u64,
}

/// Worker reports: status byte, then position and duration in milliseconds.
pub const MEDIA_REPORT_BYTES: usize = 17;

pub fn encode_media_report(status: u8, position_ms: u64, duration_ms: u64) -> [u8; MEDIA_REPORT_BYTES] {
    let mut report = [0; MEDIA_REPORT_BYTES];
    report[0] = status;
    report[1..9].copy_from_slice(&position_ms.to_le_bytes());
    report[9..].copy_from_slice(&duration_ms.to_le_bytes());
    report
}

pub fn decode_media_report(report: &[u8; MEDIA_REPORT_BYTES]) -> Option<(u8, u64, u64)> {
    let status = report[0];
    if !matches!(status, MEDIA_LOADING | MEDIA_PLAYING | MEDIA_PAUSED | MEDIA_ENDED | MEDIA_FAILED) {
        return None;
    }
    let position = u64::from_le_bytes(report[1..9].try_into().ok()?);
    let duration = u64::from_le_bytes(report[9..].try_into().ok()?);
    Some((status, position, duration))
}

pub const MEDIA_LOADING: u8 = 0;
pub const MEDIA_PLAYING: u8 = b'P';
pub const MEDIA_PAUSED: u8 = b'A';
pub const MEDIA_ENDED: u8 = b'E';
pub const MEDIA_FAILED: u8 = b'X';

#[derive(Default)]
pub struct ViewerMediaControl {
    pub command: Mutex<ViewerMediaCommand>,
    pub cancelled: AtomicBool,
    pub status: AtomicU8,
    pub position_ms: AtomicU64,
    pub duration_ms: AtomicU64,
}

impl ViewerMediaControl {
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }
    pub fn status(&self) -> u8 {
        self.status.load(Ordering::Acquire)
    }
    pub fn position_ms(&self) -> u64 {
        self.position_ms.load(Ordering::Acquire)
    }
    /// Zero until the duration is known.
    pub fn duration_ms(&self) -> u64 {
        self.duration_ms.load(Ordering::Acquire)
    }
    /// Requests a seek; the supervisor forwards the changed command.
    pub fn seek(&self, position_ms: u64) {
        if let Ok(mut command) = self.command.lock() {
            command.seek_id = command.seek_id.wrapping_add(1).max(1);
            command.seek_ms = position_ms;
        }
        self.position_ms.store(position_ms, Ordering::Release);
    }
}
