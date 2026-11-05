//! Worker side: names the codecs and rates of the file being played. The media
//! engine does not expose them, so a separate source reader reads the native types.
use super::super::viewer_media::{audio_codec_index, video_codec_index, MediaInfo};
use std::path::Path;
use windows::core::{GUID, HSTRING};
use windows::Win32::Media::MediaFoundation::*;

fn video_codec(subtype: GUID) -> Option<&'static str> {
    Some(match subtype {
        value if value == MFVideoFormat_H264 => "H.264",
        value if value == MFVideoFormat_HEVC => "HEVC",
        value if value == MFVideoFormat_VP80 => "VP8",
        value if value == MFVideoFormat_VP90 => "VP9",
        value if value == MFVideoFormat_AV1 => "AV1",
        value if value == MFVideoFormat_MP4V || value == MFVideoFormat_M4S2 => "MPEG-4",
        value if value == MFVideoFormat_MPEG2 => "MPEG-2",
        value if value == MFVideoFormat_MPG1 => "MPEG-1",
        value if value == MFVideoFormat_WMV3 || value == MFVideoFormat_WVC1 => "WMV",
        value if value == MFVideoFormat_MJPG => "MJPEG",
        _ => return None,
    })
}

fn audio_codec(subtype: GUID) -> Option<&'static str> {
    Some(match subtype {
        value if value == MFAudioFormat_AAC => "AAC",
        value if value == MFAudioFormat_MP3 => "MP3",
        value if value == MFAudioFormat_MPEG => "MP2",
        value if value == MFAudioFormat_Dolby_AC3 || value == MFAudioFormat_Dolby_AC3_SPDIF => "AC-3",
        value if value == MFAudioFormat_Dolby_DDPlus => "E-AC-3",
        value if value == MFAudioFormat_Opus => "Opus",
        value if value == MFAudioFormat_Vorbis => "Vorbis",
        value if value == MFAudioFormat_FLAC => "FLAC",
        value if value == MFAudioFormat_PCM || value == MFAudioFormat_Float => "PCM",
        value if value == MFAudioFormat_DTS => "DTS",
        value if value == MFAudioFormat_WMAudioV8 || value == MFAudioFormat_WMAudioV9 => "WMA",
        value if value == MFAudioFormat_ALAC => "ALAC",
        _ => return None,
    })
}

/// Media Foundation must be started on this process. A stream that cannot be read
/// leaves its fields at zero.
pub(super) fn read(path: &Path) -> MediaInfo {
    let mut info = MediaInfo::default();
    // SAFETY: COM calls on owned interfaces; the reader is released when dropped.
    unsafe {
        let Ok(reader) = MFCreateSourceReaderFromURL(&HSTRING::from(path.as_os_str()), None) else {
            return info;
        };
        if let Ok(video) = reader.GetNativeMediaType(MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32, 0) {
            info.video_codec = video.GetGUID(&MF_MT_SUBTYPE).ok().and_then(video_codec).map_or(0, video_codec_index);
            if let Ok(rate) = video.GetUINT64(&MF_MT_FRAME_RATE) {
                let (numerator, denominator) = (rate >> 32, rate & 0xffff_ffff);
                if numerator > 0 && denominator > 0 {
                    info.frame_rate_milli = (numerator * 1000 / denominator).min(1_000_000) as u32;
                }
            }
        }
        if let Ok(audio) = reader.GetNativeMediaType(MF_SOURCE_READER_FIRST_AUDIO_STREAM.0 as u32, 0) {
            info.audio_codec = audio.GetGUID(&MF_MT_SUBTYPE).ok().and_then(audio_codec).map_or(0, audio_codec_index);
            info.sample_rate = audio.GetUINT32(&MF_MT_AUDIO_SAMPLES_PER_SECOND).unwrap_or(0).min(1_000_000);
            info.channels = audio.GetUINT32(&MF_MT_AUDIO_NUM_CHANNELS).unwrap_or(0).min(255) as u8;
        }
    }
    info
}
