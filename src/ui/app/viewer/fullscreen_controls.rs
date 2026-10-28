//! Fullscreen playback controls, above the worker-owned video surface.
use super::Viewer;
use crate::ui::elements::motion;
use gpui::{prelude::*, *};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use std::time::{Duration, Instant};

const IDLE: Duration = Duration::from_secs(2);

#[derive(Default)]
pub(super) struct Controls {
    enabled: bool,
    visible: bool,
    hovered: bool,
    last_motion: Option<Instant>,
    last_pointer: Option<Point<Pixels>>,
    timer: Option<Task<()>>,
    window: Option<WindowHandle<ControlsWindow>>,
    pending: bool,
    revision: u64,
    rect: [i32; 4],
}

impl Viewer {
    pub(super) fn enable_fullscreen_controls(&mut self, enabled: bool, cx: &mut Context<Self>) {
        if !enabled {
            if self.fullscreen_controls.enabled {
                self.close_fullscreen_controls(cx);
            }
        } else if !self.fullscreen_controls.enabled {
            self.fullscreen_controls.enabled = true;
            self.wake_controls(cx);
        }
    }

    pub(super) fn controls_pointer_moved(
        &mut self,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        if self.fullscreen_controls.last_pointer == Some(position) {
            return;
        }
        self.fullscreen_controls.last_pointer = Some(position);
        // The bar is a separate popup: motion reaching the Viewer, including
        // input forwarded from the video surface, is outside it. Its own exit
        // event is unreliable across these nonactivating windows.
        self.fullscreen_controls.hovered = false;
        self.wake_controls(cx);
    }

    fn wake_controls(&mut self, cx: &mut Context<Self>) {
        let controls = &mut self.fullscreen_controls;
        if !controls.enabled {
            return;
        }
        controls.last_motion = Some(Instant::now());
        if !controls.visible {
            controls.visible = true;
            cx.notify();
        }
        if controls.timer.is_some() {
            return;
        }
        controls.timer = Some(cx.spawn(async move |this, cx| loop {
            cx.background_executor()
                .timer(Duration::from_millis(100))
                .await;
            let keep_running = this
                .update(cx, |this, cx| {
                    let controls = &mut this.fullscreen_controls;
                    if !controls.enabled {
                        return false;
                    }
                    if should_hide(
                        controls
                            .last_motion
                            .map(|at| at.elapsed())
                            .unwrap_or_default(),
                        controls.hovered,
                        this.seeking,
                    ) {
                        controls.visible = false;
                        controls.timer = None;
                        cx.notify();
                        return false;
                    }
                    true
                })
                .unwrap_or(false);
            if !keep_running {
                break;
            }
        }));
    }

    pub(super) fn fullscreen_controls_window(&self) -> Option<AnyWindowHandle> {
        self.fullscreen_controls.window.map(Into::into)
    }

    pub(super) fn fullscreen_controls_visible(&self) -> bool {
        self.fullscreen_controls.visible
    }

    pub(super) fn close_fullscreen_controls(&mut self, cx: &mut Context<Self>) {
        let revision = self.fullscreen_controls.revision.wrapping_add(1);
        let old = std::mem::take(&mut self.fullscreen_controls);
        self.fullscreen_controls.revision = revision;
        self.seeking = false;
        if let Some(media) = &self.media {
            if let Ok(mut command) = media.control.command.lock() {
                command.controls = 0;
                media.control.wake();
            }
        }
        if let Some(handle) = old.window {
            cx.defer(move |cx| {
                let _ = handle.update(cx, |_, window, _| window.remove_window());
            });
        }
    }

    pub(super) fn place_fullscreen_controls(
        &mut self,
        bounds: Bounds<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Without a native surface (Linux), the Viewer paints the bar itself.
        if !self.fullscreen_controls.enabled || !cfg!(target_os = "windows") {
            return;
        }
        let Bounds { origin, size: Size { width, height } } = fullscreen_bar_bounds(bounds);
        let scale = window.scale_factor();
        self.fullscreen_controls.rect = [
            origin.x.as_f32(),
            origin.y.as_f32(),
            width.as_f32(),
            height.as_f32(),
        ]
        .map(|value| (value * scale).round() as i32);
        if let Some(handle) = self.fullscreen_controls.window {
            if self.fullscreen_controls.visible {
                // A hidden HWND receives no paint until shown. Draw after the
                // owning Viewer borrow ends, so the transparent first frame can
                // show the popup and let its content transition run normally.
                cx.defer(move |cx| { draw_controls(handle.into(), cx); });
            }
            return;
        }
        if self.fullscreen_controls.pending {
            return;
        }
        self.fullscreen_controls.pending = true;
        let revision = self.fullscreen_controls.revision;
        let Ok(raw_owner) = HasWindowHandle::window_handle(window).map(|raw| raw.as_raw()) else {
            return;
        };

        let initial = Bounds::new(window.bounds().origin, size(width, height));
        let rem = window.rem_size();
        let viewer = cx.entity().downgrade();
        cx.defer(move |cx| {
            let Some(entity) = viewer.upgrade() else {
                return;
            };
            if !entity.read(cx).fullscreen_controls.enabled
                || entity.read(cx).fullscreen_controls.revision != revision
            {
                return;
            }
            let result = cx.open_window(
                WindowOptions {
                    app_id: Some(crate::ui::APP_ID.into()),
                    kind: WindowKind::PopUp,
                    titlebar: None,
                    show: false,
                    focus: false,
                    window_background: WindowBackgroundAppearance::Transparent,
                    is_resizable: false,
                    is_minimizable: false,
                    is_movable: false,
                    window_bounds: Some(WindowBounds::Windowed(initial)),
                    ..Default::default()
                },
                |window, cx| {
                    window.set_rem_size(rem);
                    cx.new(|cx| {
                        let observer = cx.observe(&entity, |_, _, cx| cx.notify());
                        let release = cx.observe_release_in(&entity, window, |_, _, window, _| {
                            window.remove_window()
                        });
                        ControlsWindow {
                            viewer: viewer.clone(),
                            owner: raw_owner,
                            _subscriptions: vec![observer, release],
                        }
                    })
                },
            );
            let ready = result.as_ref().ok().copied();
            entity.update(cx, |this, cx| {
                this.fullscreen_controls.pending = false;
                match result {
                    // The bar stacks itself above the surface (`place_viewer_overlay`);
                    // the worker never restacks its surface over the overlays.
                    Ok(handle) => this.fullscreen_controls.window = Some(handle),
                    Err(error) => log::error!("Cannot open fullscreen playback controls: {error}"),
                }
                cx.notify();
            });
            if let Some(handle) = ready { draw_controls(handle.into(), cx); }
        });
    }
}

impl Viewer {
    /// The floating bar painted over GPUI's own video frames, as the popup shows it.
    pub(super) fn render_fullscreen_bar(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        let hover = cx.entity().downgrade();
        let moving = hover.clone();
        div().id("fullscreen-playback-controls")
            .size_full().p(rems(0.5))
            .text_size(rems(0.8125)).font_weight(FontWeight::NORMAL)
            .on_hover(move |hovered, _, cx| { let _ = hover.update(cx, |this, cx| {
                this.fullscreen_controls.hovered = *hovered;
                this.wake_controls(cx);
            }); })
            .on_mouse_move(move |_, _, cx| { let _ = moving.update(cx, |this, cx| this.wake_controls(cx)); })
            .child(div().size_full().rounded_lg().overflow_hidden().child(self.render_media_bar(cx)))
    }
}

/// Where the floating bar sits in fullscreen bounds.
pub(super) fn fullscreen_bar_bounds(bounds: Bounds<Pixels>) -> Bounds<Pixels> {
    let margin = crate::ui::scale::rem_pixels(rems(1.0));
    let width = (bounds.size.width - margin * 2.0)
        .max(px(1.0))
        .min(crate::ui::scale::rem_pixels(rems(64.0)));
    let height = crate::ui::scale::rem_pixels(rems(3.5625));
    Bounds::new(bounds.origin + point(
        (bounds.size.width - width) / 2.0,
        (bounds.size.height - height - margin).max(px(0.0)),
    ), size(width, height))
}

pub(super) struct ControlsWindow {
    viewer: WeakEntity<Viewer>,
    owner: RawWindowHandle,
    _subscriptions: Vec<Subscription>,
}

impl Render for ControlsWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(viewer) = self.viewer.upgrade() else {
            return div().into_any_element();
        };
        let (visible, rect, bar) = viewer.update(cx, |this, cx| {
            (
                this.fullscreen_controls.visible,
                this.fullscreen_controls.rect,
                this.render_media_bar(cx),
            )
        });
        let raw = HasWindowHandle::window_handle(window)
            .ok()
            .map(|raw| raw.as_raw());
        let owner = self.owner;
        let hover = self.viewer.clone();
        let moving = self.viewer.clone();
        div().id("fullscreen-playback-controls")
            // GPUI does not adopt this popup's native resize, so its viewport
            // keeps the creation size. Size the bar from the placed rectangle.
            .w(px(rect[2] as f32 / window.scale_factor())).h(px(rect[3] as f32 / window.scale_factor())).p(rems(0.5))
            .text_size(rems(0.8125)).font_weight(FontWeight::NORMAL)
            .on_hover(move |hovered, _, cx| { let _ = hover.update(cx, |this, cx| {
                this.fullscreen_controls.hovered = *hovered;
                this.wake_controls(cx);
            }); })
            .on_mouse_move(move |_, _, cx| { let _ = moving.update(cx, |this, cx| this.wake_controls(cx)); })
            .child(div().size_full().rounded_lg().overflow_hidden().child(bar))
            .with_spring("fullscreen-controls-presence", motion::presence(visible).from(0.0), move |root, progress| {
                root.opacity(progress).relative().top(rems((1.0 - progress) * 0.375))
                    .child(canvas(|_, _, _| (), move |_, _, _, cx| {
                        if let Some(raw) = raw { cx.defer(move |_| {
                            if let Err(error) = oxyde_lib::platform::platform().place_viewer_overlay(raw, owner, rect, visible || progress > 0.01) {
                                log::warn!("Cannot place fullscreen playback controls: {error}");
                            }
                        }); }
                    }).absolute().inset_0().size_full())
            }).into_any_element()
    }
}

pub(super) fn draw_controls(handle: AnyWindowHandle, cx: &mut App) {
    let _ = handle.update(cx, |_, window, cx| {
        window.refresh();
        // As with native menus, initialize input only after open_window returns.
        // The untyped handle is essential: a typed update borrows ControlsWindow
        // while dispatch_event may draw and update that same root recursively.
        window.dispatch_event(PlatformInput::ModifiersChanged(ModifiersChangedEvent::default()), cx);
    });
}

fn should_hide(idle: Duration, hovered: bool, seeking: bool) -> bool {
    idle >= IDLE && !hovered && !seeking
}

#[cfg(test)]
#[path = "tests/fullscreen_controls.rs"]
mod tests;

#[cfg(all(test, target_os = "windows"))]
#[path = "tests/fullscreen_controls_native.rs"]
mod native_tests;
