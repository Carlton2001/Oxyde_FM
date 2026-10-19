use super::assets::{retain_tooltip_asset, TooltipAssetSource, TooltipImageAsset, TooltipPreviewAsset};
use super::video::{send_video_frame, CancelVideoPreview, VideoClock};
use super::{PreviewData, PreviewTask};
use crate::ui::components::icons::PreviewType;
use crate::ui::i18n::{I18nContext, Message};
use crate::ui::scale::rem_pixels;
use crate::ui::theme::OxydeTheme;
use gpui::prelude::FluentBuilder;
use gpui::*;
use std::sync::Arc;
use std::time::Duration;

const PREVIEW_LOADING_DELAY: Duration = Duration::from_secs(1);
/// One height envelope for every preview type; images are never enlarged.
const PREVIEW_MAXIMUM_HEIGHT: Rems = rems(15.0);
/// Preferred width of a text excerpt; the bubble may still be narrower.
pub(super) const TEXT_PREVIEW_WIDTH: Rems = rems(20.0);

impl PreviewTask {
    pub(super) fn may_have_cached_pixels(&self) -> bool {
        if let Some(path) = &self.audio_source {
            return crate::ui::audio_covers::may_have_cached(path);
        }
        if let Some(path) = &self.original_image {
            return super::image_cache::may_have_cached(path);
        }
        self.media_asset.as_ref().is_some_and(|path| super::pdf_cache::may_have_cached(path))
    }

    /// A vector source keeps its intrinsic size; its larger raster only keeps it sharp.
    pub(super) fn preferred_size(&self, image: &RenderImage, natural_size: Option<(u32, u32)>, maximum_width: Pixels) -> Size<Pixels> {
        let height = rem_pixels(PREVIEW_MAXIMUM_HEIGHT);
        let (width, natural_height) = natural_size.map(|(width, height)| (width as f32, height as f32))
            .unwrap_or_else(|| { let size = image.size(0); (size.width.0 as f32, size.height.0 as f32) });
        fit_media_size(width, natural_height, maximum_width, height)
    }
    pub(super) fn cached_for_handover(&self, cx: &mut App) -> bool {
        if self.audio_source.is_some() { return self.audio_cached; }
        if let Some(source) = &self.media_asset {
            return cx.fetch_asset::<TooltipPreviewAsset>(source).is_some_and(|result| matches!(result, Ok((_, true))));
        }
        if let Some(source) = &self.original_image {
            return cx.fetch_asset::<TooltipImageAsset>(source).is_some_and(|result| matches!(result, Ok((_, true))));
        }
        false
    }

    pub(super) fn ready_for_handover(&self, cx: &mut App) -> bool {
        if self.transition_image.is_some() {
            return self.ready_for_initial_display(cx);
        }
        // Video players share one admission slot: keeping the old player alive
        // would prevent its replacement from producing a first frame.
        if self._video_cancellation.is_some() { return true; }
        // Keep the old, internally consistent bubble until the replacement has
        // content or a terminal message. The existing loading deadline ensures
        // that a slow preview does not leave the old filename visible forever.
        if self.loading_delay_elapsed { return true; }
        self.ready_for_initial_display(cx)
    }

    /// Initial publication waits for content or a terminal error, including
    /// the first video frame. A loading deadline is not a completed preview.
    pub(super) fn ready_for_initial_display(&self, cx: &mut App) -> bool {
        if let Some(source) = &self.media_asset {
            return cx.fetch_asset::<TooltipPreviewAsset>(source).is_some();
        }
        if let Some(source) = &self.original_image {
            return cx.fetch_asset::<TooltipImageAsset>(source).is_some();
        }
        self.data.is_some() || self.message.is_some()
    }
    /// Read cached pixels from an event handler without subscribing a rendered
    /// view. Window::use_asset requires an active GPUI rendering phase.
    pub(super) fn snapshot_image(&self, cx: &mut App) -> Option<super::PreviewPixels> {
        if let Some(image) = &self.transition_image {
            return Some(image.clone());
        }
        if let Some(source) = &self.media_asset {
            return cx.fetch_asset::<TooltipPreviewAsset>(source)
                .and_then(Result::ok).map(|(image, _)| (image, None));
        }
        if let Some(source) = &self.original_image {
            return cx.fetch_asset::<TooltipImageAsset>(source)
                .and_then(Result::ok)
                .map(|(preview, _)| (preview.image, preview.natural_size));
        }
        match &self.data {
            Some(PreviewData::Image(image)) => Some((image.clone(), None)),
            _ => None,
        }
    }

    /// Render-only: subscribes the current view to asset completion.
    /// The loaded raster and, for a vector source, its intrinsic size.
    pub(super) fn loaded_image(&self, window: &mut Window, cx: &mut App) -> Option<(Arc<RenderImage>, Option<(u32, u32)>)> {
        if let Some(image) = &self.transition_image {
            return Some(image.clone());
        }
        if let Some(source) = &self.media_asset {
            return window
                .use_asset::<TooltipPreviewAsset>(source, cx)
                .and_then(Result::ok).map(|(image, _)| (image, None));
        }
        if let Some(source) = &self.original_image {
            return window
                .use_asset::<TooltipImageAsset>(source, cx)
                .and_then(Result::ok)
                .map(|(preview, _)| (preview.image, preview.natural_size));
        }
        match &self.data {
            Some(PreviewData::Image(image)) => Some((image.clone(), None)),
            _ => None,
        }
    }
    pub(super) fn preview_message(&self, window: &mut Window, cx: &mut App) -> Option<Message> {
        if let Some(source) = &self.original_image {
            return window.use_asset::<TooltipImageAsset>(source, cx).and_then(
                |result| match result {
                    Ok((preview, _)) => preview.notice,
                    Err(error) => Some(error.message()),
                },
            );
        }
        if let Some(source) = &self.media_asset {
            return window
                .use_asset::<TooltipPreviewAsset>(source, cx)
                .and_then(|result| result.err().map(|error| crate::ui::image_preview::worker_message(&format!("{error:?}"))));
        }
        self.message
    }
    pub(super) fn new(path: String, preview_type: PreviewType, cx: &mut Context<Self>) -> Self {
        // The task holds only a weak entity reference: closing the tooltip
        // cannot bring its loading message back after the delay.
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(PREVIEW_LOADING_DELAY).await;
            let _ = this.update(cx, |this, cx| {
                this.loading_delay_elapsed = true;
                if this.data.is_none() && this.message.is_none() {
                    cx.notify();
                }
            });
        })
        .detach();
        if preview_type == PreviewType::Image {
            let path = std::path::PathBuf::from(path);
            retain_tooltip_asset(TooltipAssetSource::Image(path.clone()), cx);
            return Self {
                transition_image: None,
                viewer: false,
                maximum_size: None,
                data: None,
                media_asset: None,
                original_image: Some(path),
                audio_source: None,
                audio_cached: false,
                message: None,
                loading_delay_elapsed: false,
                _video_cancellation: None,
            };
        }
        if preview_type == PreviewType::Pdf {
            let path = SharedString::from(path);
            retain_tooltip_asset(TooltipAssetSource::Pdf(path.clone()), cx);
            return Self {
                transition_image: None,
                viewer: false,
                maximum_size: None,
                data: None,
                media_asset: Some(path),
                original_image: None,
                audio_source: None,
                audio_cached: false,
                message: None,
                loading_delay_elapsed: false,
                _video_cancellation: None,
            };
        }
        let cancellation = crate::ui::preview_jobs::Cancellation::default();
        let audio_source = (preview_type == PreviewType::Audio).then(|| path.clone());
        let video_cancellation = (preview_type == PreviewType::Video)
            .then(|| Arc::new(CancelVideoPreview(cancellation.clone())));
        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let mut cx = cx.clone();
            async move {
                if preview_type == PreviewType::Video {
                    // Channel size 4: gives the decode thread a small buffer so it is never
                    // starved while the async side is sleeping, without letting it run
                    // arbitrarily far ahead.
                    let (tx, mut rx) =
                        tokio::sync::mpsc::channel::<(Arc<RenderImage>, Duration)>(4);

                    let path_clone = path.clone();
                    let playback_cancellation = cancellation.clone();
                    let (completion_tx,completion_rx) = tokio::sync::oneshot::channel();
                    let failure_cancellation = playback_cancellation.clone();
                    let decode =
                        crate::ui::preview_jobs::load_video(cancellation.clone(), move || {
                            let receiver = tx.clone();
                            let send_cancellation = cancellation.clone();
                            ::oxyde_lib::platform::platform().video_frames(
                                path_clone,
                                Box::new(move |rgba, pts| {
                                    let frame =
                                        Arc::new(RenderImage::new(vec![image::Frame::new(rgba)]));
                                    if let Err(error) = crate::ui::preview_budget::admit(&frame) {
                                        log::warn!("Could not admit video preview pixels: {error}");
                                        return false;
                                    }
                                    send_video_frame(&tx, (frame, pts), &send_cancellation)
                                }),
                                Box::new(move || {
                                    cancellation.is_cancelled() || receiver.is_closed()
                                }),
                            )
                        });
                    cx.background_executor()
                        .spawn(async move {
                            let failure = decode.await.err().map(|error| {
                                log::warn!("Could not stream tooltip video preview: {error}");
                                crate::ui::image_preview::worker_message(&error)
                            });
                            if failure.is_some() { failure_cancellation.cancel(); }
                            let _ = completion_tx.send(failure);
                        })
                        .detach();

                    let mut clock = VideoClock::default();

                    while let Some((frame, pts)) = rx.recv().await {
                        let now = std::time::Instant::now();
                        let Some(due) = clock.presentation_time(pts, now) else {
                            playback_cancellation.cancel();
                            break;
                        };
                        // Wait BEFORE publishing the frame: otherwise a two-frame
                        // clip presents both colors in one UI frame and looks fixed.
                        if let Some(delay) = due.checked_duration_since(now) {
                            let timer = Box::pin(cx.background_executor().timer(delay));
                            let cancelled = Box::pin(playback_cancellation.cancelled());
                            if matches!(futures::future::select(timer,cancelled).await, futures::future::Either::Right(_)) { break; }
                        }
                        let res = this.update(&mut cx, |this, cx| {
                            this.data = Some(PreviewData::Image(frame));
                            cx.notify();
                        });
                        if res.is_err() {
                            playback_cancellation.cancel();
                            break;
                        }
                    }
                    let failure = completion_rx.await.ok().flatten();
                    let _ = this.update(&mut cx, |this, cx| {
                        if failure.is_some() || this.data.is_none() {
                            this.message = failure.or(Some(Message::PreviewUnavailable));
                            cx.notify();
                        }
                    });
                    return;
                }

                let mut failure_message = None;
                let mut audio_cached = false;
                let preview = match preview_type {
                    PreviewType::Audio => {
                        match crate::ui::preview_jobs::load_hover(move || crate::ui::audio_covers::load(&path)).await {
                            Ok((image, cached)) => {
                                audio_cached = cached;
                                Some(PreviewData::Image(image))
                            },
                            Err(error) => {
                                log::debug!("Audio tooltip cover unavailable: {error}");
                                None
                            }
                        }
                    }
                    PreviewType::Image | PreviewType::Office | PreviewType::Pdf => {
                        let rgba_img = match preview_type {
                            PreviewType::Image => {
                                let path = path.clone();
                                match crate::ui::preview_jobs::load_hover(move || {
                                    ::oxyde_lib::platform::platform().thumbnail(path)
                                })
                                .await
                                {
                                    Ok(image) => Some(image),
                                    Err(error) => {
                                        failure_message = Some(crate::ui::image_preview::worker_message(&error));
                                        log::warn!("Could not load tooltip image preview: {error}");
                                        None
                                    }
                                }
                            }
                            PreviewType::Office => {
                                let path = path.clone();
                                match crate::ui::preview_jobs::load_hover(move || {
                                    ::oxyde_lib::platform::platform().office_thumbnail(path)
                                })
                                .await
                                {
                                    Ok(image) => Some(image),
                                    Err(error) => {
                                        failure_message = Some(crate::ui::image_preview::worker_message(&error));
                                        log::warn!(
                                            "Could not load tooltip office preview: {error}"
                                        );
                                        None
                                    }
                                }
                            }
                            PreviewType::Pdf => {
                                let path = path.clone();
                                match crate::ui::preview_jobs::load_hover(move || {
                                    ::oxyde_lib::platform::platform().pdf_thumbnail(path)
                                })
                                .await
                                {
                                    Ok(image) => Some(image),
                                    Err(error) => {
                                        failure_message = Some(crate::ui::image_preview::worker_message(&error));
                                        log::warn!("Could not load tooltip PDF preview: {error}");
                                        None
                                    }
                                }
                            }
                            _ => None,
                        };

                        if let Some(rgba) = rgba_img {
                            // Backend now consistently returns BGRA for all types
                            let frame = image::Frame::new(rgba);
                            let image = Arc::new(RenderImage::new(vec![frame]));
                            match crate::ui::preview_budget::admit(&image) {
                                Ok(()) => Some(PreviewData::Image(image)),
                                Err(error) => {
                                    log::warn!("Could not admit tooltip pixels: {error}");
                                    failure_message = Some(crate::ui::i18n::Message::PreviewUnavailable);
                                    None
                                }
                            }
                        } else if preview_type == PreviewType::Office {
                            // Fallback to text for Office if no image
                            let path_for_text = path.clone();
                            let content = match crate::ui::preview_jobs::load_hover(move || {
                                ::oxyde_lib::platform::platform().office_text_preview(path_for_text)
                            })
                            .await
                            {
                                Ok(content) => Some(content),
                                Err(error) => {
                                    failure_message = Some(crate::ui::image_preview::worker_message(&error));
                                    log::warn!(
                                        "Could not load tooltip office text preview: {error}"
                                    );
                                    None
                                }
                            };
                            if let Some(content) = content {
                                let snippet =
                                    content.lines().take(15).collect::<Vec<_>>().join("\n");
                                Some(PreviewData::Text(snippet))
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    }
                    PreviewType::Text => {
                        let content = crate::ui::preview_jobs::load_hover(move || {
                            ::oxyde_lib::utils::preview::read_text_preview(std::path::Path::new(
                                &path,
                            ))
                            .map_err(|error| {
                                ::oxyde_lib::models::CommandError::IoError(error.to_string())
                            })
                        })
                        .await;
                        if let Ok(content) = content {
                            Some(PreviewData::Text(content))
                        } else {
                            None
                        }
                    }
                    _ => None,
                };

                let _ = this.update(&mut cx, |this, cx| {
                    this.message = if preview.is_none() { failure_message.or(Some(Message::PreviewUnavailable)) } else { None };
                    this.data = preview;
                    this.audio_cached = audio_cached;
                    cx.notify();
                });
            }
        })
        .detach();

        Self {
            maximum_size: None,
            data: None,
            media_asset: None,
            original_image: None,
            audio_source,
            audio_cached: false,
            message: None,
            loading_delay_elapsed: false,
            transition_image: None,
            viewer: false,
            _video_cancellation: video_cancellation,
        }
    }
}

fn fit_media_size(width: f32, height: f32, maximum_width: Pixels, maximum_height: Pixels) -> Size<Pixels> {
    let scale = (maximum_width.as_f32() / width.max(1.0))
        .min(maximum_height.as_f32() / height.max(1.0)).min(1.0);
    size(px(width * scale), px(height * scale))
}

#[cfg(test)]
#[path = "tests/media_size.rs"]
mod media_size_tests;

impl Render for PreviewTask {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.viewer { return self.render_viewer(window, cx); }
        let theme = *OxydeTheme::global(cx);
        if let Some((image, natural_size)) = self.loaded_image(window, cx) {
            let maximum = self.maximum_size.map(|limit| limit.width).unwrap_or_else(||
                rem_pixels(rems(48.375))
                    .min((window.viewport_size().width - rem_pixels(rems(3.625))).max(px(1.0))));
            let mut size = self.preferred_size(&image, natural_size, maximum);
            if let Some(limit) = self.maximum_size {
                let height = (limit.height - rem_pixels(rems(2.0))).max(px(1.0));
                let scale = (limit.width / size.width).min(height / size.height).min(1.0);
                size.width *= scale;
                size.height *= scale;
            }
            let notice = if self.transition_image.is_some() { None } else { self.preview_message(window, cx) };
            return div()
                .w_full()
                .flex()
                .flex_col()
                .gap(rems(0.375))
                .child(
                    img(image)
                        // GPUI retains the animation clock only for a named image.
                        .id("tooltip-preview-image")
                        .w(size.width)
                        .h(size.height)
                        .opacity(if self.transition_image.is_some() { 0.15 } else { 1.0 })
                        .grayscale(self.transition_image.is_some())
                        .object_fit(ObjectFit::Contain),
                )
                .when_some(notice, |this, notice| {
                    this.child(
                        div()
                            .text_size(rems(0.625))
                            .text_color(theme.text_muted)
                            .child(cx.i18n().text(notice)),
                    )
                });
        }
        if let Some(preview) = &self.data {
            div().w_full().flex().child(match preview {
                PreviewData::Image(render_img) => img(render_img.clone())
                    .w(rems(25.0))
                    .max_h(rems(23.75))
                    .object_fit(ObjectFit::Contain)
                    .into_any_element(),
                PreviewData::Text(t) => div()
                    .w_full()
                    .max_h(self.maximum_size.map(|limit| limit.height).unwrap_or_else(|| rem_pixels(PREVIEW_MAXIMUM_HEIGHT)).min(rem_pixels(PREVIEW_MAXIMUM_HEIGHT)))
                    .overflow_hidden()
                    .rounded(rems(0.25))
                    .bg(theme.sidebar)
                    .border(rems(0.0625))
                    .border_color(theme.border)
                    .px(rems(0.625))
                    .py(rems(0.5))
                    .text_size(rems(0.625))
                    .font_family("JetBrains Mono")
                    .text_color(theme.text)
                    .child(t.clone())
                    .into_any_element(),
            })
        } else {
            let message = self
                .preview_message(window, cx)
                .or_else(|| self.loading_delay_elapsed.then_some(Message::PreviewLoading));
            div()
                .text_size(rems(0.625))
                .text_color(theme.text_muted)
                .when_some(message, |this, message| this.child(cx.i18n().text(message)))
        }
    }
}
