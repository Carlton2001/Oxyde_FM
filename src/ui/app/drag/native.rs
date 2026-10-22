//! Bridge between OLE and GPUI. Native callbacks enter GPUI only from the message loop.
mod ghost;
use super::super::OxydeApp;
use gpui::*;
use oxyde_lib::platform::{platform, FileDropEffect, FileDropPhase, NativeFileDrop};
use raw_window_handle::HasWindowHandle;

pub(crate) struct Outgoing {
    token: EntityId,
    paths: Vec<String>,
    pub(super) native: bool,
    ready: bool,
    outside: bool,
    effect: FileDropEffect,
}

pub(super) fn has_ghost(cx: &App) -> bool {
    ghost::is_open(cx)
}

pub(super) fn begin(
    paths: Vec<String>,
    is_dir: bool,
    app: Entity<OxydeApp>,
    window: &mut Window,
    cx: &mut Context<super::FileDragPreview>,
) {
    let token = cx.entity().entity_id();
    app.update(cx, |app, _| {
        app.file_drag_session = Some(Outgoing {
            token,
            paths: paths.clone(),
            native: false,
            ready: false,
            outside: false,
            effect: FileDropEffect::None,
        })
    });
    let weak = app.downgrade();
    cx.on_release(move |_, cx| {
        let _ = weak.update(cx, |app, cx| {
            if app
                .file_drag_session
                .as_ref()
                .is_some_and(|session| session.token == token && !session.native)
            {
                app.file_drag_session = None;
                ghost::close(cx);
            }
        });
    })
    .detach();
    ghost::open(token, paths.clone(), is_dir, app.downgrade(), window, cx);
    let owner = Window::window_handle(window);
    cx.spawn(async move |_, cx| {
        // Real files only. Archive members need materialization before they can
        // be offered to a foreign process; never export their virtual paths.
        let real = tokio::task::spawn_blocking(move || {
            paths.iter().all(|path| {
                std::path::Path::new(path).is_absolute() && std::fs::symlink_metadata(path).is_ok()
            })
        })
        .await
        .unwrap_or(false);
        let _ = owner.update(cx, |_, window, cx| {
            app.update(cx, |app, cx| {
                if let Some(session) = app
                    .file_drag_session
                    .as_mut()
                    .filter(|session| session.token == token)
                {
                    session.ready = real;
                    app.promote_file_drag(window, cx);
                }
            })
        });
    })
    .detach();
}

impl OxydeApp {
    pub(crate) fn install_file_drag(app: &Entity<Self>, window: &mut Window, cx: &mut App) {
        let Ok(raw) = HasWindowHandle::window_handle(window) else {
            return;
        };
        let owner = Window::window_handle(window);
        let weak = app.downgrade();
        let asynchronous = cx.to_async();
        let callback = Box::new(move |event: NativeFileDrop| {
            let mut asynchronous = asynchronous.clone();
            owner
                .update(&mut asynchronous, |_, window, cx| {
                    let Some(app) = weak.upgrade() else {
                        return FileDropEffect::None;
                    };
                    receive(&app, event, window, cx)
                })
                .unwrap_or(FileDropEffect::None)
        });
        if let Err(error) = platform().install_file_drop_target(raw.as_raw(), callback) {
            log::warn!("Cannot install native file drop target: {error}");
        }
    }

    pub(in crate::ui::app) fn promote_file_drag(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.file_drag_session.is_none() {
            return;
        }
        let raw = HasWindowHandle::window_handle(window)
            .ok()
            .map(|raw| raw.as_raw());
        let outside = !Bounds::new(Point::default(), window.viewport_size())
            .contains(&window.mouse_position())
            || !window.is_window_active()
            || raw.is_some_and(|raw| platform().file_drag_pointer_outside(raw));
        if let Some(session) = &mut self.file_drag_session {
            session.outside = outside;
        }
        ghost::sync(
            self.file_drag_session
                .as_ref()
                .is_some_and(|session| session.native),
            window,
            cx,
        );
        let Some(session) = self.file_drag_session.as_ref() else {
            return;
        };
        if session.native || !session.ready || !cx.has_active_drag() {
            return;
        }
        if !outside {
            return;
        }
        let Ok(raw) = HasWindowHandle::window_handle(window) else {
            return;
        };
        let token = session.token;
        let paths = session.paths.clone();
        let weak = cx.entity().downgrade();
        let owner = Window::window_handle(window);
        let asynchronous = cx.to_async();
        let callback = Box::new(move |feedback: oxyde_lib::platform::FileDragFeedback| {
            let mut asynchronous = asynchronous.clone();
            let _ = owner.update(&mut asynchronous, |_, window, cx| {
                let _ = weak.update(cx, |app, cx| {
                    let Some(session) = app
                        .file_drag_session
                        .as_mut()
                        .filter(|session| session.token == token)
                    else {
                        return;
                    };
                    session.effect = feedback.effect;
                    if feedback.finished {
                        let paths = session.paths.clone();
                        let origin = app.file_drag_origin.take();
                        cx.stop_active_drag(window);
                        app.file_drag_session = None;
                        app.finish_pointer_drag(cx);
                        ghost::close(cx);
                        if feedback.delete_sources {
                            app.queue_trash_delete(paths, false, cx);
                        }
                        if feedback.cancelled {
                            if let Some((pane, tab)) = origin {
                                if let Some(index) = app.state.find_pane(&pane).and_then(|pane| {
                                    pane.tabs.iter().position(|item| item.id == tab)
                                }) {
                                    app.switch_tab(&pane, index, cx);
                                }
                            }
                        }
                        app.refresh_all_tabs(cx);
                    } else {
                        ghost::sync(true, window, cx);
                    }
                    cx.notify();
                });
            });
        });
        match platform().queue_file_drag(raw.as_raw(), paths, callback) {
            Ok(()) => {
                self.file_drag_session.as_mut().unwrap().native = true;
                // The native target now owns all releases, including re-entry.
                // Keeping the internal drag would permit a second transfer.
                cx.stop_active_drag(window);
                ghost::sync(true, window, cx);
            }
            Err(error) => log::warn!("Cannot start native file drag: {error}"),
        }
    }
}

fn receive(
    app: &Entity<OxydeApp>,
    event: NativeFileDrop,
    window: &mut Window,
    cx: &mut App,
) -> FileDropEffect {
    if event.phase == FileDropPhase::Leave {
        window.dispatch_event(FileDropEvent::Exited.to_platform_input(), cx);
        app.update(cx, |app, _| app.external_drop = Default::default());
        return FileDropEffect::None;
    }
    let scale = window.scale_factor().max(0.1);
    let position = point(
        px(event.position[0] as f32 / scale),
        px(event.position[1] as f32 / scale),
    );
    let modifiers = Modifiers {
        control: event.control,
        shift: event.shift,
        ..Default::default()
    };
    app.update(cx, |app, _| {
        if event.phase == FileDropPhase::Enter {
            app.external_drop = Default::default();
        }
        app.external_drop.effect = FileDropEffect::None;
        app.external_drop.can_copy = event.can_copy;
        app.external_drop.can_move = event.can_move;
        if event.phase == FileDropPhase::Enter {
            app.external_drop.submitted = false;
        }
    });
    if event.phase == FileDropPhase::Enter {
        let paths = ExternalPaths(
            event
                .paths
                .into_iter()
                .map(std::path::PathBuf::from)
                .collect(),
        );
        window.dispatch_event(
            FileDropEvent::Entered { position, paths }.to_platform_input(),
            cx,
        );
    }
    // FileDropEvent's synthesized mouse events carry default modifiers in GPUI.
    // Dispatch actual OLE modifiers, including at the final release.
    window.dispatch_event(
        MouseMoveEvent {
            position,
            pressed_button: Some(MouseButton::Left),
            modifiers,
        }
        .to_platform_input(),
        cx,
    );
    if event.phase == FileDropPhase::Drop {
        window.dispatch_event(
            MouseUpEvent {
                position,
                button: MouseButton::Left,
                modifiers,
                click_count: 1,
            }
            .to_platform_input(),
            cx,
        );
        let effect = app.read(cx).external_drop.effect;
        let submitted = app.read(cx).external_drop.submitted;
        cx.stop_active_drag(window);
        app.update(cx, |app, cx| app.finish_pointer_drag(cx));
        if submitted {
            effect
        } else {
            FileDropEffect::None
        }
    } else {
        app.read(cx).external_drop.effect
    }
}

pub(crate) fn close(cx: &mut App) {
    ghost::close(cx);
}

#[cfg(test)]
#[path = "tests/native_transfer.rs"]
mod tests;
