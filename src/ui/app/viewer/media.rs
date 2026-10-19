use super::Viewer;
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
impl Drop for MediaSession {
    fn drop(&mut self) {
        self.control.cancel();
        self.cancellation.cancel();
    }
}

impl Viewer {
    pub(super) fn update_visibility(&self) {
        if let Some(media) = &self.media {
            if let Ok(mut command) = media.control.command.lock() {
                command.visible = !self.blocked;
                command.paused = media.paused || self.blocked;
            }
        }
    }

    pub(super) fn place_media(
        &mut self,
        bounds: Bounds<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let bounds = bounds.intersect(&window.content_mask().bounds);
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
            self.media = None;
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
            let Some(path) = self.files.get(self.index).map(|file| file.path.clone()) else {
                return;
            };
            let control = Arc::new(ViewerMediaControl::default());
            *control.command.lock().expect("new media controls") =
                oxyde_lib::platform::ViewerMediaCommand {
                    parent,
                    rect,
                    visible: !self.blocked,
                    paused: self.blocked,
                };
            let cancellation = crate::ui::preview_jobs::Cancellation::default();
            self.media = Some(MediaSession {
                control: control.clone(),
                paused: false,
                cancellation: cancellation.clone(),
            });
            let work_control = control.clone();
            // Admission and parsing run on a worker, never during render/paint.
            let work = crate::ui::preview_jobs::load_media(cancellation, move || {
                let result = oxyde_lib::platform::platform()
                    .play_viewer_media(std::path::Path::new(&path), &work_control);
                if result.is_err() && !work_control.is_cancelled() {
                    work_control.status.store(MEDIA_FAILED, Ordering::Release);
                }
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
                let mut previous = MEDIA_LOADING;
                while !control.is_cancelled() {
                    cx.background_executor()
                        .timer(std::time::Duration::from_millis(80))
                        .await;
                    let status = control.status();
                    if status != previous {
                        previous = status;
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
                command.rect = rect;
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
