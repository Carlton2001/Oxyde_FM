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

pub fn get_office_thumbnail_cached(
    path: String,
    cache_dir: PathBuf,
) -> Result<String, CommandError> {
    let source_path = Path::new(&path);
    if !source_path.exists() {
        return Err(CommandError::PathError(path.clone()));
    }

    // Generate cache filename
    let metadata = fs::metadata(source_path).map_err(|e| CommandError::IoError(e.to_string()))?;
    let modified = metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH);
    let duration = modified.duration_since(SystemTime::UNIX_EPOCH).unwrap_or_default();
    
    let hash_input = format!("{}_{}_{}_office", path, metadata.len(), duration.as_secs());
    let hash = hex::encode(hash_input);
    
    let cache_file = cache_dir.join(format!("{}.jpg", hash));
    
    // Create cache dir if it doesn't exist
    if !cache_dir.exists() {
        fs::create_dir_all(&cache_dir).map_err(|e| CommandError::IoError(e.to_string()))?;
    }

    if cache_file.exists() {
        return Ok(cache_file.to_string_lossy().to_string());
    }

    // Try to open as Zip archive
    let file = fs::File::open(source_path).map_err(|e| CommandError::IoError(e.to_string()))?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| CommandError::Other(e.to_string()))?;

    // Office formats store it as docProps/thumbnail.jpeg
    // LibreOffice stores it as Thumbnails/thumbnail.png
    let target_files = vec!["docProps/thumbnail.jpeg", "Thumbnails/thumbnail.png"];
    
    let mut extracted_data = Vec::new();
    let mut found = false;
    
    for target in target_files {
        if let Ok(mut content_file) = archive.by_name(target) {
            use std::io::Read;
            if content_file.read_to_end(&mut extracted_data).is_ok() {
                found = true;
                break;
            }
        }
    }

    if !found {
        return Err(CommandError::Other("No thumbnail found in archive".to_string()));
    }

    // Attempt to parse the extracted data 
    let img = image::load_from_memory(&extracted_data).map_err(|e| CommandError::Other(format!("Failed to parse embedded thumbnail: {}", e)))?;
    
    // Max dimensions for thumbnails (e.g., 256px) - larger than display for quality
    let (width, height) = img.dimensions();
    let n_width = 256;
    let n_height = if width > 0 { (height as f64 * (n_width as f64 / width as f64)) as u32 } else { 256 };

    let thumbnail = img.resize(n_width, n_height, FilterType::Lanczos3);
    
    // Save as JPEG with 80% quality for efficiency
    thumbnail.save_with_format(&cache_file, image::ImageFormat::Jpeg)
        .map_err(|e| CommandError::Other(format!("Failed to save thumbnail: {}", e)))?;

    Ok(cache_file.to_string_lossy().to_string())
}

pub fn get_office_text_preview(
    path: String,
) -> Result<String, CommandError> {
    let source_path = Path::new(&path);
    if !source_path.exists() {
        return Err(CommandError::PathError(path.clone()));
    }

    let file = fs::File::open(source_path).map_err(|e| CommandError::IoError(e.to_string()))?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| CommandError::Other(e.to_string()))?;

    let ext = source_path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    let targets = match ext.as_str() {
        "docx" | "docm" => vec!["word/document.xml"],
        "xlsx" | "xlsm" => vec!["xl/sharedStrings.xml"],
        "pptx" | "pptm" => vec!["ppt/slides/slide1.xml", "ppt/slides/slide2.xml"],
        "odt" | "ods" | "odp" | "ott" | "ots" | "otp" => vec!["content.xml"],
        _ => vec!["content.xml", "word/document.xml"]
    };

    let mut preview = String::new();
    let mut chars_read = 0;
    let max_chars = 1500;

    for target in targets {
        if chars_read >= max_chars { break; }
        if let Ok(mut content_file) = archive.by_name(target) {
            use std::io::Read;
            // Only read a chunk to preserve memory
            let mut buf = vec![0u8; 10240]; 
            if let Ok(n) = content_file.read(&mut buf) {
                let content = String::from_utf8_lossy(&buf[..n]);
                let mut in_tag = false;
                let mut tag_buffer = String::new();
                
                for c in content.chars() {
                    if chars_read >= max_chars { break; }
                    
                    if c == '<' {
                        in_tag = true;
                        tag_buffer.clear();
                    } else if c == '>' {
                        in_tag = false;
                        let tl = &tag_buffer;
                        if tl.starts_with("w:p") || tl.starts_with("/w:p") ||
                           tl.starts_with("w:br") || tl.starts_with("text:p") ||
                           tl.starts_with("/text:p") || tl == "p" || tl == "/p" {
                            if !preview.ends_with('\n') {
                                preview.push('\n');
                                chars_read += 1;
                            }
                        }
                    } else if in_tag {
                        if tag_buffer.len() < 10 {
                            tag_buffer.push(c);
                        }
                    } else {
                        preview.push(c);
                        chars_read += 1;
                    }
                }
            }
        }
    }

    if preview.trim().is_empty() {
        return Err(CommandError::Other("No text found in archive".to_string()));
    }

    Ok(preview.trim().to_string())
}
