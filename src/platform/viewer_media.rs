//! Cross-platform contract. Native decoding and IPC run outside the UI thread.
use std::sync::{
    atomic::{AtomicBool, AtomicU64, AtomicU8, Ordering},
    Mutex,
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ViewerMediaCommand {
    pub parent: u64,
    /// Nonactivating GPUI controls; the video surface stays below this HWND.
    #[serde(default)]
    pub controls: u64,
    pub rect: [i32; 4],
    pub paused: bool,
    pub visible: bool,
    /// Each new `seek_id` moves playback to `seek_ms` once the media is loaded.
    pub seek_id: u32,
    pub seek_ms: u64,
}

/// Worker reports: status, position/duration in milliseconds, native video width/height.
pub const MEDIA_REPORT_BYTES: usize = 25;

pub fn encode_media_report(status: u8, position_ms: u64, duration_ms: u64, dimensions: [u32; 2]) -> [u8; MEDIA_REPORT_BYTES] {
    let mut report = [0; MEDIA_REPORT_BYTES];
    report[0] = status;
    report[1..9].copy_from_slice(&position_ms.to_le_bytes());
    report[9..17].copy_from_slice(&duration_ms.to_le_bytes());
    report[17..21].copy_from_slice(&dimensions[0].to_le_bytes());
    report[21..25].copy_from_slice(&dimensions[1].to_le_bytes());
    report
}

pub fn decode_media_report(report: &[u8; MEDIA_REPORT_BYTES]) -> Option<(u8, u64, u64, [u32; 2])> {
    let status = report[0];
    if !matches!(status, MEDIA_LOADING | MEDIA_PLAYING | MEDIA_PAUSED | MEDIA_ENDED | MEDIA_FAILED) {
        return None;
    }
    let position = u64::from_le_bytes(report[1..9].try_into().ok()?);
    let duration = u64::from_le_bytes(report[9..17].try_into().ok()?);
    Some((status, position, duration, [u32::from_le_bytes(report[17..21].try_into().ok()?), u32::from_le_bytes(report[21..25].try_into().ok()?)]))
}

/// A report slot carrying `MediaInfo` instead of progress, sent once streams are known.
pub const MEDIA_INFO: u8 = b'I';

/// Codecs both platforms recognize; anything else stays unnamed.
pub const VIDEO_CODECS: [&str; 12] = ["H.264", "HEVC", "VP8", "VP9", "AV1", "MPEG-4", "MPEG-2", "MPEG-1", "WMV", "ProRes", "MJPEG", "Theora"];
pub const AUDIO_CODECS: [&str; 13] = ["AAC", "MP3", "AC-3", "E-AC-3", "Opus", "Vorbis", "FLAC", "PCM", "DTS", "WMA", "ALAC", "MP2", "TrueHD"];

/// Technical description of the playing file. Codecs are 1-based indices into
/// `VIDEO_CODECS`/`AUDIO_CODECS`, 0 when absent or unknown.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MediaInfo {
    pub video_codec: u8,
    pub audio_codec: u8,
    /// Frames per thousand seconds; 0 when unknown.
    pub frame_rate_milli: u32,
    pub sample_rate: u32,
    pub channels: u8,
}

impl MediaInfo {
    pub fn video_codec_name(&self) -> Option<&'static str> {
        VIDEO_CODECS.get(usize::from(self.video_codec).checked_sub(1)?).copied()
    }
    pub fn audio_codec_name(&self) -> Option<&'static str> {
        AUDIO_CODECS.get(usize::from(self.audio_codec).checked_sub(1)?).copied()
    }
}

pub fn video_codec_index(name: &str) -> u8 {
    VIDEO_CODECS.iter().position(|codec| *codec == name).map_or(0, |index| index as u8 + 1)
}

pub fn audio_codec_index(name: &str) -> u8 {
    AUDIO_CODECS.iter().position(|codec| *codec == name).map_or(0, |index| index as u8 + 1)
}

pub fn encode_media_info(info: &MediaInfo) -> [u8; MEDIA_REPORT_BYTES] {
    let mut report = [0; MEDIA_REPORT_BYTES];
    report[0] = MEDIA_INFO;
    report[1] = info.video_codec;
    report[2] = info.audio_codec;
    report[3..7].copy_from_slice(&info.frame_rate_milli.to_le_bytes());
    report[7..11].copy_from_slice(&info.sample_rate.to_le_bytes());
    report[11] = info.channels;
    report
}

/// `None` for a progress report or values out of range.
pub fn decode_media_info(report: &[u8; MEDIA_REPORT_BYTES]) -> Option<MediaInfo> {
    if report[0] != MEDIA_INFO {
        return None;
    }
    let info = MediaInfo {
        video_codec: report[1],
        audio_codec: report[2],
        frame_rate_milli: u32::from_le_bytes(report[3..7].try_into().ok()?),
        sample_rate: u32::from_le_bytes(report[7..11].try_into().ok()?),
        channels: report[11],
    };
    (usize::from(info.video_codec) <= VIDEO_CODECS.len()
        && usize::from(info.audio_codec) <= AUDIO_CODECS.len()
        && info.frame_rate_milli <= 1_000_000
        && info.sample_rate <= 1_000_000)
        .then_some(info)
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
    pub video_dimensions: AtomicU64,
    /// Where no native surface shows the video (Linux), its latest frame in premultiplied
    /// BGRA, which the UI takes and paints itself once notified.
    pub frame: Mutex<Option<image::RgbaImage>>,
    pub frame_ready: tokio::sync::Notify,
    /// Codecs and rates, once the worker has identified the streams.
    pub info: Mutex<Option<MediaInfo>>,
    supervisor: Mutex<Option<std::thread::Thread>>,
}

impl ViewerMediaControl {
    #[cfg(target_os = "windows")]
    pub(crate) fn register_supervisor(&self) {
        if let Ok(mut thread) = self.supervisor.lock() {
            *thread = Some(std::thread::current());
        }
    }
    pub fn wake(&self) {
        if let Ok(thread) = self.supervisor.lock() {
            if let Some(thread) = thread.as_ref() { thread.unpark(); }
        }
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
        self.wake();
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
    pub fn info(&self) -> Option<MediaInfo> {
        self.info.lock().ok().and_then(|info| *info)
    }
    pub(crate) fn set_info(&self, info: MediaInfo) {
        if let Ok(mut slot) = self.info.lock() {
            *slot = Some(info);
        }
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
        self.wake();
    }
}
