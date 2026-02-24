use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};
use crate::models::{FileEntry, CommandError};
use crate::utils::path_security::validate_path;
pub use crate::utils::archive::{ArchiveFormat, is_archive, split_virtual_path};
use log::info;
use tauri::command;
use zip::ZipArchive;
use sevenz_rust as sevenz;
use tar::Archive as TarArchive;
use flate2::read::GzDecoder;
use xz2::read::XzDecoder;
use bzip2::read::BzDecoder;
use zstd::stream::read::Decoder as ZstdDecoder;
use zstd::stream::write::Encoder as ZstdEncoder;
use iso9660_core::iso9660entry::{IsISO9660Record, ISO9660Record};
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::State;

pub struct ArchiveState(pub AtomicBool);

pub fn remove_items_from_archive(archive_path: PathBuf, internal_paths: Vec<String>) -> Result<(), CommandError> {
    let format = ArchiveFormat::from_path(&archive_path).ok_or(CommandError::ArchiveError("Unsupported archive format".to_string()))?;
    
    match format {
        ArchiveFormat::Zip => remove_from_zip(&archive_path, &internal_paths),
        _ => Err(CommandError::ArchiveError("Deleting from this archive format is not supported yet.".to_string())),
    }
}

fn remove_from_zip(archive_path: &Path, internal_paths: &[String]) -> Result<(), CommandError> {
    let file = File::open(archive_path).map_err(|e| CommandError::IoError(e.to_string()))?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| CommandError::ArchiveError(e.to_string()))?;
    
    let temp_file_path = archive_path.with_extension("zip.tmp");
    let temp_file = File::create(&temp_file_path).map_err(|e| CommandError::IoError(e.to_string()))?;
    let mut writer = zip::ZipWriter::new(temp_file);

    for i in 0..archive.len() {
        let entry = archive.by_index(i).map_err(|e| CommandError::ArchiveError(e.to_string()))?;
        let entry_name = entry.name().to_string();
        
        let should_remove = internal_paths.iter().any(|p| {
            let normalized_p = p.replace('\\', "/");
            let normalized_archive_entry = entry_name.replace('\\', "/");
            
            if normalized_archive_entry == normalized_p {
                return true;
            }
            
            // Handle directory removal: if normalized_p is a dir, it might end with / or not.
            // If it ends with /, check starts_with.
            // If it doesn't, add / and check starts_with.
            let dir_prefix = if normalized_p.ends_with('/') {
                normalized_p.clone()
            } else {
                format!("{}/", normalized_p)
            };
            
            normalized_archive_entry.starts_with(&dir_prefix)
        });

        if !should_remove {
            writer.raw_copy_file(entry).map_err(|e| CommandError::ArchiveError(e.to_string()))?;
        }
    }
    
    let _temp_file = writer.finish().map_err(|e| CommandError::ArchiveError(e.to_string()))?;
    
    // Explicitly drop everything to close file handles before disk operations
    drop(archive);
    
    std::fs::remove_file(archive_path).map_err(|e| CommandError::IoError(e.to_string()))?;
    std::fs::rename(temp_file_path, archive_path).map_err(|e| CommandError::IoError(e.to_string()))?;

    Ok(())
}

#[command]
pub fn list_archive_contents(archive_path: String, internal_path: String) -> Result<Vec<FileEntry>, CommandError> {
    let path_buf = validate_path(&archive_path)?;
    let path = path_buf.as_path();
    let format = ArchiveFormat::from_path(path).ok_or(CommandError::ArchiveError("Unsupported archive format".to_string()))?;

    match format {
        ArchiveFormat::Zip => list_zip(path, &internal_path),
        ArchiveFormat::SevenZip => list_seven_zip(path, &internal_path),
        ArchiveFormat::Tar | ArchiveFormat::TarGz | ArchiveFormat::TarXz | ArchiveFormat::TarZst | ArchiveFormat::TarBz2 => list_tar(path, &internal_path, format),
        ArchiveFormat::Iso => list_iso(path, &internal_path),
        ArchiveFormat::Rar => Err(CommandError::ArchiveError("Rar navigation not supported yet. Please extract it first.".to_string())),
    }
}

fn list_iso(path: &Path, internal_prefix: &str) -> Result<Vec<FileEntry>, CommandError> {
    let file = File::open(path).map_err(|e| CommandError::IoError(e.to_string()))?;
    let mut iso = iso9660_core::ISO9660::load(file).map_err(|e| CommandError::ArchiveError(format!("{:?}", e)))?;
    
    let internal_path = if internal_prefix.is_empty() { "/" } else { internal_prefix };
    let mut results = Vec::new();

    let mut entries_iter = iso.listdir(internal_path).map_err(|e| CommandError::ArchiveError(format!("{:?}", e)))?;
    
    // UniqueISO9660RecordIterator::next takes &mut ISO9660<T>
    loop {
        let record = entries_iter.next(&mut iso);
        let Some(rec) = record else { break; };
        
        let name = match &rec {
            ISO9660Record::Directory(d) => d.identifier(),
            ISO9660Record::File(f) => f.identifier(),
        };

        if name == "." || name == ".." { continue; }
        
        let display_name = name.split(';').next().unwrap_or(&name).to_string();
        if display_name.is_empty() { continue; }

        let is_dir = matches!(rec, ISO9660Record::Directory(_));
        let size = match &rec {
            ISO9660Record::File(f) => f.data_length() as u64,
            _ => 0,
        };

        let internal_norm = internal_prefix.trim_start_matches(['/', '\\']).replace('/', "\\");
        let full_virtual_path = if internal_norm.is_empty() {
            format!("{}\\{}", path.to_string_lossy(), display_name)
        } else {
            format!("{}\\{}\\{}", path.to_string_lossy(), internal_norm, display_name)
        }.replace("\\\\", "\\");

        results.push(FileEntry {
            name: display_name,
            path: full_virtual_path,
            is_dir,
            is_hidden: false,
            is_system: false,
            is_symlink: false,
            is_junction: false,
            size,
            is_calculated: false,
            modified: 0,
            is_readonly: true,
            original_path: None,
            deleted_time: None,
        });
    }

    Ok(results)
}

fn list_zip(path: &Path, internal_prefix: &str) -> Result<Vec<FileEntry>, CommandError> {
    let file = File::open(path).map_err(|e| CommandError::IoError(e.to_string()))?;
    let mut archive = ZipArchive::new(file).map_err(|e| CommandError::ArchiveError(e.to_string()))?;
    let mut entries = Vec::new();
    let prefix = if internal_prefix.is_empty() { "".to_string() } else { format!("{}/", internal_prefix.trim_end_matches('/')) };

    let mut seen = std::collections::HashSet::new();

    for i in 0..archive.len() {
        let file = archive.by_index(i).map_err(|e| CommandError::ArchiveError(e.to_string()))?;
        let name = file.name().replace('\\', "/");
        
        if name.starts_with(&prefix) && name != prefix {
            let relative = &name[prefix.len()..];
            let parts: Vec<&str> = relative.split('/').filter(|s| !s.is_empty()).collect();
            if parts.is_empty() { continue; }
            
            let entry_name = parts[0].to_string();
            if seen.contains(&entry_name) { continue; }
            seen.insert(entry_name.clone());

            let is_dir = file.is_dir() || parts.len() > 1;
            let full_virtual_path = format!("{}\\{}\\{}", path.to_string_lossy(), internal_prefix.replace('/', "\\"), entry_name).replace("\\\\", "\\");

            entries.push(FileEntry {
                name: entry_name,
                path: full_virtual_path,
                is_dir,
                is_hidden: false,
                is_system: false,
                is_symlink: false,
                is_junction: false,
                size: if is_dir { 0 } else { file.size() },
                is_calculated: false,
                modified: file.last_modified()
                    .and_then(|dt| {
                        let t: Result<time::OffsetDateTime, _> = dt.try_into();
                        t.ok()
                    })
                    .map(|ts| ts.unix_timestamp() as u64 * 1000)
                    .unwrap_or(0),
                is_readonly: false,
                original_path: None,
                deleted_time: None,
            });
        }
    }
    Ok(entries)
}

fn list_seven_zip(path: &Path, internal_prefix: &str) -> Result<Vec<FileEntry>, CommandError> {
    let file = File::open(path).map_err(|e| CommandError::IoError(e.to_string()))?;
    let len = file.metadata().map_err(|e| CommandError::IoError(e.to_string()))?.len();
    let mut reader = sevenz::SevenZReader::new(file, len, "".into()).map_err(|e| CommandError::ArchiveError(e.to_string()))?;
    let mut entries = Vec::new();
    let prefix = if internal_prefix.is_empty() { "".to_string() } else { format!("{}/", internal_prefix.trim_end_matches('/')) };
    let mut seen = std::collections::HashSet::new();

    reader.for_each_entries(|entry, _reader| {
        let name = entry.name().replace('\\', "/");
        if name.starts_with(&prefix) && name != prefix {
             let relative = &name[prefix.len()..];
             let parts: Vec<&str> = relative.split('/').filter(|s| !s.is_empty()).collect();
             if !parts.is_empty() {
                 let entry_name = parts[0].to_string();
                 if !seen.contains(&entry_name) {
                     seen.insert(entry_name.clone());
                     let is_dir = entry.is_directory() || parts.len() > 1;
                     entries.push(FileEntry {
                        name: entry_name.clone(),
                        path: format!("{}\\{}", path.to_string_lossy(), name.replace('/', "\\")),
                        is_dir,
                        is_hidden: false,
                        is_system: false,
                        is_symlink: false,
                        is_junction: false,
                        size: entry.size(),
                        is_calculated: false,
                        modified: 0, // sevenz-rust entry modified is complex to get
                        is_readonly: false,
                        original_path: None,
                        deleted_time: None,
                    });
                 }
             }
        }
        Ok(true)
    }).map_err(|e| CommandError::ArchiveError(e.to_string()))?;

    Ok(entries)
}

fn list_tar(path: &Path, internal_prefix: &str, format: ArchiveFormat) -> Result<Vec<FileEntry>, CommandError> {
    let file = File::open(path).map_err(|e| CommandError::IoError(e.to_string()))?;
    let reader: Box<dyn io::Read> = match format {
        ArchiveFormat::TarGz => Box::new(GzDecoder::new(file)),
        ArchiveFormat::TarXz => Box::new(XzDecoder::new(file)),
        ArchiveFormat::TarBz2 => Box::new(BzDecoder::new(file)),
        ArchiveFormat::TarZst => Box::new(ZstdDecoder::new(file).map_err(|e| CommandError::IoError(e.to_string()))?),
        _ => Box::new(file),
    };

    let mut archive = TarArchive::new(reader);
    let mut entries = Vec::new();
    let prefix = if internal_prefix.is_empty() { "".to_string() } else { format!("{}/", internal_prefix.trim_end_matches('/')) };
    let mut seen = std::collections::HashSet::new();

    for entry in archive.entries().map_err(|e| CommandError::ArchiveError(e.to_string()))? {
        let entry = entry.map_err(|e| CommandError::ArchiveError(e.to_string()))?;
        let path_field = entry.path().map_err(|e| CommandError::ArchiveError(e.to_string()))?;
        let name = path_field.to_string_lossy().replace('\\', "/");

        if name.starts_with(&prefix) && name != prefix {
            let relative = &name[prefix.len()..];
            let parts: Vec<&str> = relative.split('/').filter(|s| !s.is_empty()).collect();
            if parts.is_empty() { continue; }
            
            let entry_name = parts[0].to_string();
            if seen.contains(&entry_name) { continue; }
            seen.insert(entry_name.clone());

            let is_dir = entry.header().entry_type().is_dir() || parts.len() > 1;
            entries.push(FileEntry {
                name: entry_name,
                path: format!("{}\\{}", path.to_string_lossy(), name.replace('/', "\\")),
                is_dir,
                is_hidden: false,
                is_system: false,
                is_symlink: false,
                is_junction: false,
                size: entry.header().size().unwrap_or(0),
                is_calculated: false,
                modified: entry.header().mtime().unwrap_or(0) * 1000,
                is_readonly: false,
                original_path: None,
                deleted_time: None,
            });
        }
    }
    Ok(entries)
}

#[command]
pub async fn extract_archive(archive_path: String, target_dir: String, state: State<'_, ArchiveState>) -> Result<(), CommandError> {
    state.0.store(false, Ordering::Relaxed);
    let path_buf = validate_path(&archive_path)?;
    let path = path_buf.as_path();
    let format = ArchiveFormat::from_path(path).ok_or(CommandError::ArchiveError("Unsupported archive format".to_string()))?;
    let target_buf = validate_path(&target_dir)?;
    let target = target_buf.as_path();

    info!("Extracting {:?} to {:?}", path, target);

    if !target.exists() {
        fs::create_dir_all(target).map_err(|e| CommandError::IoError(e.to_string()))?;
    }

    match format {
        ArchiveFormat::Zip => {
            let file = File::open(path).map_err(|e| CommandError::IoError(e.to_string()))?;
            let mut archive = ZipArchive::new(file).map_err(|e| CommandError::ArchiveError(e.to_string()))?;
            // zip-rs doesn't have an easy way to check cancellation mid-extract without custom implementation
            // so we'll just check at the start.
            archive.extract(target).map_err(|e| CommandError::ArchiveError(e.to_string()))?;
        }
        ArchiveFormat::SevenZip => {
            sevenz::decompress_file(path, target).map_err(|e| CommandError::ArchiveError(e.to_string()))?;
        }
        ArchiveFormat::Tar | ArchiveFormat::TarGz | ArchiveFormat::TarXz | ArchiveFormat::TarZst | ArchiveFormat::TarBz2 => {
            let file = File::open(path).map_err(|e| CommandError::IoError(e.to_string()))?;
            let reader: Box<dyn io::Read> = match format {
                ArchiveFormat::TarGz => Box::new(GzDecoder::new(file)),
                ArchiveFormat::TarXz => Box::new(XzDecoder::new(file)),
                ArchiveFormat::TarBz2 => Box::new(BzDecoder::new(file)),
                ArchiveFormat::TarZst => Box::new(ZstdDecoder::new(file).map_err(|e| CommandError::IoError(e.to_string()))?),
                _ => Box::new(file),
            };
            let mut archive = TarArchive::new(reader);
            archive.unpack(target).map_err(|e| CommandError::ArchiveError(e.to_string()))?;
        }
        ArchiveFormat::Iso => {
            extract_iso(&archive_path, &target_dir)?;
        }
        ArchiveFormat::Rar => {
            // Rar extraction is not natively supported by our current crates.
            // We could use a library or call 7z.exe if available.
            return Err(CommandError::ArchiveError("Rar extraction requires external tools (like 7-Zip or WinRAR).".to_string()));
        }
    }
    Ok(())
}

fn extract_iso(archive_path: &str, target_dir: &str) -> Result<(), CommandError> {
    let file = File::open(archive_path).map_err(|e| CommandError::IoError(e.to_string()))?;
    let mut iso = iso9660_core::ISO9660::load(file).map_err(|e| CommandError::ArchiveError(format!("{:?}", e)))?;
    
    extract_iso_recursive(&mut iso, "/", target_dir)
}

fn extract_iso_recursive<T: iso9660_core::block_device::ISORead>(
    iso: &mut iso9660_core::ISO9660<T>,
    internal_path: &str,
    target_base: &str
) -> Result<(), CommandError> {
    let mut iter = iso.listdir(internal_path).map_err(|e| CommandError::ArchiveError(format!("{:?}", e)))?;
    
    let mut records = Vec::new();
    while let Some(record) = iter.next(iso) {
        records.push(record);
    }
    
    for rec in records {
        let name = match &rec {
            ISO9660Record::Directory(d) => d.identifier(),
            ISO9660Record::File(f) => f.identifier(),
        };

        if name == "." || name == ".." { continue; }
        
        let display_name = name.split(';').next().unwrap_or(&name);
        let new_internal = if internal_path == "/" {
            format!("/{}", display_name)
        } else {
            format!("{}/{}", internal_path.trim_end_matches('/'), display_name)
        };
        
        let relative_path = new_internal.trim_start_matches('/').replace('/', "\\");
        let target_path = Path::new(target_base).join(&relative_path);
        
        match rec {
            ISO9660Record::Directory(_) => {
                fs::create_dir_all(&target_path).map_err(|e| CommandError::IoError(e.to_string()))?;
                extract_iso_recursive(iso, &new_internal, target_base)?;
            }
            ISO9660Record::File(_) => {
                if let Some(parent) = target_path.parent() {
                    fs::create_dir_all(parent).map_err(|e| CommandError::IoError(e.to_string()))?;
                }
                
                let size = iso.total_size(&new_internal).map_err(|e| CommandError::ArchiveError(format!("{:?}", e)))?;
                let mut writer = File::create(target_path).map_err(|e| CommandError::IoError(e.to_string()))?;
                
                let mut offset = 0;
                let mut buf = [0u8; 65536];
                while offset < size {
                    let to_read = std::cmp::min(buf.len(), size - offset);
                    let n = iso.read(&new_internal, &mut buf[..to_read], offset).map_err(|e| CommandError::ArchiveError(format!("{:?}", e)))?;
                    if n == 0 { break; }
                    use std::io::Write;
                    writer.write_all(&buf[..n]).map_err(|e| CommandError::IoError(e.to_string()))?;
                    offset += n;
                }
            }
        }
    }
    Ok(())
}

#[command]
pub async fn cancel_archive_operation(state: State<'_, ArchiveState>) -> Result<(), CommandError> {
    state.0.store(true, Ordering::Relaxed);
    Ok(())
}

#[command]
pub async fn compress_to_archive(paths: Vec<String>, archive_path: String, format: String, quality: String, state: State<'_, ArchiveState>) -> Result<(), CommandError> {
    state.0.store(false, Ordering::Relaxed);
    let target_path_buf = validate_path(&archive_path)?;
    let target_path = target_path_buf.as_path();
    
    info!("Compressing {:?} items to {:?}", paths.len(), target_path);
    
    let result = match format.to_lowercase().as_str() {
        "zip" => compress_zip(paths, target_path, &quality, &state),
        "7z" => compress_seven_zip(paths, target_path, &quality, &state),
        "tar" => compress_tar(paths, target_path, false, &state),
        "zst" | "tar.zst" => compress_tar_zst(paths, target_path, &quality, &state),
        _ => Err(CommandError::ArchiveError("Unsupported format".to_string())),
    };

    if result.is_err()
        && target_path.exists() {
            let _ = fs::remove_file(target_path);
        }

    result
}

#[command]
pub async fn add_to_archive(paths: Vec<String>, archive_path: String, state: State<'_, ArchiveState>) -> Result<(), CommandError> {
    state.0.store(false, Ordering::Relaxed);
    let target_path_buf = validate_path(&archive_path)?;
    let target_path = target_path_buf.as_path();
    
    let format = ArchiveFormat::from_path(target_path).ok_or(CommandError::ArchiveError("Unsupported archive format".to_string()))?;
    
    match format {
        ArchiveFormat::Zip => add_to_zip(paths, target_path, &state),
        _ => Err(CommandError::ArchiveError("Adding to this archive format is not supported yet.".to_string())),
    }
}

fn add_to_zip(paths: Vec<String>, target: &Path, state: &State<'_, ArchiveState>) -> Result<(), CommandError> {
    let file = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(target)
        .map_err(|e| CommandError::IoError(e.to_string()))?;

    let mut zip = zip::ZipWriter::new_append(file).map_err(|e| CommandError::ArchiveError(e.to_string()))?;
    
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .unix_permissions(0o755);

    for p in paths {
        if state.0.load(Ordering::Relaxed) { return Err(CommandError::Other("Cancelled".into())); }
        let path = Path::new(&p);
        let parent = path.parent().unwrap_or(path);
        
        if path.is_dir() {
            for entry in walkdir::WalkDir::new(path) {
                if state.0.load(Ordering::Relaxed) { return Err(CommandError::Other("Cancelled".into())); }
                let entry = entry.map_err(|e| CommandError::IoError(e.to_string()))?;
                let entry_path = entry.path();
                let name = entry_path.strip_prefix(parent).map_err(|e| CommandError::PathError(e.to_string()))?;
                let name_str = name.to_string_lossy().replace('\\', "/");
                
                if entry.file_type().is_dir() {
                    zip.add_directory(name_str, options).map_err(|e| CommandError::ArchiveError(e.to_string()))?;
                } else {
                    zip.start_file(name_str, options).map_err(|e| CommandError::ArchiveError(e.to_string()))?;
                    let mut f = File::open(entry_path).map_err(|e| CommandError::IoError(e.to_string()))?;
                    io::copy(&mut f, &mut zip).map_err(|e| CommandError::IoError(e.to_string()))?;
                }
            }
        } else {
            let name = path.file_name().ok_or(CommandError::PathError("Invalid path".to_string()))?;
            zip.start_file(name.to_string_lossy().replace('\\', "/"), options).map_err(|e| CommandError::ArchiveError(e.to_string()))?;
            let mut f = File::open(path).map_err(|e| CommandError::IoError(e.to_string()))?;
            io::copy(&mut f, &mut zip).map_err(|e| CommandError::IoError(e.to_string()))?;
        }
    }

    zip.finish().map_err(|e| CommandError::ArchiveError(e.to_string()))?;
    Ok(())
}

fn compress_zip(paths: Vec<String>, target: &Path, quality: &str, state: &State<'_, ArchiveState>) -> Result<(), CommandError> {
    let file = File::create(target).map_err(|e| CommandError::IoError(e.to_string()))?;
    let mut zip = zip::ZipWriter::new(io::BufWriter::with_capacity(128 * 1024, file));
    
    let method = match quality {
        "fast" => zip::CompressionMethod::Deflated,
        "best" => zip::CompressionMethod::Deflated,
        _ => zip::CompressionMethod::Deflated,
    };
    
    let level = match quality {
        "fast" => Some(1),
        "best" => Some(9),
        _ => Some(6),
    };

    let options = zip::write::SimpleFileOptions::default()
        .compression_method(method)
        .compression_level(level)
        .unix_permissions(0o755);

    for p in paths {
        if state.0.load(Ordering::Relaxed) { return Err(CommandError::Other("Cancelled".into())); }
        let path = Path::new(&p);
        let parent = path.parent().unwrap_or(path);
        
        if path.is_dir() {
            for entry in walkdir::WalkDir::new(path) {
                if state.0.load(Ordering::Relaxed) { return Err(CommandError::Other("Cancelled".into())); }
                let entry = entry.map_err(|e| CommandError::IoError(e.to_string()))?;
                let entry_path = entry.path();
                let name = entry_path.strip_prefix(parent).map_err(|e| CommandError::PathError(e.to_string()))?;
                let name_str = name.to_string_lossy().replace('\\', "/");
                
                if entry.file_type().is_dir() {
                    zip.add_directory(name_str, options).map_err(|e| CommandError::ArchiveError(e.to_string()))?;
                } else {
                    zip.start_file(name_str, options).map_err(|e| CommandError::ArchiveError(e.to_string()))?;
                    let mut f = File::open(entry_path).map_err(|e| CommandError::IoError(e.to_string()))?;
                    io::copy(&mut f, &mut zip).map_err(|e| CommandError::IoError(e.to_string()))?;
                }
            }
        } else {
            let name = path.file_name().ok_or(CommandError::PathError("Invalid path".to_string()))?;
            zip.start_file(name.to_string_lossy().replace('\\', "/"), options).map_err(|e| CommandError::ArchiveError(e.to_string()))?;
            let mut f = File::open(path).map_err(|e| CommandError::IoError(e.to_string()))?;
            io::copy(&mut f, &mut zip).map_err(|e| CommandError::IoError(e.to_string()))?;
        }
    }

    zip.finish().map_err(|e| CommandError::ArchiveError(e.to_string()))?;
    Ok(())
}

fn compress_seven_zip(paths: Vec<String>, target: &Path, _quality: &str, state: &State<'_, ArchiveState>) -> Result<(), CommandError> {
    if paths.is_empty() { return Ok(()); }
    
    let file = File::create(target).map_err(|e| CommandError::IoError(e.to_string()))?;
    let mut writer = sevenz::SevenZWriter::new(file).map_err(|e| CommandError::ArchiveError(e.to_string()))?;
    
    for p in paths {
        if state.0.load(Ordering::Relaxed) { return Err(CommandError::Other("Cancelled".into())); }
        let path = Path::new(&p);
        if path.is_dir() {
            add_dir_to_sevenz(&mut writer, path, path.parent().unwrap_or(path), state)?;
        } else {
            add_file_to_sevenz(&mut writer, path, path.parent().unwrap_or(path))?;
        }
    }
    writer.finish().map_err(|e| CommandError::ArchiveError(e.to_string()))?;
    Ok(())
}

fn add_file_to_sevenz<W: io::Write + io::Seek>(
    writer: &mut sevenz::SevenZWriter<W>,
    path: &Path,
    base: &Path
) -> Result<(), CommandError> {
    let f = File::open(path).map_err(|e| CommandError::IoError(e.to_string()))?;
    let name = path.strip_prefix(base).map_err(|e| CommandError::PathError(e.to_string()))?.to_string_lossy();
    writer.push_archive_entry(
        sevenz::SevenZArchiveEntry::from_path(path, name.into()),
        Some(&mut io::BufReader::with_capacity(128 * 1024, f))
    ).map_err(|e| CommandError::ArchiveError(e.to_string()))?;
    Ok(())
}

fn add_dir_to_sevenz<W: io::Write + io::Seek>(
    writer: &mut sevenz::SevenZWriter<W>,
    path: &Path,
    base: &Path,
    state: &State<'_, ArchiveState>
) -> Result<(), CommandError> {
    let name = path.strip_prefix(base).map_err(|e| CommandError::PathError(e.to_string()))?.to_string_lossy();
    writer.push_archive_entry(
        sevenz::SevenZArchiveEntry::from_path(path, name.into()),
        None::<&mut File>
    ).map_err(|e| CommandError::ArchiveError(e.to_string()))?;
    
    for entry in fs::read_dir(path).map_err(|e| CommandError::IoError(e.to_string()))? {
        if state.0.load(Ordering::Relaxed) { return Err(CommandError::Other("Cancelled".into())); }
        let entry = entry.map_err(|e| CommandError::IoError(e.to_string()))?;
        let p = entry.path();
        if p.is_dir() {
            add_dir_to_sevenz(writer, &p, base, state)?;
        } else {
            add_file_to_sevenz(writer, &p, base)?;
        }
    }
    Ok(())
}

fn compress_tar(paths: Vec<String>, target: &Path, _gz: bool, state: &State<'_, ArchiveState>) -> Result<(), CommandError> {
    let file = File::create(target).map_err(|e| CommandError::IoError(e.to_string()))?;
    let mut tar = tar::Builder::new(io::BufWriter::with_capacity(128 * 1024, file));

    for p in paths {
        if state.0.load(Ordering::Relaxed) { return Err(CommandError::Other("Cancelled".into())); }
        let path = Path::new(&p);
        let name = path.file_name().ok_or(CommandError::PathError("Invalid path".to_string()))?;
        if path.is_dir() {
            tar.append_dir_all(name, path).map_err(|e| CommandError::ArchiveError(e.to_string()))?;
        } else {
            let f = File::open(path).map_err(|e| CommandError::IoError(e.to_string()))?;
            let mut header = tar::Header::new_gnu();
            header.set_path(name).map_err(|e| CommandError::ArchiveError(e.to_string()))?;
            header.set_size(f.metadata().map_err(|e| CommandError::IoError(e.to_string()))?.len());
            header.set_cksum();
            tar.append(&header, &mut io::BufReader::with_capacity(128 * 1024, f)).map_err(|e| CommandError::ArchiveError(e.to_string()))?;
        }
    }
    tar.finish().map_err(|e| CommandError::ArchiveError(e.to_string()))?;
    Ok(())
}

fn compress_tar_zst(paths: Vec<String>, target: &Path, quality: &str, state: &State<'_, ArchiveState>) -> Result<(), CommandError> {
    let file = File::create(target).map_err(|e| CommandError::IoError(e.to_string()))?;
    let level = match quality {
        "fast" => 1,
        "best" => 19,
        _ => 3,
    };
    let zstd = ZstdEncoder::new(file, level).map_err(|e| CommandError::IoError(e.to_string()))?.auto_finish();
    let mut tar = tar::Builder::new(zstd);

    for p in paths {
        if state.0.load(Ordering::Relaxed) { return Err(CommandError::Other("Cancelled".into())); }
        let path = Path::new(&p);
        let name = path.file_name().ok_or(CommandError::PathError("Invalid path".to_string()))?;
        if path.is_dir() {
            tar.append_dir_all(name, path).map_err(|e| CommandError::ArchiveError(e.to_string()))?;
        } else {
            let f = File::open(path).map_err(|e| CommandError::IoError(e.to_string()))?;
            let mut header = tar::Header::new_gnu();
            header.set_path(name).map_err(|e| CommandError::ArchiveError(e.to_string()))?;
            header.set_size(f.metadata().map_err(|e| CommandError::IoError(e.to_string()))?.len());
            header.set_cksum();
            tar.append(&header, &mut io::BufReader::with_capacity(128 * 1024, f)).map_err(|e| CommandError::ArchiveError(e.to_string()))?;
        }
    }
    tar.finish().map_err(|e| CommandError::ArchiveError(e.to_string()))?;
    Ok(())
}
