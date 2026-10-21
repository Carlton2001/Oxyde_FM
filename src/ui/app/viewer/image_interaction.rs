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
}
const ZOOM_LABEL_TIME: std::time::Duration = std::time::Duration::from_millis(1200);
impl Default for ImageView {
    fn default() -> Self {
        Self { scale: 1.0, offset: Point::default(), target: Bounds::default(), drag: None, zooming_until: None, zoom_label: None, presentation: (false, false) }
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

    pub(super) fn render_interactive_image(&mut self, image: Arc<RenderImage>, fullscreen: bool, cx: &mut Context<Self>) -> AnyElement {
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
        let zooming = self.image_view.drag.is_none() && self.image_view.zooming_until.is_some_and(|until| until > std::time::Instant::now());
        div().id("viewer-image").size_full().relative().overflow_hidden()
            .when(self.image_view.drag.is_some(), |surface| surface.cursor(CursorStyle::ClosedHand))
            .on_scroll_wheel(cx.listener(move |this, event: &ScrollWheelEvent, window, cx| {
                let view = &mut this.image_view;
                if !view.target.contains(&event.position) || event.modifiers.control { return; }
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

                let base = fit.get_bounds(bounds, image.size(0));
                let dimensions = size(base.size.width * scale, base.size.height * scale);
                let target = Bounds::new(base.center() - point(dimensions.width / 2.0, dimensions.height / 2.0) + offset, dimensions);
                let _ = handle.update(cx, |this, cx| {
                    this.image_view.target = target;
                    this.set_overlay_media(target, cx);
                });
                let _ = window.paint_image(bounds, target, Corners::default(), image.clone(), 0, false);
            }).absolute().inset_0().size_full())
            .when_some(zoom_label, |surface, percent| {
                surface.child(div().absolute().bottom(rems(1.0)).left_0().right_0().flex().justify_center()
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