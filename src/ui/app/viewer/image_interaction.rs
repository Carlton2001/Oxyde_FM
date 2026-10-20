use super::Viewer;
use gpui::{prelude::*, *};
use std::sync::Arc;

pub(super) struct ImageView {
    scale: f32,
    offset: Point<Pixels>,
    target: Bounds<Pixels>,
    drag: Option<Point<Pixels>>,
    zooming_until: Option<std::time::Instant>,
}
impl Default for ImageView {
    fn default() -> Self {
        Self { scale: 1.0, offset: Point::default(), target: Bounds::default(), drag: None, zooming_until: None }
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
            crate::ui::components::tool_window::close_transients(window, cx);
            window.remove_window();
        } else {
            window.toggle_fullscreen();
        }
        cx.notify();
    }

    pub(super) fn render_interactive_image(&self, image: Arc<RenderImage>, cx: &mut Context<Self>) -> AnyElement {
        let handle = cx.entity().downgrade();
        let fit = if self.detached { ObjectFit::Contain } else { ObjectFit::ScaleDown };
        let scale = self.image_view.scale;
        let offset = self.image_view.offset;
        let dragging = self.image_view.drag.is_some();
        let zooming = self.image_view.drag.is_none() && self.image_view.zooming_until.is_some_and(|until| until > std::time::Instant::now());
        div().id("viewer-image").size_full().relative().overflow_hidden()
            .when(self.image_view.drag.is_some(), |surface| surface.cursor(CursorStyle::ClosedHand))
            .on_scroll_wheel(cx.listener(|this, event: &ScrollWheelEvent, window, cx| {
                let view = &mut this.image_view;
                if !view.target.contains(&event.position) || event.modifiers.control { return; }
                let delta = event.delta.pixel_delta(window.line_height()).y.as_f32();
                if !delta.is_finite() || delta == 0.0 { return; }
                let next = (view.scale * (delta * 0.002).exp()).clamp(1.0, 32.0);
                let ratio = next / view.scale;
                let center = view.target.center();
                view.offset = anchored_offset(view.offset, event.position, center, ratio);
                view.scale = next;
                view.zooming_until = Some(std::time::Instant::now() + std::time::Duration::from_millis(180));
                if next == 1.0 { view.offset = Point::default(); }
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
                let _ = handle.update(cx, |this, _| this.image_view.target = target);
                let _ = window.paint_image(bounds, target, Corners::default(), image.clone(), 0, false);
            }).absolute().inset_0().size_full())
            .into_any_element()
    }
}

fn anchored_offset(offset: Point<Pixels>, cursor: Point<Pixels>, center: Point<Pixels>, ratio: f32) -> Point<Pixels> {
    offset + (cursor - center) * (1.0 - ratio)
}

#[cfg(test)]
#[path = "tests/image_interaction.rs"]
mod tests;