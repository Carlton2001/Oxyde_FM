//! Hover affordances above an image or video: fullscreen and skipping to the
//! neighboring media of the same kind. Images draw them in the Viewer; video
//! sits under GPUI's composition, so its affordances use owned popups.
use super::Viewer;
use crate::ui::{elements::motion, scale::rem_pixels};
use gpui::{prelude::*, *};
use oxyde_lib::models::{file_entry::NativeFileKind, FileEntry};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use std::time::{Duration, Instant};

/// Forwarded video motion does not report leaving the window; poll the pointer.
const VIDEO_POLL: Duration = Duration::from_millis(100);
/// In fullscreen, the exit button leaves the picture after this much stillness.
const FULLSCREEN_IDLE: Duration = Duration::from_secs(2);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum MediaKind {
    Image,
    Video,
}

/// Mirrors `Viewer::load_current`: audio is never a skipping target.
pub(super) fn media_kind(file: &FileEntry) -> Option<MediaKind> {
    use crate::ui::components::icons::{get_preview_type, PreviewType, AUDIO_EXTENSIONS, VIDEO_EXTENSIONS};
    if file.is_dir || file.archive_path.is_some() {
        return None;
    }
    let extension = file.name.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    if AUDIO_EXTENSIONS.contains(extension.as_str()) || file.native_kind == Some(NativeFileKind::Audio) {
        None
    } else if VIDEO_EXTENSIONS.contains(extension.as_str()) || file.native_kind == Some(NativeFileKind::Video) {
        Some(MediaKind::Video)
    } else if get_preview_type(&file.name) == PreviewType::Image {
        Some(MediaKind::Image)
    } else {
        None
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Part {
    Fullscreen,
    Previous,
    Next,
}

impl Part {
    const ALL: [Part; 3] = [Part::Fullscreen, Part::Previous, Part::Next];

    fn extent(self) -> Rems {
        if self == Part::Fullscreen { rems(2.0) } else { rems(2.5) }
    }
}

#[derive(Default)]
pub(super) struct Overlay {
    pointer: Option<Point<Pixels>>,
    /// Media and content area of the last frame, in window coordinates.
    media: Bounds<Pixels>,
    area: Bounds<Pixels>,
    fullscreen: bool,
    video: bool,
    /// Image or playable video: affordances and edge clicks apply.
    enabled: bool,
    last_motion: Option<Instant>,
    /// The pointer has rested for `FULLSCREEN_IDLE`.
    resting: bool,
    timer: Option<Task<()>>,
    popups: Vec<(Part, WindowHandle<OverlayWindow>)>,
    owner: Option<AnyWindowHandle>,
    owner_raw: Option<RawWindowHandle>,
    scale: f32,
    pending: bool,
    revision: u64,
}

impl Overlay {
    /// The media's visible part, or the whole area before any media is placed.
    fn visible_media(&self) -> Bounds<Pixels> {
        let media = self.media.intersect(&self.area);
        if media.size.width > px(0.0) && media.size.height > px(0.0) { media } else { self.area }
    }

    /// Every part lies in its own zone, so the polled pointer keeps a part
    /// shown over its popup: the popup's own exit event can be missed.
    fn shown(&self, part: Part, neighbors: (bool, bool)) -> bool {
        let Some(pointer) = self.pointer else { return false };
        let media = self.visible_media();
        if !self.area.contains(&pointer) {
            return false;
        }
        // Each arrow answers the pointer near its own edge.
        match part {
            // In fullscreen the exit button follows motion over the whole
            // screen and leaves when the pointer rests, unless it is on it.
            Part::Fullscreen if self.fullscreen => !self.resting || self.bounds(part).contains(&pointer),
            Part::Fullscreen => media.contains(&pointer),
            _ => self.zone(pointer, neighbors) == Some(part),
        }
    }

    /// The edge band whose click skips, as in dedicated image viewers.
    fn zone(&self, pointer: Point<Pixels>, neighbors: (bool, bool)) -> Option<Part> {
        if !self.enabled || !self.area.contains(&pointer) {
            return None;
        }
        let band = (self.area.size.width / 3.0).min(rem_pixels(rems(8.0)));
        if neighbors.0 && pointer.x < self.area.left() + band {
            Some(Part::Previous)
        } else if neighbors.1 && pointer.x > self.area.right() - band {
            Some(Part::Next)
        } else {
            None
        }
    }

    /// Window-space bounds of a part: fullscreen in the media's top-right
    /// corner, arrows on the area's edges, centered on the media.
    fn bounds(&self, part: Part) -> Bounds<Pixels> {
        let media = self.visible_media();
        let extent = rem_pixels(part.extent());
        let margin = rem_pixels(rems(0.75));
        let origin = match part {
            Part::Fullscreen => point(media.right() - margin - extent, media.top() + margin),
            Part::Previous => point(self.area.left() + margin, media.center().y - extent / 2.0),
            Part::Next => point(self.area.right() - margin - extent, media.center().y - extent / 2.0),
        };
        Bounds::new(origin, size(extent, extent))
    }
}

fn chip(part: Part, shown: bool, fullscreen: bool) -> Div {
    let icon = match part {
        Part::Fullscreen if fullscreen => "icons/minimize-2.svg",
        Part::Fullscreen => "icons/maximize-2.svg",
        Part::Previous => "icons/chevron-left.svg",
        Part::Next => "icons/chevron-right.svg",
    };
    div()
        .size(part.extent())
        .flex()
        .items_center()
        .justify_center()
        .rounded(rems(0.5))
        .bg(hsla(0.0, 0.0, 0.0, 0.55))
        .border(rems(0.0625))
        .border_color(hsla(0.0, 0.0, 1.0, 0.14))
        .when(shown, |chip| chip.hover(|style| style.bg(hsla(0.0, 0.0, 0.0, 0.72))))
        .child(
            svg()
                .path(icon)
                .size(if part == Part::Fullscreen { rems(1.0) } else { rems(1.375) })
                .text_color(hsla(0.0, 0.0, 1.0, 0.95)),
        )
}

/// The scroll-to-top presence: fade with a short glide from the media's inside.
fn presence<E: Styled + IntoElement + 'static>(
    element: E,
    id: SharedString,
    part: Part,
    shown: bool,
    left: Pixels,
    top: Pixels,
) -> impl IntoElement {
    let glide = px(4.0);
    element.with_spring(id, motion::presence(shown).from(0.0), move |element, progress| {
        let shift = glide * (1.0 - progress);
        let (left, top) = match part {
            Part::Fullscreen => (left, top + shift),
            Part::Previous => (left + shift, top),
            Part::Next => (left - shift, top),
        };
        element.left(left).top(top).opacity(progress)
    })
}

/// Before a Viewer window closes, take its overlays back from the video
/// surface, which Windows destroys with that window.
pub(super) fn release_surface_overlays(window: &Window) {
    if let Ok(raw) = HasWindowHandle::window_handle(window) {
        oxyde_lib::platform::platform().release_viewer_overlays(raw.as_raw());
    }
}

impl Viewer {
    fn neighbor_flags(&self) -> (bool, bool) {
        (self.neighbors.0.is_some(), self.neighbors.1.is_some())
    }

    fn activate_part(&mut self, part: Part, window: &mut Window, cx: &mut Context<Self>) {
        match part {
            Part::Fullscreen => self.toggle_preview_fullscreen(window, cx),
            Part::Previous => self.step(false, cx),
            Part::Next => self.step(true, cx),
        }
    }

    /// A press in an edge band skips, wherever the arrow itself is drawn.
    pub(super) fn overlay_click(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) -> bool {
        match self.overlay.zone(position, self.neighbor_flags()) {
            Some(part) => {
                self.step(part == Part::Next, cx);
                true
            }
            None => false,
        }
    }

    pub(super) fn overlay_pointer_moved(&mut self, position: Option<Point<Pixels>>, cx: &mut Context<Self>) {
        if self.overlay.pointer == position {
            return;
        }
        let before = Part::ALL.map(|part| self.overlay.shown(part, self.neighbor_flags()));
        self.overlay.pointer = position;
        self.overlay.last_motion = Some(Instant::now());
        self.overlay.resting = false;
        if position.is_some() && (self.overlay.video || self.overlay.fullscreen) {
            self.wake_overlay(cx);
        }
        if before != Part::ALL.map(|part| self.overlay.shown(part, self.neighbor_flags())) {
            cx.notify();
        }
    }

    pub(super) fn set_overlay_media(&mut self, media: Bounds<Pixels>, cx: &mut Context<Self>) {
        if self.overlay.media != media {
            self.overlay.media = media;
            cx.notify();
        }
    }

    /// While video affordances may show, follow the real pointer: motion
    /// forwarded from the surface never reports leaving the window.
    fn wake_overlay(&mut self, cx: &mut Context<Self>) {
        if self.overlay.timer.is_some() {
            return;
        }
        self.overlay.timer = Some(cx.spawn(async move |this, cx| loop {
            cx.background_executor().timer(VIDEO_POLL).await;
            let keep_running = this
                .update(cx, |this, cx| {
                    let overlay = &this.overlay;
                    if !(overlay.video || overlay.fullscreen) {
                        this.overlay.timer = None;
                        return false;
                    }
                    let resting = overlay.last_motion.is_some_and(|at| at.elapsed() >= FULLSCREEN_IDLE);
                    if resting != overlay.resting {
                        this.overlay.resting = resting;
                        cx.notify();
                    }
                    let overlay = &this.overlay;
                    let scale = overlay.scale.max(0.1);
                    let pointer = overlay.owner_raw
                        .and_then(|raw| oxyde_lib::platform::platform().cursor_client_position(raw))
                        .map(|[x, y]| point(px(x as f32 / scale), px(y as f32 / scale)))
                        .filter(|pointer| overlay.area.contains(pointer));
                    this.overlay_pointer_moved(pointer, cx);
                    if pointer.is_none() {
                        this.overlay.timer = None;
                        return false;
                    }
                    // Re-place periodically: any restack of the surface is undone at once.
                    this.restack_overlay(cx);
                    true
                })
                .unwrap_or(false);
            if !keep_running {
                break;
            }
        }));
    }

    /// Each popup re-places itself directly above the video surface when drawn.
    /// A hidden HWND receives no paint: a popup that must show is drawn at once,
    /// or it would stay hidden after its first fade-out or a surface change.
    fn restack_overlay(&self, cx: &mut Context<Self>) {
        let neighbors = self.neighbor_flags();
        let mut handles: Vec<(AnyWindowHandle, bool)> = self.overlay.popups.iter()
            .map(|(part, handle)| ((*handle).into(), self.overlay.video && self.overlay.shown(*part, neighbors)))
            .collect();
        handles.extend(self.fullscreen_controls_window().map(|handle| (handle, self.fullscreen_controls_visible())));
        if handles.is_empty() {
            return;
        }
        cx.defer(move |cx| {
            for (handle, shown) in handles {
                if shown {
                    super::fullscreen_controls::draw_controls(handle, cx);
                } else {
                    let _ = handle.update(cx, |_, window, _| window.refresh());
                }
            }
        });
    }

    pub(super) fn close_overlay(&mut self, cx: &mut Context<Self>) {
        let revision = self.overlay.revision.wrapping_add(1);
        let old = std::mem::take(&mut self.overlay);
        self.overlay.revision = revision;
        // Keep the pointer: the next file shows its affordances without motion.
        self.overlay.pointer = old.pointer;
        self.overlay.area = old.area;
        for (_, handle) in old.popups {
            cx.defer(move |cx| {
                let _ = handle.update(cx, |_, window, _| window.remove_window());
            });
        }
    }

    /// The layer above the content: in-window chips for images, popups for video.
    pub(super) fn render_overlay(&mut self, window: &Window, failed: bool, cx: &mut Context<Self>) -> AnyElement {
        // Chips stay mounted while the next image loads, under a resting pointer.
        let image = self.file.as_ref().and_then(media_kind) == Some(MediaKind::Image)
            && !matches!(self.content, super::content::Content::Unavailable);
        let video = self.native_media && !self.audio && !failed && !self.blocked;
        self.overlay.fullscreen = window.is_fullscreen();
        self.overlay.video = video;
        self.overlay.enabled = image || video;
        // A pointer kept across a skip is still followed.
        if (video || self.overlay.fullscreen) && self.overlay.pointer.is_some() && self.overlay.timer.is_none() {
            self.wake_overlay(cx);
        }
        let handle = cx.entity().downgrade();
        let measure = canvas(|_, _, _| (), move |bounds, _, window, cx| {
            let _ = handle.update(cx, |this, cx| {
                if this.overlay.area != bounds {
                    this.overlay.area = bounds;
                    cx.notify();
                }
                // The fullscreen poll reads the real pointer for images too;
                // without this window it would report none and hide the chips.
                this.overlay.scale = window.scale_factor();
                this.overlay.owner_raw = HasWindowHandle::window_handle(window).ok().map(|raw| raw.as_raw());
                if this.overlay.video {
                    this.place_overlay_popups(window, cx);
                }
            });
        })
        .absolute()
        .size_full();
        let layer = div().absolute().inset_0().child(measure);
        if !image {
            return layer.into_any_element();
        }
        let area = self.overlay.area.origin;
        let neighbors = self.neighbor_flags();
        layer
            .children(Part::ALL.map(|part| {
                let shown = self.overlay.shown(part, neighbors);
                let origin = self.overlay.bounds(part).origin - area;
                let chip = chip(part, shown, self.overlay.fullscreen)
                    .id(SharedString::from(format!("viewer-overlay-{part:?}")))
                    .absolute()
                    .when(shown, |chip| {
                        chip.on_mouse_down(MouseButton::Left, cx.listener(move |this, _, window, cx| {
                            cx.stop_propagation();
                            this.activate_part(part, window, cx);
                        }))
                    });
                presence(chip, format!("viewer-overlay-presence-{part:?}").into(), part, shown, origin.x, origin.y)
            }))
            .into_any_element()
    }

    fn place_overlay_popups(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let owner = Window::window_handle(window);
        if self.overlay.owner.is_some_and(|current| current != owner) {
            // The player moved to another window: its popups follow.
            self.close_overlay(cx);
        }
        self.overlay.owner = Some(owner);
        self.overlay.scale = window.scale_factor();
        self.overlay.owner_raw = HasWindowHandle::window_handle(window).ok().map(|raw| raw.as_raw());
        let neighbors = self.neighbor_flags();
        if !self.overlay.popups.is_empty() {
            self.restack_overlay(cx);
            return;
        }
        // Create the popups on the first hover only.
        if self.overlay.pending || !Part::ALL.iter().any(|part| self.overlay.shown(*part, neighbors)) {
            return;
        }
        let Ok(raw_owner) = HasWindowHandle::window_handle(window).map(|raw| raw.as_raw()) else {
            return;
        };
        self.overlay.pending = true;
        let revision = self.overlay.revision;
        let rem = window.rem_size();
        let origin = window.bounds().origin;
        let viewer = cx.entity().downgrade();
        cx.defer(move |cx| {
            let Some(entity) = viewer.upgrade() else { return };
            if entity.read(cx).overlay.revision != revision {
                return;
            }
            let mut opened = Vec::new();
            for part in Part::ALL {
                let extent = rem_pixels(part.extent());
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
                        window_bounds: Some(WindowBounds::Windowed(Bounds::new(origin, size(extent, extent)))),
                        ..Default::default()
                    },
                    |window, cx| {
                        window.set_rem_size(rem);
                        cx.new(|cx| {
                            let observer = cx.observe(&entity, |_, _, cx| cx.notify());
                            let release = cx.observe_release_in(&entity, window, |_, _, window, _| {
                                window.remove_window()
                            });
                            OverlayWindow {
                                viewer: viewer.clone(),
                                part,
                                owner: raw_owner,
                                _subscriptions: vec![observer, release],
                            }
                        })
                    },
                );
                match result {
                    Ok(handle) => opened.push((part, handle)),
                    Err(error) => log::error!("Cannot open Viewer media controls: {error}"),
                }
            }
            let current = entity.update(cx, |this, cx| {
                this.overlay.pending = false;
                let current = this.overlay.revision == revision;
                if current {
                    this.overlay.popups = opened.clone();
                }
                cx.notify();
                current
            });
            for (_, handle) in opened {
                if current {
                    super::fullscreen_controls::draw_controls(handle.into(), cx);
                } else {
                    let _ = handle.update(cx, |_, window, _| window.remove_window());
                }
            }
        });
    }
}

pub(super) struct OverlayWindow {
    viewer: WeakEntity<Viewer>,
    part: Part,
    owner: RawWindowHandle,
    _subscriptions: Vec<Subscription>,
}

impl Render for OverlayWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let part = self.part;
        let Some(viewer) = self.viewer.upgrade() else {
            return div().into_any_element();
        };
        let (shown, rect, fullscreen) = {
            let viewer = viewer.read(cx);
            let overlay = &viewer.overlay;
            let shown = overlay.video && overlay.shown(part, viewer.neighbor_flags());
            let bounds = overlay.bounds(part);
            (shown, bounds, overlay.fullscreen)
        };
        // Popup rectangles are in the owner's physical client coordinates.
        let scale = viewer.read(cx).overlay.scale;
        let rect = [rect.origin.x, rect.origin.y, rect.size.width, rect.size.height]
            .map(|value| (value.as_f32() * scale).round() as i32);
        let raw = HasWindowHandle::window_handle(window).ok().map(|raw| raw.as_raw());
        let owner = self.owner;
        let hover = self.viewer.clone();
        let press = self.viewer.clone();
        let chip = chip(part, shown, fullscreen)
            .id("viewer-media-control")
            .relative()
            .on_hover(move |hovered, _, cx| {
                if *hovered {
                    let _ = hover.update(cx, |this, cx| {
                        if this.overlay.video {
                            this.wake_overlay(cx);
                        }
                    });
                }
            })
            .when(shown, |chip| {
                chip.on_mouse_down(MouseButton::Left, move |_, _, cx| {
                    let Some(viewer) = press.upgrade() else { return };
                    let Some(window) = viewer.read(cx).overlay.owner else { return };
                    // Act in the Viewer's window, never in this popup.
                    cx.defer(move |cx| {
                        let _ = window.update(cx, |_, window, cx| {
                            viewer.update(cx, |this, cx| this.activate_part(part, window, cx));
                        });
                    });
                })
            });
        // The popup stays placed until its fade-out completes.
        chip.with_spring("viewer-media-control-presence", motion::presence(shown).from(0.0), move |chip, progress| {
            chip.opacity(progress).child(canvas(|_, _, _| (), move |_, _, _, cx| {
                if let Some(raw) = raw {
                    cx.defer(move |_| {
                        let visible = shown || progress > 0.01;
                        if let Err(error) = oxyde_lib::platform::platform().place_viewer_overlay(raw, owner, rect, visible) {
                            log::warn!("Cannot place Viewer media controls: {error}");
                        }
                    });
                }
            }).absolute().inset_0().size_full())
        })
        .into_any_element()
    }
}
