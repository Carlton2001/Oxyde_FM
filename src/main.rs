#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod ui;

use crate::ui::app::OxydeApp;
use gpui::*;
use std::sync::atomic::{AtomicBool, Ordering};

static SHUTDOWN_STARTED: AtomicBool = AtomicBool::new(false);
static SHUTDOWN_FINISHED: AtomicBool = AtomicBool::new(false);
static SHUTDOWN_STAGE: std::sync::Mutex<&str> = std::sync::Mutex::new("not started");

pub(crate) fn trace_shutdown_stage(stage: &'static str) {
    if let Ok(mut current) = SHUTDOWN_STAGE.lock() { *current = stage; }
    log::info!("shutdown: {stage}");
}

pub(crate) fn trace_shutdown_request(source: &'static str) {
    log::info!("shutdown requested: {source}");
    if SHUTDOWN_STARTED.swap(true, Ordering::SeqCst) { return; }
    trace_shutdown_stage("waiting for GPUI quit hook");
    // Independent of GPUI and Tokio: still reports if either executor is stuck.
    if let Err(error) = std::thread::Builder::new().name("shutdown-diagnostics".into()).spawn(|| {
        let started = std::time::Instant::now();
        while !SHUTDOWN_FINISHED.load(Ordering::SeqCst) {
            std::thread::sleep(std::time::Duration::from_secs(3));
            if SHUTDOWN_FINISHED.load(Ordering::SeqCst) { break; }
            let stage = SHUTDOWN_STAGE.lock().map(|stage| *stage).unwrap_or("unknown");
            log::warn!("shutdown still pending after {:?}; last stage: {stage}", started.elapsed());
        }
    }) {
        log::warn!("Cannot start shutdown diagnostics: {error}");
    }
}

fn initialize_logging() {
    use std::io::Write;
    let mut logger = env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn"));
    logger.filter_module("oxyde", log::LevelFilter::Info)
        .filter_module("oxyde_lib", log::LevelFilter::Info)
        .format(|buffer, record| writeln!(buffer, "{} [{:?}] {} {}: {}",
            buffer.timestamp_millis(), std::thread::current().id(), record.level(), record.target(), record.args()));
    let log_file = oxyde_lib::models::ConfigManager::config_path().ok().and_then(|config| {
        let directory = config.parent()?;
        std::fs::create_dir_all(directory).ok()?;
        let path = directory.join("oxyde.log");
        if std::fs::metadata(&path).is_ok_and(|metadata| metadata.len() > 2 * 1024 * 1024) {
            let previous = directory.join("oxyde.previous.log");
            let _ = std::fs::remove_file(&previous);
            let _ = std::fs::rename(&path, previous);
        }
        std::fs::OpenOptions::new().create(true).append(true).open(&path).ok().map(|file| (path, file))
    });
    let path = log_file.as_ref().map(|(path, _)| path.clone());
    if let Some((_, file)) = log_file { logger.target(env_logger::Target::Pipe(Box::new(file))); }
    logger.init();
    log::info!("session start; pid={}; version={}; log={path:?}", std::process::id(), env!("CARGO_PKG_VERSION"));
    let previous_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        log::error!("panic: {info}");
        previous_hook(info);
    }));
}

fn main() {
    initialize_logging();
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("Failed to create Tokio runtime");
    let guard = runtime.enter();

    let config_path = oxyde_lib::models::ConfigManager::config_path().ok();
    let (mut config, load_failed) = match config_path.as_deref() {
        Some(path) => match oxyde_lib::models::ConfigManager::read_from_path(path) {
            Ok(config) => (config, false),
            Err(error) => {
                log::warn!("Cannot load preferences: {error:?}");
                (oxyde_lib::models::AppConfig::default(), true)
            }
        },
        None => (oxyde_lib::models::AppConfig::default(), true),
    };

    let initialize_date_format = oxyde_lib::models::preferences::DateFormat::from_id(&config.date_format).is_none();
    let date_format = oxyde_lib::models::preferences::DateFormat::from_id(&config.date_format)
        .unwrap_or_else(|| oxyde_lib::models::preferences::DateFormat::for_locale(
            &oxyde_lib::platform::platform().regional_locale().unwrap_or_default()));
    config.date_format = date_format.as_str().into();
    let asset_source = crate::ui::assets::EmbeddedAssetSource::new();
    gpui_platform::application()
        .with_assets(asset_source)
        .run(move |cx| {
            let assets = crate::ui::assets::EmbeddedAssetSource::new();
            assets.load_fonts(cx).expect("fonts could not be loaded");
            cx.set_global(crate::ui::theme::OxydeTheme::for_id(
                crate::ui::theme::ThemeId::from_id(&config.theme),
            ));
            cx.set_global(crate::ui::i18n::I18n::for_language(
                crate::ui::i18n::Language::from_id(&config.language),
            ));
            cx.set_global(crate::ui::format::DateFormatPreference(date_format));
            OxydeApp::bind_keys(cx);
            // GPUI gives later bindings precedence. Text editing shortcuts must
            // override file actions while the TextInput context is focused.
            crate::ui::components::input::bind_keys(cx);
            let app = cx.new(|cx| OxydeApp::new(config, config_path, load_failed, cx));
            let operations_panel = cx.new(|cx| {
                crate::ui::components::operations::OperationsPanelView::new(app.clone(), cx)
            });
            app.update(cx, |this, cx| {
                this.handle = Some(app.clone());
                this.operations_panel_view = Some(operations_panel);
                if initialize_date_format && !load_failed { this.save_preferences(cx); }
                this.start_watcher_task(cx);
                this.reconcile_watchers(cx);
                this.refresh_all_tabs(cx);
            });
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds {
                        origin: point(px(100.0), px(100.0)),
                        size: size(px(1200.0), px(800.0)),
                    })),
                    titlebar: Some(TitlebarOptions {
                        appears_transparent: true,
                        title: Some("Oxyde GPUI".into()),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                move |window, cx| {
                    window.on_window_should_close(cx, |_, _| {
                        trace_shutdown_request("native window close");
                        true
                    });
                    let focus_handle = app.read(cx).focus_handle.clone();
                    window.on_next_frame(move |window, cx| {
                        window.focus(&focus_handle, cx);
                        match raw_window_handle::HasWindowHandle::window_handle(window) {
                            Ok(handle) => {
                                let platform = oxyde_lib::platform::platform();
                                if let Err(error) = platform.install_disk_image_autoplay_handler(handle.as_raw()) {
                                    log::warn!("Cannot install disk-image AutoPlay handler: {error}");
                                }
                                let assets = crate::ui::assets::EmbeddedAssetSource::new();
                                if let Ok(Some(icon_bytes)) = assets.load("icons/logo.png") {
                                    platform.set_application_icon(handle.as_raw(), &icon_bytes);
                                }
                            }
                            Err(error) => log::warn!("Cannot access Oxyde window handle: {error}"),
                        }
                    });
                    app
                },
            )
            .expect("window opens");
        });
    trace_shutdown_stage("GPUI event loop returned");
    drop(guard);
    shutdown_runtime(runtime);
    trace_shutdown_stage("application cleanup complete; final process exit");
    SHUTDOWN_FINISHED.store(true, Ordering::SeqCst);
    log::logger().flush();
    if let Err(error) = oxyde_lib::platform::platform().finish_process_shutdown() {
        log::error!("Final process exit failed: {error}");
        log::logger().flush();
    }
}

fn shutdown_runtime(runtime: tokio::runtime::Runtime) {
    trace_shutdown_stage("Tokio shutdown started");
    // GPUI has already run the quit hooks (including preference saving).
    // Blocking filesystem / shell workers cannot be aborted by Tokio and may
    // be stuck in an OS call. Dropping the runtime would wait for them forever.
    runtime.shutdown_timeout(std::time::Duration::from_secs(2));
    trace_shutdown_stage("Tokio shutdown completed");
}

#[cfg(test)]
mod tests {
    #[test]
    fn shutdown_returns_while_a_blocking_worker_is_still_running() {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .build()
            .unwrap();
        let (started_tx, started_rx) = std::sync::mpsc::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        let (finished_tx, finished_rx) = std::sync::mpsc::channel();
        runtime.spawn_blocking(move || {
            started_tx.send(()).unwrap();
            release_rx.recv().unwrap();
            finished_tx.send(()).unwrap();
        });
        started_rx.recv_timeout(std::time::Duration::from_secs(5)).unwrap();

        let (shutdown_tx, shutdown_rx) = std::sync::mpsc::channel();
        let shutdown = std::thread::spawn(move || {
            super::shutdown_runtime(runtime);
            shutdown_tx.send(()).unwrap();
        });
        let result = shutdown_rx.recv_timeout(std::time::Duration::from_secs(5));

        // Keep the worker blocked until the deadline, then release it even on
        // failure so a regression fails the test instead of hanging the suite.
        release_tx.send(()).unwrap();
        finished_rx.recv_timeout(std::time::Duration::from_secs(5)).unwrap();
        shutdown.join().unwrap();
        assert!(result.is_ok(), "runtime shutdown waited for a blocked worker");
    }
}
