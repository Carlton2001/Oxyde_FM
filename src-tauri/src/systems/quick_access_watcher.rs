use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::PathBuf;
use tauri::{AppHandle, Emitter};

pub fn setup_quick_access_watcher(app_handle: AppHandle) {
    #[cfg(target_os = "windows")]
    {
        std::thread::spawn(move || {
            let app_data = std::env::var("APPDATA").unwrap_or_default();
            if app_data.is_empty() {
                return;
            }

            let watch_path = PathBuf::from(app_data)
                .join("Microsoft")
                .join("Windows")
                .join("Recent")
                .join("AutomaticDestinations");

            if !watch_path.exists() {
                return;
            }

            let app_handle_clone = app_handle.clone();
            let watcher = RecommendedWatcher::new(
                move |res| match res {
                    Ok(_) => {
                        // Quick Access files modified, emit event to frontend
                        let _ = app_handle_clone.emit("quick-access-changed", ());
                    }
                    Err(e) => log::error!("Quick Access watcher error: {:?}", e),
                },
                Config::default(),
            )
            .ok();

            if let Some(mut w) = watcher {
                if let Err(e) = w.watch(&watch_path, RecursiveMode::NonRecursive) {
                    log::error!("Failed to watch Quick Access folder: {:?}", e);
                } else {
                    // Keep the watcher alive in this thread
                    loop {
                        std::thread::sleep(std::time::Duration::from_secs(3600));
                    }
                }
            }
        });
    }
}
