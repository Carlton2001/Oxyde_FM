use crate::models::Result;
use image::RgbaImage;
use std::time::Duration;

pub fn stream_video_frames<F>(path: String, on_frame: F) -> Result<()>
where
    F: FnMut(RgbaImage, Duration) -> bool + Send + 'static,
{
    crate::utils::thumbnails::stream_video_frames_sync(path, on_frame)
}
