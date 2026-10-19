//! Audio presentation owns no file I/O; stale results use the normal content generation.
use super::{content::Content, Viewer};
use crate::ui::{
    components::scroll_area::scroll_area,
    i18n::{I18nContext, Message},
    theme::ThemeContext,
};
use gpui::{prelude::*, *};
use std::sync::Arc;

pub(super) struct Audio {
    tags: Vec<(String, String)>,
    pub(super) cover: Option<Arc<RenderImage>>,
}

impl Viewer {
    pub(super) fn load_audio(&mut self, path: String, cx: &mut Context<Self>) {
        let wanted = self.wanted.clone();
        let generation = self.generation;
        let work = crate::ui::preview_jobs::load_viewer(move || {
            if wanted.load(std::sync::atomic::Ordering::Acquire) != generation {
                return Err("Audio no longer shown".to_owned());
            }
            // Metadata failure must not affect playback or replace the music fallback.
            let metadata = oxyde_lib::platform::platform()
                .audio_metadata(std::path::Path::new(&path))
                .unwrap_or_else(|error| {
                    log::warn!("Audio metadata unavailable: {error}");
                    Default::default()
                });
            let cover = metadata.cover.and_then(|cover| {
                image::RgbaImage::from_raw(cover.width, cover.height, cover.pixels)
                    .map(|image| crate::ui::audio_covers::share(Arc::new(RenderImage::new(vec![image::Frame::new(image)]))))
            });
            Ok(Audio {
                tags: metadata.tags,
                cover,
            })
        });
        self.finish(generation, work, Content::Audio, cx);
    }

    pub(super) fn render_audio(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let theme = *cx.theme();
        let audio = match &self.content {
            Content::Audio(audio) => Some(audio),
            _ => None,
        };
        let value = |keys: &[&str]| audio.and_then(|audio| audio.value(keys));
        let title = value(&["TrackTitle"])
            .or_else(|| self.files.get(self.index).map(|file| file.name.as_str()))
            .unwrap_or("—");
        let track = match (value(&["TrackNumber"]), value(&["TrackTotal"])) {
            (Some(number), Some(total)) => format!("{number} / {total}"),
            (number, _) => number.unwrap_or("—").to_owned(),
        };
        let date = value(&["Year", "RecordingDate", "ReleaseDate"]).unwrap_or("—");
        let year = date
            .get(..4)
            .filter(|year| year.bytes().all(|byte| byte.is_ascii_digit()))
            .unwrap_or(date);
        let cover = div()
            .w(rems(16.0))
            .max_w_full()
            .aspect_ratio(1.0)
            .flex()
            .items_center()
            .justify_center()
            .when_some(
                audio.and_then(|audio| audio.cover.clone()),
                |frame, cover| frame.child(img(cover).size_full().object_fit(ObjectFit::Contain)),
            )
            .when(audio.is_none_or(|audio| audio.cover.is_none()), |frame| {
                frame.child(
                    svg()
                        .path("icons/music.svg")
                        .size(rems(5.0))
                        .text_color(theme.text_muted),
                )
            });
        let card = div()
            .w_full()
            .flex_none()
            .overflow_hidden()
            .rounded_lg()
            .border(rems(0.0625))
            .border_color(theme.border)
            .bg(theme.surface)
            .shadow_sm()
            .child(
                div()
                    .w_full()
                    .flex()
                    .justify_center()
                    .p(rems(1.5))
                    .bg(theme.sidebar)
                    .child(cover),
            )
            .child(
                div()
                    .w_full()
                    .flex()
                    .flex_col()
                    .items_center()
                    .text_center()
                    .px(rems(1.0))
                    .py(rems(1.0))
                    .gap(rems(0.375))
                    .border_t(rems(0.0625))
                    .border_color(theme.border)
                    .child(
                        div()
                            .w_full()
                            .whitespace_normal()
                            .text_size(rems(1.25))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(title.to_owned()),
                    ),
            )
            .child(
                div()
                    .w_full()
                    .flex()
                    .border_t(rems(0.0625))
                    .border_color(theme.border)
                    .child(audio_cell(
                        Message::AudioAlbum,
                        value(&["AlbumTitle"]).unwrap_or("—").to_owned(),
                        true,
                        cx,
                    ))
                    .child(audio_cell(Message::AudioTrack, track, true, cx))
                    .child(audio_cell(Message::AudioYear, year.to_owned(), false, cx)),
            );
        let mut player = div()
            .w_full()
            .max_w(rems(32.0))
            .flex_none()
            .flex()
            .flex_col()
            .gap(rems(0.75))
            .my_auto()
            .mx_auto()
            .child(card)
            .when(self.native_media, |player| {
                player.child(
                    div()
                        .w_full()
                        .rounded_lg()
                        .overflow_hidden()
                        .border(rems(0.0625))
                        .border_color(theme.border)
                        .child(self.render_media_bar(cx).border_t_0()),
                )
            });
        if let Some(audio) = audio {
            if audio.tags.is_empty() {
                player = player.child(
                    div()
                        .text_center()
                        .text_color(theme.text_muted)
                        .child(cx.i18n().text(Message::AudioNoTags)),
                );
            }
        } else if matches!(self.content, Content::Loading) {
            player = player.child(
                div()
                    .text_center()
                    .text_color(theme.text_muted)
                    .child(cx.i18n().text(Message::ViewerLoading)),
            );
        }
        scroll_area(
            ("viewer-audio-scroll", self.generation),
            div().size_full(),
            div()
                .id("viewer-audio-tags")
                .size_full()
                .flex()
                .flex_col()
                .p(rems(1.5))
                .child(player),
            false,
            window,
            cx,
        )
        .into_any_element()
    }
}

impl Audio {
    fn value(&self, keys: &[&str]) -> Option<&str> {
        keys.iter().find_map(|key| {
            self.tags
                .iter()
                .find(|(tag, _)| tag == key)
                .map(|(_, value)| value.as_str())
        })
    }
}

fn audio_cell(label: Message, value: String, separator: bool, cx: &App) -> Div {
    let theme = *cx.theme();
    div()
        .flex_1()
        .min_w_0()
        .flex()
        .flex_col()
        .items_center()
        .text_center()
        .gap(rems(0.375))
        .px(rems(0.75))
        .py(rems(0.875))
        .when(separator, |cell| {
            cell.border_r(rems(0.0625)).border_color(theme.border)
        })
        .child(
            div()
                .text_size(rems(0.6875))
                .text_color(theme.text_muted)
                .child(cx.i18n().text(label)),
        )
        .child(div().w_full().whitespace_normal().child(value))
}
