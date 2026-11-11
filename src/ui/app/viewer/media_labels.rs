//! Technical labels under the image, audio and video players.
use crate::ui::{
    i18n::{FluentArgs, I18nContext, Message},
    theme::ThemeContext,
};
use gpui::{prelude::*, *};
use oxyde_lib::models::preferences::Language;
use oxyde_lib::platform::MediaInfo;

/// Least height of the label row under the video.
pub(super) const ROW_HEIGHT: Rems = Rems(2.0);

/// Fades in once `labels` holds everything known about the file; `id` names
/// the file so each one starts hidden.
pub(super) fn row(id: SharedString, labels: Vec<SharedString>, cx: &App) -> AnyElement {
    if labels.is_empty() {
        return div().into_any_element();
    }
    let theme = *cx.theme();
    div()
        .w_full()
        .flex()
        .flex_wrap()
        .justify_center()
        .gap(rems(0.375))
        .children(labels.into_iter().map(move |label| {
            div()
                .px(rems(0.5))
                .py(rems(0.125))
                .rounded(rems(0.375))
                .border(rems(0.0625))
                .border_color(theme.border)
                .bg(theme.surface)
                .text_size(rems(0.75))
                .text_color(theme.text_muted)
                .child(label)
        }))
        .with_spring(id, crate::ui::elements::motion::presence(true).from(0.0), |row, progress| row.opacity(progress))
        .into_any_element()
}

/// Names the stream no installed decoder plays, with its codec when known.
pub(super) fn missing_codec(video: bool, info: &MediaInfo, cx: &App) -> SharedString {
    let (message, codec) = if video {
        (Message::ViewerVideoCodecMissing, info.video_codec_name())
    } else {
        (Message::ViewerAudioCodecMissing, info.audio_codec_name())
    };
    let mut args = FluentArgs::new();
    args.set("codec", codec.unwrap_or("unknown"));
    cx.i18n().text_with_args(message, &args)
}

/// What cannot be played and how to fix it, in place of the picture or of the file.
pub(super) fn missing_codec_notice(info: &MediaInfo, cx: &App) -> Div {
    let lines = [(info.video_decoder_missing(), true), (info.audio_decoder_missing(), false)];
    div()
        .flex()
        .flex_col()
        .items_center()
        .gap(rems(0.5))
        .text_center()
        .min_w_0()
        .max_w_full()
        .children(lines.into_iter().filter(|(missing, _)| *missing).map(|(_, video)| super::content_render::wrapping(missing_codec(video, info, cx))))
        .child(super::content_render::wrapping(cx.i18n().text(Message::ViewerCodecMissingHint)).text_size(rems(0.75)).opacity(0.75))
}

/// `value` with at most `decimals` digits, trailing zeros removed, in the UI's notation.
fn decimal(value: f64, decimals: usize, cx: &App) -> String {
    let text = format!("{value:.decimals$}");
    let text = if text.contains('.') { text.trim_end_matches('0').trim_end_matches('.') } else { &text };
    if cx.i18n().language() == Language::English { text.to_owned() } else { text.replace('.', ",") }
}

pub(super) fn sample_rate(hertz: u32, cx: &App) -> SharedString {
    format!("{} kHz", decimal(f64::from(hertz) / 1000.0, 1, cx)).into()
}

pub(super) fn channels(count: u8, cx: &App) -> SharedString {
    let i18n = cx.i18n();
    match count {
        1 => i18n.text(Message::AudioMono),
        2 => i18n.text(Message::AudioStereo),
        count => {
            let mut args = FluentArgs::new();
            args.set("count", count);
            i18n.text_with_args(Message::AudioChannels, &args)
        }
    }
}

/// The usual name of a picture size, by its larger side as for widescreen films.
pub(super) fn definition(width: u32, height: u32) -> Option<&'static str> {
    let (long, short) = (width.max(height), width.min(height));
    Some(match () {
        _ if long >= 7000 || short >= 4000 => "8K",
        _ if long >= 3600 || short >= 2000 => "4K",
        _ if long >= 2400 || short >= 1400 => "1440p",
        _ if long >= 1800 || short >= 1000 => "1080p",
        _ if long >= 1200 || short >= 700 => "720p",
        _ if short >= 560 => "576p",
        _ if short >= 460 => "480p",
        _ => return None,
    })
}

pub(super) fn video(dimensions: (u32, u32), info: Option<MediaInfo>, cx: &App) -> Vec<SharedString> {
    let mut labels: Vec<SharedString> = Vec::new();
    let (width, height) = dimensions;
    // Nothing until the streams are described too, so the row appears whole.
    let Some(info) = info.filter(|_| width > 0 && height > 0) else { return labels };
    {
        labels.extend(definition(width, height).map(SharedString::from));
        labels.push(format!("{width} × {height}").into());
    }
    labels.extend(info.video_codec_name().map(SharedString::from));
    if info.frame_rate_milli > 0 {
        let mut args = FluentArgs::new();
        args.set("rate", decimal(f64::from(info.frame_rate_milli) / 1000.0, 3, cx));
        labels.push(cx.i18n().text_with_args(Message::VideoFrameRate, &args));
    }
    if info.audio_decoder_missing() {
        labels.push(missing_codec(false, &info, cx));
    } else {
        labels.extend(info.audio_codec_name().map(SharedString::from));
    }
    if info.sample_rate > 0 {
        labels.push(sample_rate(info.sample_rate, cx));
    }
    if info.channels > 0 {
        labels.push(channels(info.channels, cx));
    }
    labels
}

/// Format, size in pixels, megapixels and frame count of a picture.
pub(super) fn image(image: &RenderImage, natural: Option<(u32, u32)>, extension: Option<&str>, cx: &App) -> Vec<SharedString> {
    let mut labels: Vec<SharedString> = Vec::new();
    labels.extend(extension.filter(|ext| !ext.is_empty()).map(|ext| ext.to_uppercase().into()));
    let raster = image.size(0);
    let (width, height) = natural.unwrap_or((raster.width.0.max(0) as u32, raster.height.0.max(0) as u32));
    if width > 0 && height > 0 {
        labels.push(format!("{width} × {height}").into());
        let megapixels = f64::from(width) * f64::from(height) / 1_000_000.0;
        if megapixels >= 0.1 {
            let mut args = FluentArgs::new();
            args.set("value", decimal(megapixels, 1, cx));
            labels.push(cx.i18n().text_with_args(Message::ImageMegapixels, &args));
        }
    }
    let frames = image.frame_count();
    if frames > 1 {
        let mut args = FluentArgs::new();
        args.set("count", frames);
        labels.push(cx.i18n().text_with_args(Message::ImageFrames, &args));
    }
    labels
}

#[cfg(test)]
#[path = "tests/media_labels.rs"]
mod tests;
