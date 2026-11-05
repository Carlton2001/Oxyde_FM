use super::*;

#[test]
fn media_info_carries_missing_decoders_and_rejects_unknown_flags() {
    let info = MediaInfo {
        video_codec: video_codec_index("HEVC"),
        audio_codec: audio_codec_index("AAC"),
        frame_rate_milli: 29_970,
        sample_rate: 48_000,
        channels: 2,
        missing_decoders: MISSING_VIDEO,
    };
    let decoded = decode_media_info(&encode_media_info(&info)).unwrap();
    assert_eq!(decoded, info);
    assert!(decoded.video_decoder_missing() && !decoded.audio_decoder_missing());

    let mut report = encode_media_info(&info);
    report[12] = 4;
    assert!(decode_media_info(&report).is_none());
}
