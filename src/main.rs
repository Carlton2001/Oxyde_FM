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

/// Bytes one session may add to the log: a fault repeating at full speed once wrote 5.6 GB.
const LOG_SESSION_LIMIT: u64 = 32 * 1024 * 1024;

/// The log file, which drops what a session writes past `LOG_SESSION_LIMIT`.
struct SessionLog {
    file: std::fs::File,
    left: u64,
}

impl std::io::Write for SessionLog {
    fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
        if self.left == 0 {
            return Ok(buffer.len());
        }
        if buffer.len() as u64 > self.left {
            self.left = 0;
            self.file.write_all(b"Log size limit reached: this session's later messages are dropped.\n")?;
            return Ok(buffer.len());
        }
        self.left -= buffer.len() as u64;
        self.file.write_all(buffer)?;
        Ok(buffer.len())
    }

    fn flush(&mut self) -> std::io::Result<()> { self.file.flush() }
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
    if let Some((_, file)) = log_file {
        logger.target(env_logger::Target::Pipe(Box::new(SessionLog { file, left: LOG_SESSION_LIMIT })));
    }
    logger.init();
    log::info!("session start; pid={}; version={}; log={path:?}", std::process::id(), env!("CARGO_PKG_VERSION"));
    let previous_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        log::error!("panic: {info}");
        previous_hook(info);
        // The interface thread takes the file choosers down with it: applications
        // waiting on one get the system's instead, before the process ends.
        if std::thread::current().name() == Some("main") {
            oxyde_lib::platform::platform().hand_off_file_choosers();
        }
    }));
}

/// What the uninstaller runs: folders go back to the program Oxyde replaced.
#[cfg(target_os = "windows")]
const RESTORE_FOLDER_HANDLER: &str = "--restore-folder-handler";

/// Leaves the menus of folders, and gives folders back only when this copy is the one
/// they open with: another copy keeps its choice.
#[cfg(target_os = "windows")]
fn restore_folder_handler() {
    let platform = oxyde_lib::platform::platform();
    platform.withdraw_from_folder_menus();
    platform.withdraw_from_file_choosers();
    if platform.opens_folders_by_default() == Some(true) {
        let kept = oxyde_lib::models::ConfigManager::config_path().ok()
            .and_then(|path| oxyde_lib::models::ConfigManager::read_from_path(&path).ok())
            .and_then(|config| config.folder_handler_before);
        if let Err(error) = platform.open_folders_by_default(false, kept.as_deref()) {
            log::warn!("Cannot give folders back before uninstalling: {error}");
        }
    }
    log::logger().flush();
}

fn main() {
    #[cfg(target_os = "windows")]
    if oxyde_lib::commands::icon_session_worker::run_if_requested() { return; }
    #[cfg(target_os = "windows")]
    if oxyde_lib::commands::archive_io_worker::run_if_requested() { return; }
    #[cfg(target_os = "windows")]
    if oxyde_lib::commands::archive_worker::run_if_requested() { return; }
    if crate::ui::preview_worker::run_if_requested() { return; }
    let idle_gpus = oxyde_lib::platform::platform().prefer_display_gpu();
    if let Err(error) = oxyde_lib::app_paths::initialize() {
        let platform = oxyde_lib::platform::platform();
        let locale = platform.regional_locale().unwrap_or_default();
        let i18n = crate::ui::i18n::I18n::for_language(crate::ui::i18n::Language::from_id(&locale));
        platform.show_startup_error("Oxyde", &format!("{}\n\n{error}", i18n.text(crate::ui::i18n::Message::StorageUnavailable)));
        return;
    }
    initialize_logging();
    #[cfg(target_os = "windows")]
    if std::env::args_os().any(|argument| argument == RESTORE_FOLDER_HANDLER) {
        restore_folder_handler();
        return;
    }
    // The task of the taskbar menu: a shortcut cannot ask for elevation itself.
    #[cfg(target_os = "windows")]
    if std::env::args_os().any(|argument| argument == oxyde_lib::platform::AS_ADMINISTRATOR) {
        if let Err(error) = oxyde_lib::platform::platform().start_as_administrator() {
            log::warn!("Cannot start as administrator: {error}");
        }
        log::logger().flush();
        return;
    }
    let startup_locations = oxyde_lib::platform::platform().startup_locations();
    let for_file_chooser = oxyde_lib::platform::platform().started_for_file_chooser();
    // A later launch hands its locations to the running Oxyde and opens no window.
    let instance_requests = match oxyde_lib::platform::platform().claim_instance(&startup_locations) {
        oxyde_lib::platform::InstanceClaim::Forwarded => {
            if for_file_chooser {
                log::info!("file chooser portal handed to the running Oxyde");
            } else {
                log::info!("locations handed to the running Oxyde: {startup_locations:?}");
            }
            log::logger().flush();
            return;
        }
        oxyde_lib::platform::InstanceClaim::Relay => {
            oxyde_lib::platform::platform().relay_file_chooser();
            log::logger().flush();
            return;
        }
        oxyde_lib::platform::InstanceClaim::First(requests) => Some(requests),
        // Nothing could reach a file chooser started without the session bus.
        oxyde_lib::platform::InstanceClaim::Alone if for_file_chooser => return,
        oxyde_lib::platform::InstanceClaim::Alone => None,
    };
    oxyde_lib::platform::platform().identify_process();
    if let Some(drivers) = idle_gpus {
        log::info!("Vulkan drivers disabled: {drivers}");
    }
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
    // The shared native folder icons are ready before any file list is painted.
    // No file operation blocks navigation: the first window does not exist yet.
    let folder_icons = crate::ui::components::icons::prepare_system_folder_icons(config.use_system_icons);
    let asset_source = crate::ui::assets::EmbeddedAssetSource::new();
    gpui_platform::application()
        .with_assets(asset_source)
        .run(move |cx| {
            crate::ui::memory_diagnostics::start(cx);
            cx.set_global(folder_icons);
            let assets = crate::ui::assets::EmbeddedAssetSource::new();
            assets.load_fonts(cx).expect("fonts could not be loaded");
            cx.set_global(crate::ui::theme::OxydeTheme::for_choice(
                &crate::ui::theme::ThemeChoice::from_config(&config.theme, &config.custom_themes),
                &config.custom_themes,
                crate::ui::theme::ColorVision::from_id(&config.color_vision),
            ));
            cx.set_global(crate::ui::i18n::I18n::for_language(
                crate::ui::i18n::Language::from_id(&config.language),
            ));
            cx.set_global(crate::ui::format::DateFormatPreference(date_format));
            // Text fields handle their own editing actions; file shortcuts are
            // handled by the explorer only while its focus handle is focused.
            crate::ui::components::input::bind_keys(cx);
            crate::ui::app::main_window::end_with_last_window(cx);
            let app = cx.new(|cx| OxydeApp::new(config, config_path, load_failed, cx));
            let operations_panel = cx.new(|cx| {
                crate::ui::components::operations::OperationsPanelView::new(app.clone(), cx)
            });
            app.update(cx, |this, cx| {
                this.handle = Some(app.clone());
                this.operations_panel_view = Some(operations_panel);
                this.refresh_system_menus(cx);
                // Also removes a dialog registration whose library is gone, which would
                // leave other applications without Open and Save dialogs.
                this.refresh_file_chooser_setting(cx);
                match instance_requests {
                    // The only Oxyde: its session stays, with the received locations added.
                    Some(requests) => {
                        this.add_startup_locations(&startup_locations);
                        this.receive_instance_requests(requests, cx);
                    }
                    // Another process may own the session: show only what this one received.
                    None => this.show_only_locations(&startup_locations),
                }
                if initialize_date_format && !load_failed { this.save_preferences(cx); }
            });
            if for_file_chooser {
                // Started by the bus for the portal: an Oxyde left without a window, and
                // without a call to answer, does not stay in the background.
                cx.spawn(async move |cx| {
                    cx.background_executor().timer(std::time::Duration::from_secs(30)).await;
                    cx.update(|cx| if cx.windows().is_empty() {
                        crate::ui::app::main_window::end_when_unused(cx);
                    });
                }).detach();
            } else if crate::ui::app::main_window::open(app, cx).is_none() {
                cx.quit();
            }
        });
    trace_shutdown_stage("GPUI event loop returned");
    crate::ui::app::shutdown::finish_pending_cleanup();
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
