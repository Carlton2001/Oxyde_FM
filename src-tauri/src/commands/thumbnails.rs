use tauri::{AppHandle, Manager};
use crate::models::{Result, CommandError};
use crate::utils::thumbnails::get_thumbnail_cached;

#[tauri::command]
pub async fn get_image_thumbnail(
    app: AppHandle,
    path: String,
) -> Result<String> {
    // Get the app's cache directory
    let cache_dir = app.path().app_cache_dir()
        .map_err(|e| CommandError::IoError(e.to_string()))?
        .join("thumbnails");

    // Offload CPU intensive resizing to a dedicated thread pool to keep the async bridge responsive
    tokio::task::spawn_blocking(move || {
        get_thumbnail_cached(path, cache_dir)
    }).await.map_err(|e| CommandError::Other(format!("Thread panic: {}", e)))?
}

#[tauri::command]
pub async fn get_office_thumbnail(
    app: AppHandle,
    path: String,
) -> Result<String> {
    // Get the app's cache directory
    let cache_dir = app.path().app_cache_dir()
        .map_err(|e| CommandError::IoError(e.to_string()))?
        .join("thumbnails");

    tokio::task::spawn_blocking(move || {
        crate::utils::thumbnails::get_office_thumbnail_cached(path, cache_dir)
    }).await.map_err(|e| CommandError::Other(format!("Thread panic: {}", e)))?
}

#[tauri::command]
pub async fn get_office_text_preview(
    path: String,
) -> Result<String> {
    tokio::task::spawn_blocking(move || {
        crate::utils::thumbnails::get_office_text_preview(path)
    }).await.map_err(|e| CommandError::Other(format!("Thread panic: {}", e)))?
}
