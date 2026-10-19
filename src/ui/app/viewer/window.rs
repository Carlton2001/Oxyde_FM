use super::Viewer;
use crate::ui::{
    components::tool_window,
    i18n::{I18nContext, Message},
    theme::ThemeContext,
};
use gpui::{prelude::*, *};

pub(super) struct ViewerWindow {
    viewer: Entity<Viewer>,
    _subscriptions: Vec<Subscription>,
}

pub(crate) fn close_viewer_windows(cx: &mut App) {
    for handle in cx.windows() {
        if let Some(handle) = handle.downcast::<ViewerWindow>() {
            let _ = handle.update(cx, |view, window, cx| {
                view.viewer.update(cx, |viewer, _| viewer.media = None);
                tool_window::close_transients(window, cx);
                window.remove_window();
            });
        }
    }
}

impl Viewer {
    pub(super) fn detach(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.detached {
            if let Some(handle) = self.detached_window {
                cx.defer(move |cx| {
                    let _ = handle.update(cx, |_, window, _| window.activate_window());
                });
            }
            return;
        }
        let viewer = cx.entity();
        let owner = self.owner.clone();
        let parent = raw_window_handle::HasWindowHandle::window_handle(window)
            .ok()
            .map(|handle| handle.as_raw());
        let title = cx.i18n().text(Message::ViewerTitle);
        let bounds = Bounds::new(
            window.bounds().origin + point(px(40.0), px(40.0)),
            size(px(900.0), px(650.0)),
        );
        cx.defer(move |cx| {
            if owner.upgrade().is_none() {
                return;
            }
            let result = cx.open_window(
                WindowOptions {
                    window_bounds: Some(super::super::tool_geometry::restore_sized(
                        "icons/eye.svg",
                        bounds,
                        cx,
                    )),
                    is_resizable: true,
                    is_minimizable: true,
                    titlebar: Some(TitlebarOptions {
                        appears_transparent: true,
                        title: Some(title),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                |window, cx| {
                    if let (Some(parent), Ok(handle)) = (
                        parent,
                        raw_window_handle::HasWindowHandle::window_handle(window),
                    ) {
                        if let Err(error) = oxyde_lib::platform::platform()
                            .set_window_owner(handle.as_raw(), parent)
                        {
                            log::warn!("Cannot set Viewer owner: {error}");
                        }
                    }
                    // Only this window paints the player from its first frame;
                    // otherwise both windows would claim it in turn.
                    viewer.update(cx, |viewer, _| viewer.detached = true);
                    cx.new(|cx| {
                        let mut subscriptions = Vec::new();
                        let weak_viewer = viewer.downgrade();
                        subscriptions.push(cx.on_release(move |_, cx| {
                            // The tab and its pane remain the owner of this Viewer.
                            // Stop the secondary surface before restoring inline rendering.
                            let _ = weak_viewer.update(cx, |viewer, cx| {
                                viewer.detached = false;
                                viewer.detached_window = None;
                                viewer.suspend_media();
                                viewer.focus_pending = true;
                                viewer.blocked = viewer.owner.upgrade().is_some_and(|owner| {
                                    let app = owner.read(cx);
                                    app.settings_open || app.dragging_tab.is_some()
                                });
                                cx.notify();
                            });
                        }));
                        if let Some(owner) = owner.upgrade() {
                            subscriptions.push(cx.observe_release_in(
                                &owner,
                                window,
                                |_, _, window, cx| {
                                    tool_window::close_transients(window, cx);
                                    window.remove_window();
                                },
                            ));
                        }
                        tool_window::install_icon("icons/eye.svg", window);
                        window.on_window_should_close(cx, |window, cx| {
                            tool_window::close_transients(window, cx);
                            true
                        });
                        ViewerWindow {
                            viewer: viewer.clone(),
                            _subscriptions: subscriptions,
                        }
                    })
                },
            );
            match result {
                Ok(handle) => {
                    viewer.update(cx, |viewer, cx| {
                        viewer.detached = true;
                        viewer.detached_window = Some(handle);
                        viewer.focus_pending = true;
                        // The secondary window's first paint moves the player
                        // there (see `place_media`), keeping its position.
                        viewer.blocked = false;
                        cx.notify();
                    });
                }
                Err(error) => {
                    viewer.update(cx, |viewer, cx| {
                        viewer.detached = false;
                        cx.notify();
                    });
                    log::error!("Cannot detach Viewer: {error}");
                }
            }
        });
    }
}

impl Render for ViewerWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if let Some(owner) = self.viewer.read(cx).owner.upgrade() {
            window.set_rem_size(px(owner.read(cx).ui_rem_size() as f32));
        }
        let theme = *cx.theme();
        div()
            .size_full()
            .flex()
            .flex_col()
            .min_h_0()
            .bg(theme.active_background)
            .text_color(theme.text)
            .font_weight(FontWeight::NORMAL)
            .text_size(rems(0.8125))
            .child(tool_window::header(
                "icons/eye.svg",
                cx.i18n().text(Message::ViewerTitle),
                window.is_maximized(),
                true,
                cx,
                |window, cx| {
                    tool_window::close_transients(window, cx);
                    window.remove_window();
                },
            ))
            // An absolute box gives the Viewer a definite size: percentage
            // heights inside a flex item would follow the image's natural size.
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .relative()
                    .child(div().absolute().inset_0().child(self.viewer.clone())),
            )
    }
}
