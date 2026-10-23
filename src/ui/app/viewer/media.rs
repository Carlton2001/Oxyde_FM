use super::{OxydeApp, Viewer};
use gpui::*;
use oxyde_lib::platform::{
    ViewerMediaControl, MEDIA_ENDED, MEDIA_FAILED, MEDIA_LOADING, MEDIA_PAUSED,
};
use std::sync::{atomic::Ordering, Arc};

pub(super) struct MediaSession {
    pub(super) control: Arc<ViewerMediaControl>,
    pub(super) paused: bool,
    cancellation: crate::ui::preview_jobs::Cancellation,
}

#[cfg(test)]
#[path = "tests/media_visibility.rs"]
mod visibility_tests;
impl Drop for MediaSession {
    fn drop(&mut self) {
        // Overlays owned by the surface must not be destroyed with it.
        let parent = self.control.command.lock().map(|command| command.parent).unwrap_or(0);
        if let Some(parent) = std::num::NonZeroIsize::new(parent as isize) {
            oxyde_lib::platform::platform().release_viewer_overlays(
                raw_window_handle::RawWindowHandle::Win32(raw_window_handle::Win32WindowHandle::new(parent)),
            );
        }
        self.control.cancel();
        self.cancellation.cancel();
    }
}

impl MediaSession {
    fn pause(&mut self, blocked: bool) {
        self.paused = true;
        self.update_visibility(blocked);
    }

    fn update_visibility(&self, blocked: bool) {
        if let Ok(mut command) = self.control.command.lock() {
            let visible = !blocked;
            if command.visible != visible || command.paused != self.paused {
                command.visible = visible;
                command.paused = self.paused;
                self.control.wake();
            }
        }
    }
}

impl OxydeApp {
    pub(crate) fn pause_viewer_media_for_external_open(
        &mut self,
        path: &str,
        cx: &mut Context<Self>,
    ) {
        for viewer in self.viewers.values() {
            viewer.update(cx, |viewer, cx| {
                // Selection notifications may not have reached the Viewer yet
                // when a double click opens the newly selected file.
                let source = super::Source::capture(self, &viewer.source);
                viewer.set_source(source, cx);
                if viewer.source.selected.contains(path) {
                    viewer.external_open_pauses.insert(path.to_owned());
                }
                if !viewer.native_media
                    || !viewer.file.as_ref().is_some_and(|file| file.path == path)
                {
                    return;
                }
                if let Some(media) = &mut viewer.media {
                    media.pause(viewer.blocked);
                } else {
                    // Also cover a pending first paint or a surface transfer.
                    viewer.resume.get_or_insert((0, true)).1 = true;
                }
                cx.notify();
            });
        }
    }
}

impl Viewer {
    /// Stops the current player and remembers where it was, so the next
    /// surface (docked or detached) resumes at the same point.
    pub(super) fn suspend_media(&mut self) {
        if let Some(media) = self.media.take() {
            let ended = media.control.status() == MEDIA_ENDED;
            if !ended && media.control.status() != MEDIA_FAILED {
                self.resume = Some((media.control.position_ms(), media.paused));
            }
        }
    }

    pub(super) fn update_visibility(&self) {
        if let Some(media) = &self.media {
            media.update_visibility(self.blocked);
        }
    }

    pub(super) fn place_media(
        &mut self,
        bounds: Bounds<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let bounds = bounds.intersect(&window.content_mask().bounds);
        if !self.audio {
            self.set_overlay_media(bounds, cx);
        }
        use raw_window_handle::HasWindowHandle;
        let Some(parent) = HasWindowHandle::window_handle(window)
            .ok()
            .and_then(|handle| {
                oxyde_lib::platform::platform().viewer_media_parent(handle.as_raw())
            })
        else {
            return;
        };
        if self.media.as_ref().is_some_and(|media| {
            media
                .control
                .command
                .lock()
                .is_ok_and(|command| command.parent != parent)
        }) {
            // The player moves to another window: continue where it was.
            self.suspend_media();
        }
        let scale = window.scale_factor();
        let rect = [
            bounds.origin.x.as_f32() * scale,
            bounds.origin.y.as_f32() * scale,
            bounds.size.width.as_f32() * scale,
            bounds.size.height.as_f32() * scale,
        ]
        .map(|value| value.round() as i32);
        if self.media.is_none() {
            let Some(path) = self.file.as_ref().map(|file| file.path.clone()) else {
                return;
            };
            let control = Arc::new(ViewerMediaControl::default());
            let (resume_ms, resume_paused) = self.resume.take().unwrap_or((0, !self.media_autoplay));
            *control.command.lock().expect("new media controls") =
                oxyde_lib::platform::ViewerMediaCommand {
                    parent,
                    controls: 0,
                    rect,
                    visible: !self.blocked,
                    paused: resume_paused,
                    seek_id: u32::from(resume_ms > 0),
                    seek_ms: resume_ms,
                };
            control.position_ms.store(resume_ms, Ordering::Release);
            let cancellation = crate::ui::preview_jobs::Cancellation::default();
            self.media = Some(MediaSession {
                control: control.clone(),
                paused: resume_paused,
                cancellation: cancellation.clone(),
            });
            let work_control = control.clone();
            let completion = self.jobs.register(&path);
            // Admission and parsing run on a worker, never during render/paint.
            let work = crate::ui::preview_jobs::load_media(cancellation, move || {
                let result = oxyde_lib::platform::platform()
                    .play_viewer_media(std::path::Path::new(&path), &work_control);
                if result.is_err() && !work_control.is_cancelled() {
                    work_control.status.store(MEDIA_FAILED, Ordering::Release);
                }
                drop(completion);
                result
            });
            let failure_control = control.clone();
            cx.background_executor()
                .spawn(async move {
                    if let Err(error) = work.await {
                        if !failure_control.is_cancelled() {
                            failure_control
                                .status
                                .store(MEDIA_FAILED, Ordering::Release);
                        }
                        log::warn!("Viewer media playback: {error}");
                    }
                })
                .detach();
            cx.spawn(async move |this, cx| {
                let mut previous = (MEDIA_LOADING, u64::MAX, u64::MAX, 0);
                while !control.is_cancelled() {
                    cx.background_executor()
                        .timer(std::time::Duration::from_millis(80))
                        .await;
                    let status = control.status();
                    // The progress bar moves in quarter seconds.
                    let current = (status, control.position_ms() / 250, control.duration_ms(), control.video_dimensions.load(Ordering::Acquire));
                    if current != previous {
                        previous = current;
                        if this.update(cx, |_, cx| cx.notify()).is_err() {
                            control.cancel();
                            break;
                        }
                    }
                    if status == MEDIA_FAILED {
                        break;
                    }
                }
            })
            .detach();
        } else if let Some(media) = &self.media {
            if let Ok(mut command) = media.control.command.lock() {
                if command.rect != rect {
                    command.rect = rect;
                    media.control.wake();
                }
            }
            self.update_visibility();
        }
    }

    pub(super) fn toggle_media(&mut self, cx: &mut Context<Self>) {
        if let Some(media) = &mut self.media {
            if matches!(media.control.status(), MEDIA_ENDED | MEDIA_FAILED) {
                self.media = None;
            } else {
                media.paused = !media.paused;
                self.update_visibility();
            }
            cx.notify();
        }
    }

    pub(super) fn seek_media(&mut self, fraction: f32, cx: &mut Context<Self>) {
        if let Some(media) = &self.media {
            let duration = media.control.duration_ms();
            if duration > 0 {
                media.control.seek((duration as f64 * fraction.clamp(0.0, 1.0) as f64) as u64);
                cx.notify();
            }
        }
    }

    pub(super) fn media_paused(&self) -> bool {
        self.media.as_ref().is_some_and(|media| {
            media.paused
                || matches!(
                    media.control.status(),
                    MEDIA_PAUSED | MEDIA_ENDED | MEDIA_FAILED
                )
        })
    }
}
