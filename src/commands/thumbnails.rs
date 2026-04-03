use crate::models::{Result, CommandError};
use crate::utils::thumbnails::get_thumbnail_cached;

pub async fn get_image_thumbnail(
    path: String,
) -> Result<String> {
    let cache_dir = dirs::cache_dir()
        .ok_or_else(|| CommandError::SystemError("Failed to get cache directory".to_string()))?
        .join("oxyde")
        .join("thumbnails");

    tokio::task::spawn_blocking(move || {
        get_thumbnail_cached(path, cache_dir)
    }).await.map_err(|e| CommandError::Other(format!("Thread panic: {}", e)))?
}

pub async fn get_office_thumbnail(
    path: String,
) -> Result<String> {
    let cache_dir = dirs::cache_dir()
        .ok_or_else(|| CommandError::SystemError("Failed to get cache directory".to_string()))?
        .join("oxyde")
        .join("thumbnails");

    tokio::task::spawn_blocking(move || {
        crate::utils::thumbnails::get_office_thumbnail_cached(path, cache_dir)
    }).await.map_err(|e| CommandError::Other(format!("Thread panic: {}", e)))?
}

pub async fn get_office_text_preview(
    path: String,
) -> Result<String> {
    tokio::task::spawn_blocking(move || {
        crate::utils::thumbnails::get_office_text_preview(path)
    }).await.map_err(|e| CommandError::Other(format!("Thread panic: {}", e)))?
}
