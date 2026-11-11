use super::Viewer;
use crate::ui::theme::ThemeContext;
use gpui::{prelude::*, *};
use std::sync::Arc;

pub(super) struct ImageView {
    scale: f32,
    offset: Point<Pixels>,
    target: Bounds<Pixels>,
    drag: Option<Point<Pixels>>,
    zooming_until: Option<std::time::Instant>,
    /// Zoom relative to the image's real size, shown briefly after a change.
    zoom_label: Option<(u32, std::time::Instant)>,
    /// Fullscreen and detached state of the last frame: any change restarts the zoom.
    presentation: (bool, bool),
    /// Top of the label row, under the unzoomed image, from the last paint.
    labels_top: Pixels,
    /// Clock of the animation shown, restarted when the image changes.
    animation: Option<(ImageId, std::time::Instant)>,
}
const ZOOM_LABEL_TIME: std::time::Duration = std::time::Duration::from_millis(1200);
impl Default for ImageView {
    fn default() -> Self {
        Self { scale: 1.0, offset: Point::default(), target: Bounds::default(), drag: None, zooming_until: None, zoom_label: None, presentation: (false, false), labels_top: Pixels::ZERO, animation: None }
    }
}
impl Viewer {
    pub(super) fn toggle_preview_fullscreen(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if window.is_fullscreen() {
            self.exit_fullscreen(window, cx);
        } else if self.detached {
            window.toggle_fullscreen();
            window.focus(&self.focus, cx);
        } else {
            self.fullscreen_pending = true;
            self.fullscreen_temporary = true;
            self.detach(window, cx);
        }
        cx.notify();
    }

    pub(super) fn exit_fullscreen(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.fullscreen_temporary {
            super::media_overlay::release_surface_overlays(window);
            crate::ui::components::tool_window::close_transients(window, cx);
            window.remove_window();
        } else {
            window.toggle_fullscreen();
        }
        cx.notify();
    }

    /// The image with its technical labels just below it, which fullscreen leaves out.
    pub(super) fn render_image_with_labels(&mut self, image: Arc<RenderImage>, natural: Option<(u32, u32)>, fullscreen: bool, cx: &mut Context<Self>) -> AnyElement {
        let labels = (!fullscreen).then(|| {
            let extension = self.file.as_ref().and_then(|file| std::path::Path::new(&file.path).extension().map(|ext| ext.to_string_lossy().into_owned()));
            let labels = super::media_labels::image(&image, natural, extension.as_deref(), cx);
            let id: SharedString = format!("viewer-image-labels-{}", self.generation).into();
            super::media_labels::row(id, labels, cx)
        });
        self.render_interactive_image(image, fullscreen, labels, cx)
    }

    pub(super) fn render_interactive_image(&mut self, image: Arc<RenderImage>, fullscreen: bool, labels: Option<AnyElement>, cx: &mut Context<Self>) -> AnyElement {
        let presentation = (fullscreen, self.detached);
        if self.image_view.presentation != presentation {
            self.image_view = ImageView { presentation, ..ImageView::default() };
        }
        let handle = cx.entity().downgrade();
        let natural_width = image.size(0).width.0.max(1) as f32;
        let zoom_label = self.image_view.zoom_label
            .filter(|(_, until)| *until > std::time::Instant::now())
            .map(|(percent, _)| percent);
        let theme = *cx.theme();
        // Only fullscreen enlarges small images; windows show them at real size.
        let fit = if fullscreen { ObjectFit::Contain } else { ObjectFit::ScaleDown };
        let scale = self.image_view.scale;
        let offset = self.image_view.offset;
        let dragging = self.image_view.drag.is_some();
        let reserve = labels.is_some();
        let labels_top = self.image_view.labels_top;
        let animation = animation_clock(&mut self.image_view.animation, &image);
        let zooming = self.image_view.drag.is_none() && self.image_view.zooming_until.is_some_and(|until| until > std::time::Instant::now());
        div().id("viewer-image").size_full().relative().overflow_hidden()
            .when(self.image_view.drag.is_some(), |surface| surface.cursor(CursorStyle::ClosedHand))
            .on_scroll_wheel(cx.listener(move |this, event: &ScrollWheelEvent, window, cx| {
                let view = &mut this.image_view;
                if event.modifiers.control { return; }
                let delta = event.delta.pixel_delta(window.line_height()).y.as_f32();
                if !delta.is_finite() || delta == 0.0 { return; }
                let next = (view.scale * (delta * 0.002).exp()).clamp(1.0, 32.0);
                let ratio = next / view.scale;
                let percent = (view.target.size.width.as_f32() * ratio / natural_width * 100.0).round().max(1.0) as u32;
                view.zoom_label = Some((percent, std::time::Instant::now() + ZOOM_LABEL_TIME));
                let center = view.target.center();
                view.offset = anchored_offset(view.offset, event.position, center, ratio);
                view.scale = next;
                view.zooming_until = Some(std::time::Instant::now() + std::time::Duration::from_millis(180));
                if next == 1.0 { view.offset = Point::default(); }
                cx.spawn(async move |this, cx| {
                    cx.background_executor().timer(ZOOM_LABEL_TIME).await;
                    let _ = this.update(cx, |this, cx| {
                        if this.image_view.zoom_label.is_some_and(|(_, until)| until <= std::time::Instant::now()) {
                            this.image_view.zoom_label = None;
                            cx.notify();
                        }
                    });
                }).detach();
                cx.spawn(async move |this, cx| {
                    cx.background_executor().timer(std::time::Duration::from_millis(180)).await;
                    let _ = this.update(cx, |this, cx| {
                        if this.image_view.zooming_until.is_some_and(|until| until <= std::time::Instant::now()) {
                            this.image_view.zooming_until = None;
                            cx.notify();
                        }
                    });
                }).detach();
                cx.stop_propagation();
                cx.notify();
            }))
            .on_mouse_down(MouseButton::Left, cx.listener(|this, event: &MouseDownEvent, window, cx| {
                window.focus(&this.focus, cx);
                if event.click_count == 1 && this.image_view.scale > 1.0 && this.image_view.target.contains(&event.position) {
                    this.image_view.drag = Some(event.position);
                    cx.notify();
                }
            }))
            .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, _, cx| {
                if event.pressed_button != Some(MouseButton::Left) {
                    if this.image_view.drag.take().is_some() { cx.notify(); }
                    return;
                }
                if let Some(previous) = this.image_view.drag {
                    this.image_view.offset += event.position - previous;
                    this.image_view.drag = Some(event.position);
                    cx.notify();
                }
            }))
            .on_mouse_up(MouseButton::Left, cx.listener(|this, _, _, cx| { this.image_view.drag = None; cx.notify(); }))
            .on_mouse_up_out(MouseButton::Left, cx.listener(|this, _, _, cx| { this.image_view.drag = None; cx.notify(); }))
            .child(canvas(|_, _, _| (), move |bounds, _, window, cx| {
                use raw_window_handle::HasWindowHandle;
                if let Ok(handle) = HasWindowHandle::window_handle(window) {
                    oxyde_lib::platform::platform().viewer_image_cursor(handle.as_raw(), zooming, dragging);
                }

                // The image and the label row below it are centered together.
                let row = if reserve { super::media_labels::ROW_HEIGHT.to_pixels(window.rem_size()) } else { Pixels::ZERO };
                let available = Bounds::new(bounds.origin, size(bounds.size.width, (bounds.size.height - row).max(Pixels::ZERO)));
                let base = fit.get_bounds(available, image.size(0));
                // Zoomed, the row waits at the bottom and the image stays above it.
                let top = if scale > 1.0 { available.bottom() } else { base.bottom() } - bounds.top();
                let dimensions = size(base.size.width * scale, base.size.height * scale);
                let target = Bounds::new(base.center() - point(dimensions.width / 2.0, dimensions.height / 2.0) + offset, dimensions);
                let _ = handle.update(cx, |this, cx| {
                    this.image_view.target = target;
                    if reserve && this.image_view.labels_top != top {
                        this.image_view.labels_top = top;
                        cx.notify();
                    }
                    this.set_overlay_media(target, cx);
                });
                let frame = animation.map_or(0, |started| animation_frame(&image, started.elapsed()));
                if animation.is_some() && !cx.reduce_motion() && window.is_visible() {
                    window.request_animation_frame();
                }
                window.with_content_mask(Some(ContentMask { bounds: available }), |window| {
                    let _ = window.paint_image(available, target, Corners::default(), image.clone(), frame, false);
                });
            }).absolute().inset_0().size_full())
            .when_some(labels, |surface, labels| {
                surface.child(div().absolute().left_0().right_0().top(labels_top)
                    .flex().justify_center().px(rems(0.5)).pt(rems(0.375)).child(labels))
            })
            .when_some(zoom_label, |surface, percent| {
                surface.child(div().absolute().bottom(rems(if reserve { 3.0 } else { 1.0 })).left_0().right_0().flex().justify_center()
                    .child(div().px(rems(0.625)).py(rems(0.25)).rounded_md().bg(theme.surface)
                        .border(rems(0.0625)).border_color(theme.border).text_color(theme.text)
                        .text_size(rems(0.8125)).child(format!("{percent}%"))))
            })
            .into_any_element()
    }
}

fn anchored_offset(offset: Point<Pixels>, cursor: Point<Pixels>, center: Point<Pixels>, ratio: f32) -> Point<Pixels> {
    offset + (cursor - center) * (1.0 - ratio)
}

#[cfg(test)]
#[path = "tests/image_interaction.rs"]
mod tests;
/// Start of the animation clock, kept while the same multi-frame image is shown.
fn animation_clock(clock: &mut Option<(ImageId, std::time::Instant)>, image: &RenderImage) -> Option<std::time::Instant> {
    if image.frame_count() < 2 {
        *clock = None;
        return None;
    }
    match clock {
        Some((id, started)) if *id == image.id => Some(*started),
        _ => {
            let started = std::time::Instant::now();
            *clock = Some((image.id, started));
            Some(started)
        }
    }
}

/// The frame shown after `elapsed`, looping over the frames' own delays.
fn animation_frame(image: &RenderImage, elapsed: std::time::Duration) -> usize {
    let delays: Vec<u128> = (0..image.frame_count())
        .map(|index| std::time::Duration::from(image.delay(index)).as_millis().max(1))
        .collect();
    let mut remaining = elapsed.as_millis() % delays.iter().sum::<u128>();
    for (index, delay) in delays.iter().enumerate() {
        if remaining < *delay {
            return index;
        }
        remaining -= delay;
    }
    0
}
