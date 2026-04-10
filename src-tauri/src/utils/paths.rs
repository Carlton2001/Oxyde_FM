use std::path::PathBuf;
use tauri::{AppHandle, Manager};

/// Returns the root directory to be used for app data.
/// If a "data" folder exists in the same directory as the executable,
/// we are in portable mode and return that path.
/// Otherwise, we fall back to the standard Tauri app data directory.
pub fn get_app_data_root(app: &AppHandle) -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        if let Ok(exe_path) = std::env::current_exe() {
            if let Some(exe_dir) = exe_path.parent() {
                let portable_dir = exe_dir.join("data");
                if portable_dir.exists() && portable_dir.is_dir() {
                    return portable_dir;
                }
            }
        }
    }

    // Fallback to standard Tauri path
    app.path().app_local_data_dir().unwrap_or_else(|_| {
        // Absolute fallback if everything else fails (should not happen on Windows)
        PathBuf::from("data") 
    })
}

/// Returns the config directory (settings, etc.)
pub fn get_config_dir(app: &AppHandle) -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        if let Ok(exe_path) = std::env::current_exe() {
            if let Some(exe_dir) = exe_path.parent() {
                let portable_dir = exe_dir.join("data");
                if portable_dir.exists() && portable_dir.is_dir() {
                    return portable_dir.join("config");
                }
            }
        }
    }
    app.path().app_config_dir().unwrap_or_else(|_| PathBuf::from("config"))
}

/// Returns the cache directory (icons, thumbnails, etc.)
pub fn get_cache_dir(app: &AppHandle) -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        if let Ok(exe_path) = std::env::current_exe() {
            if let Some(exe_dir) = exe_path.parent() {
                let portable_dir = exe_dir.join("data");
                if portable_dir.exists() && portable_dir.is_dir() {
                    return portable_dir.join("cache");
                }
            }
        }
    }
    app.path().app_cache_dir().unwrap_or_else(|_| PathBuf::from("cache"))
}

/// Returns the local data directory (session, etc.)
pub fn get_local_data_dir(app: &AppHandle) -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        if let Ok(exe_path) = std::env::current_exe() {
            if let Some(exe_dir) = exe_path.parent() {
                let portable_dir = exe_dir.join("data");
                if portable_dir.exists() && portable_dir.is_dir() {
                    return portable_dir.join("config"); // In portable mode, we group session with config
                }
            }
        }
    }
    app.path().app_local_data_dir().unwrap_or_else(|_| PathBuf::from("data"))
}

/// Returns the logs directory
pub fn get_log_dir(app: &AppHandle) -> PathBuf {
    get_app_data_root(app).join("logs")
}

/// Returns the WebView2 User Data Folder (UDF) path
pub fn get_webview_data_dir(app: &AppHandle) -> PathBuf {
    get_app_data_root(app).join("webview")
}

/// Checks if the application is currently running in portable mode.
pub fn is_portable() -> bool {
    #[cfg(target_os = "windows")]
    {
        if let Ok(exe_path) = std::env::current_exe() {
            if let Some(exe_dir) = exe_path.parent() {
                return exe_dir.join("data").exists();
            }
        }
    }
    false
}
