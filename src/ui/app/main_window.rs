//! The main window, opened at startup, or later when Oxyde started for the file
//! chooser portal and a launch then asks for folders. Closing it ends Oxyde, unless a
//! file chooser still waits for its answer: the application asking keeps it open.
use super::OxydeApp;
use gpui::*;

/// Oxyde ends with its last window, as GPUI ends by default, once no application waits
/// for a file from it: a dialog Oxyde handed to the system's portal has no window here.
pub(crate) fn end_with_last_window(cx: &mut App) {
    if cfg!(target_os = "windows") {
        return;
    }
    cx.set_quit_mode(QuitMode::Explicit);
    cx.on_window_closed(|cx, _| {
        if cx.windows().is_empty() {
            end_when_unused(cx);
        }
    }).detach();
}

/// Ends Oxyde once it has no window and answers no application.
pub(crate) fn end_when_unused(cx: &mut App) {
    cx.spawn(async move |cx| {
        while oxyde_lib::platform::platform().answering_file_chooser() {
            cx.background_executor().timer(std::time::Duration::from_millis(250)).await;
        }
        cx.update(|cx| if cx.windows().is_empty() {
            cx.quit();
        });
    }).detach();
}

impl OxydeApp {
    /// What only a browsing Oxyde needs: folder watching, the tabs' listings and the
    /// update check. Once per process, before the first main window.
    pub(crate) fn start_browsing(&mut self, cx: &mut Context<Self>) {
        if self.browsing {
            return;
        }
        self.browsing = true;
        self.start_watcher_task(cx);
        self.reconcile_watchers(cx);
        self.refresh_all_tabs(cx);
        self.schedule_update_check(cx);
    }
}

/// Opens the main window on `app`. GPUI draws it before returning: call it outside any
/// update of `app`.
pub(crate) fn open(app: Entity<OxydeApp>, cx: &mut App) -> Option<AnyWindowHandle> {
    app.update(cx, |this, cx| this.start_browsing(cx));
    let mut screens = Vec::new();
    if let Some(primary) = cx.primary_display() { screens.push(primary.visible_bounds()); }
    screens.extend(cx.displays().into_iter().map(|display| display.visible_bounds()));
    let screens = crate::ui::window_geometry::restorable_screens(screens);
    let (rem_size, placement) = {
        let config = &app.read(cx).config;
        (config.ui_rem_size, config.window_placement)
    };
    // Keep the title bar, navigation bar, sidebar and one usable file column visible.
    let rem = crate::ui::scale::normalized_reference(rem_size) as f32;
    let min_size = size(px(rem * 45.0), px(rem * 30.0));
    #[cfg(not(target_os = "windows"))]
    let min_size = crate::ui::window_geometry::fitting_min_size(min_size, &screens, px(rem * 3.0));
    let window_bounds = crate::ui::window_geometry::restore(placement, &screens, min_size);
    let opened = cx.open_window(
        WindowOptions {
            app_id: Some(crate::ui::APP_ID.into()),
            window_decorations: crate::ui::window_decorations(),
            window_bounds: Some(window_bounds),
            window_min_size: Some(min_size),
            titlebar: Some(TitlebarOptions {
                appears_transparent: true,
                title: Some("Oxyde".into()),
                ..Default::default()
            }),
            ..Default::default()
        },
        move |window, cx| {
            app.update(cx, |this, cx| this.observe_main_window(window, cx));
            let viewer_owner = app.downgrade();
            window.on_window_should_close(cx, move |_, cx| {
                let _ = viewer_owner.update(cx, |app, cx| {
                    app.stop_viewers(cx);
                    app.main_window = None;
                });
                // Oxyde ends with its last window, and a file chooser waiting for the
                // user is one: the shutdown starts when it closes.
                if !super::file_chooser::is_open(cx) && !oxyde_lib::platform::platform().answering_file_chooser() {
                    crate::trace_shutdown_request("native window close");
                }
                #[cfg(target_os = "windows")]
                crate::ui::app::close_file_drag(cx);
                crate::ui::components::tooltip::hover::close_native_windows(cx);
                crate::ui::components::popup::close_all(cx);
                crate::ui::app::duplicates::close_duplicate_windows(cx);
                crate::ui::components::operations::close_operations_windows(cx);
                crate::ui::app::advanced_search::close_advanced_search_windows(cx);
                crate::ui::app::theme_window::close_theme_windows(cx);
                crate::ui::app::viewer::close_viewer_windows(cx);
                crate::ui::app::properties::close_properties_windows(cx);
                crate::ui::app::close_dialog_windows(cx);
                true
            });
            let focus_handle = app.read(cx).focus_handle.clone();
            #[cfg(target_os = "windows")]
            let drag_owner = app.clone();
            window.on_next_frame(move |window, cx| {
                #[cfg(target_os = "windows")]
                crate::ui::app::OxydeApp::install_file_drag(&drag_owner, window, cx);
                window.focus(&focus_handle, cx);
                match raw_window_handle::HasWindowHandle::window_handle(window) {
                    Ok(handle) => {
                        let platform = oxyde_lib::platform::platform();
                        if let Err(error) = platform.install_pointer_capture(handle.as_raw()) {
                            log::warn!("Cannot install pointer capture: {error}");
                        }
                        if let Err(error) = platform.install_disk_image_autoplay_handler(handle.as_raw()) {
                            log::warn!("Cannot install disk-image AutoPlay handler: {error}");
                        }
                        let assets = crate::ui::assets::EmbeddedAssetSource::new();
                        if let Ok(Some(icon_bytes)) = gpui::AssetSource::load(&assets, "icons/logo.png") {
                            platform.set_application_icon(handle.as_raw(), &icon_bytes);
                        }
                    }
                    Err(error) => log::warn!("Cannot access Oxyde window handle: {error}"),
                }
            });
            app
        },
    );
    match opened {
        Ok(window) => Some(window.into()),
        Err(error) => {
            log::error!("Cannot open the main window: {error}");
            None
        }
    }
}
