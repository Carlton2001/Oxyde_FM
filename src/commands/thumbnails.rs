use crate::models::Result;
use image::RgbaImage;
use std::time::Duration;

pub async fn get_image_thumbnail(path: String) -> Result<RgbaImage> {
    tokio::task::spawn_blocking(move || crate::utils::thumbnails::get_thumbnail_raw(path))
        .await
        .map_err(|e| crate::models::CommandError::Other(format!("Thread panic: {}", e)))?
}

pub async fn get_office_thumbnail(path: String) -> Result<RgbaImage> {
    tokio::task::spawn_blocking(move || crate::utils::thumbnails::get_office_thumbnail_raw(path))
        .await
        .map_err(|e| crate::models::CommandError::Other(format!("Thread panic: {}", e)))?
}

pub async fn get_pdf_thumbnail(path: String) -> Result<RgbaImage> {
    tokio::task::spawn_blocking(move || crate::utils::thumbnails::get_pdf_thumbnail_raw(path))
        .await
        .map_err(|e| crate::models::CommandError::Other(format!("Thread panic: {}", e)))?
}

pub fn stream_video_frames<F>(path: String, on_frame: F) -> Result<()>
where
    F: FnMut(RgbaImage, Duration) -> bool + Send + 'static,
{
    crate::utils::thumbnails::stream_video_frames_sync(path, on_frame)
}

pub async fn get_office_text_preview(path: String) -> Result<String> {
    tokio::task::spawn_blocking(move || crate::utils::thumbnails::get_office_text_preview(path))
        .await
        .map_err(|e| crate::models::CommandError::Other(format!("Thread panic: {}", e)))?
}
