use std::path::{Path, PathBuf};
use std::fs;
use std::time::SystemTime;
use std::io::BufWriter;
use image::{imageops::FilterType, GenericImageView};
use image::codecs::jpeg::JpegEncoder;
use crate::models::CommandError;

use once_cell::sync::Lazy;
use std::sync::{Mutex, Condvar};

/// Simple concurrency limiter using Mutex + Condvar (std::sync::Semaphore is unstable).
/// Limits concurrent thumbnail generation threads to MAX_CONCURRENT.
struct ConcurrencyLimiter {
    state: Mutex<u32>,
    cvar: Condvar,
    max: u32,
}

impl ConcurrencyLimiter {
    fn new(max: u32) -> Self {
        Self { state: Mutex::new(0), cvar: Condvar::new(), max }
    }
    fn acquire(&self) {
        let mut count = self.state.lock().unwrap();
        while *count >= self.max {
            count = self.cvar.wait(count).unwrap();
        }
        *count += 1;
    }
    fn release(&self) {
        let mut count = self.state.lock().unwrap();
        *count -= 1;
        self.cvar.notify_one();
    }
}

static THUMB_LIMITER: Lazy<ConcurrencyLimiter> = Lazy::new(|| ConcurrencyLimiter::new(4));

/// Target thumbnail size in pixels (longest side). 128px is plenty for grid view.
const THUMB_SIZE: u32 = 128;

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
    
    let hash_input = format!("{}_{}_{}", path, metadata.len(), duration.as_secs());
    let hash = hex::encode(hash_input);
    
    let cache_file = cache_dir.join(format!("{}.jpg", hash));
    
    if !cache_dir.exists() {
        fs::create_dir_all(&cache_dir).map_err(|e| CommandError::IoError(e.to_string()))?;
    }

    // Return cached version immediately if it exists
    if cache_file.exists() {
        return Ok(cache_file.to_string_lossy().to_string());
    }

    // Acquire permit – blocks if 4 threads are already generating thumbnails
    THUMB_LIMITER.acquire();

    // Double-check cache after acquiring permit (another thread may have generated it)
    if cache_file.exists() {
        THUMB_LIMITER.release();
        return Ok(cache_file.to_string_lossy().to_string());
    }

    // Generate thumbnail – wrapped to guarantee release on all paths
    let result = (|| -> Result<(), CommandError> {
        let img = image::open(source_path).map_err(|e| CommandError::Other(format!("Failed to open image: {}", e)))?;
        
        let (width, height) = img.dimensions();
        let (n_width, n_height) = if width >= height {
            (THUMB_SIZE, (height as f64 * (THUMB_SIZE as f64 / width as f64)).max(1.0) as u32)
        } else {
            ((width as f64 * (THUMB_SIZE as f64 / height as f64)).max(1.0) as u32, THUMB_SIZE)
        };

        let thumbnail = img.resize(n_width, n_height, FilterType::Nearest);
        
        let out_file = fs::File::create(&cache_file)
            .map_err(|e| CommandError::IoError(e.to_string()))?;
        let writer = BufWriter::new(out_file);
        let encoder = JpegEncoder::new_with_quality(writer, 75);
        thumbnail.write_with_encoder(encoder)
            .map_err(|e| CommandError::Other(format!("Failed to save thumbnail: {}", e)))?;
        Ok(())
    })();

    THUMB_LIMITER.release();
    result?;

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
    
    let (width, height) = img.dimensions();
    let (n_width, n_height) = if width >= height {
        (THUMB_SIZE, if width > 0 { (height as f64 * (THUMB_SIZE as f64 / width as f64)).max(1.0) as u32 } else { THUMB_SIZE })
    } else {
        (if height > 0 { (width as f64 * (THUMB_SIZE as f64 / height as f64)).max(1.0) as u32 } else { THUMB_SIZE }, THUMB_SIZE)
    };

    let thumbnail = img.resize(n_width, n_height, FilterType::Nearest);
    
    let out_file = fs::File::create(&cache_file)
        .map_err(|e| CommandError::IoError(e.to_string()))?;
    let writer = BufWriter::new(out_file);
    let encoder = JpegEncoder::new_with_quality(writer, 75);
    thumbnail.write_with_encoder(encoder)
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
