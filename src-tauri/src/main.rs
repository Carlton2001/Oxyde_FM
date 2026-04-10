// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // Portable Mode: Redirect WebView2 data folder to local "data" directory if it exists
    #[cfg(target_os = "windows")]
    {
        if let Ok(exe_path) = std::env::current_exe() {
            if let Some(exe_dir) = exe_path.parent() {
                let portable_data_dir = exe_dir.join("data");
                if portable_data_dir.exists() && portable_data_dir.is_dir() {
                    let webview_dir = portable_data_dir.join("webview");
                    std::env::set_var("WEBVIEW2_USER_DATA_FOLDER", webview_dir);
                }
            }
        }
    }

    oxyde_lib::run()
}
