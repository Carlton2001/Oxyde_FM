use std::path::{Path, PathBuf};
use std::fs;
use std::time::SystemTime;
use image::{imageops::FilterType, GenericImageView};
use crate::models::CommandError;

pub fn get_thumbnail_cached(
    path: String,
    cache_dir: PathBuf,
) -> Result<String, CommandError> {
    let source_path = Path::new(&path);
    if !source_path.exists() {
        return Err(CommandError::PathError(path));
    }

    // Generate a unique cache name based on path and modification time
    let metadata = fs::metadata(source_path).map_err(|e| CommandError::IoError(e.to_string()))?;
    let modified = metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH);
    let duration = modified.duration_since(SystemTime::UNIX_EPOCH).unwrap_or_default();
    
    // Use path + modified to make a unique string, then hex encode it
    let hash_input = format!("{}_{}_{}", path, metadata.len(), duration.as_secs());
    let hash = hex::encode(hash_input);
    
    let cache_file = cache_dir.join(format!("{}.jpg", hash));
    
    // Create cache dir if it doesn't exist
    if !cache_dir.exists() {
        fs::create_dir_all(&cache_dir).map_err(|e| CommandError::IoError(e.to_string()))?;
    }

    // If cached version exists, return it as base64 or a path?
    // Returning base64 is safer for Tauri local asset protocols if we want to avoid extra setup.
    // But for 100 images, base64 might be slightly heavier than convertFileSrc of a cached file.
    // Let's return the absolute path to the cached file so the frontend can use convertFileSrc.
    
    if cache_file.exists() {
        return Ok(cache_file.to_string_lossy().to_string());
    }

    // Generate thumbnail
    let img = image::open(source_path).map_err(|e| CommandError::Other(format!("Failed to open image: {}", e)))?;
    
    // Max dimensions for thumbnails (e.g., 256px) - larger than display for quality
    let (width, height) = img.dimensions();
    let n_width = 256;
    let n_height = (height as f64 * (n_width as f64 / width as f64)) as u32;

    let thumbnail = img.resize(n_width, n_height, FilterType::Lanczos3);
    
    // Save as JPEG with 80% quality for efficiency
    thumbnail.save_with_format(&cache_file, image::ImageFormat::Jpeg)
        .map_err(|e| CommandError::Other(format!("Failed to save thumbnail: {}", e)))?;

    Ok(cache_file.to_string_lossy().to_string())
}
