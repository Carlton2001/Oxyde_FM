use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;
use crate::models::{FileEntry, CommandError};
use crate::utils::path_security::validate_path;
pub use crate::utils::archive::{ArchiveFormat, is_archive, split_virtual_path};
use crate::systems::file_ops::{FileOperation, FileOpType, FileOperationManager, OpStatus};
use log::info;
use tauri::{command, AppHandle, Emitter, State};
use zip::ZipArchive;
use sevenz_rust as sevenz;
use tar::Archive as TarArchive;
use flate2::read::GzDecoder;
use xz2::read::XzDecoder;
use bzip2::read::BzDecoder;
use zstd::stream::read::Decoder as ZstdDecoder;
use zstd::stream::write::Encoder as ZstdEncoder;
use iso9660_core::iso9660entry::{IsISO9660Record, ISO9660Record};
use std::process::Command;
use std::os::windows::process::CommandExt;

// ---------------------------------------------------------------------------
// Progress helpers
// ---------------------------------------------------------------------------

fn emit_op_progress(
    app: &AppHandle,
    op_arc: &Arc<Mutex<FileOperation>>,
    processed_bytes: u64,
    processed_files: usize,
    bps: u64,
) {
    let mut locked = op_arc.lock().unwrap();
    locked.processed_bytes = processed_bytes;
    locked.processed_files = processed_files;
    locked.bytes_per_second = bps;
    let data = locked.clone();
    drop(locked);
    let _ = app.emit("file_op_event", data);
}

fn emit_op_status(app: &AppHandle, op_arc: &Arc<Mutex<FileOperation>>, status: OpStatus) {
    let mut locked = op_arc.lock().unwrap();
    locked.status = status;
    let data = locked.clone();
    drop(locked);
    let _ = app.emit("file_op_event", data);
}

/// Walk `sources` and sum uncompressed byte counts and file counts.
fn count_sources(sources: &[PathBuf]) -> (u64, usize) {
    let mut total_bytes: u64 = 0;
    let mut total_files: usize = 0;
    for p in sources {
        for e in walkdir::WalkDir::new(p).into_iter().flatten() {
            if e.file_type().is_file() {
                total_bytes += e.metadata().map(|m| m.len()).unwrap_or(0);
                total_files += 1;
            }
        }
    }
    (total_bytes, total_files)
}

// ---------------------------------------------------------------------------
// External Engine Helpers
// ---------------------------------------------------------------------------

fn find_7z_binary() -> Option<PathBuf> {
    let common_paths = [
        "C:\\Program Files\\NanaZip\\NanaZipC.exe",
        "C:\\Program Files\\7-Zip\\7z.exe",
        "C:\\Program Files (x86)\\7-Zip\\7z.exe",
    ];
    for p in &common_paths {
        let path = PathBuf::from(p);
        if path.exists() {
            return Some(path);
        }
    }
    
    // Check if in PATH
    if Command::new("7z.exe").arg("-h").creation_flags(0x08000000).status().is_ok() {
        return Some(PathBuf::from("7z.exe"));
    }
    if Command::new("7z").arg("-h").creation_flags(0x08000000).status().is_ok() {
        return Some(PathBuf::from("7z"));
    }
    if Command::new("NanaZipC.exe").arg("-h").creation_flags(0x08000000).status().is_ok() {
        return Some(PathBuf::from("NanaZipC.exe"));
    }
    
    None
}

/// Helper to monitor a file size during compression to provide progress
fn monitor_external_progress(
    target: &Path,
    total_uncompressed_bytes: u64,
    total_files: usize,
    app: &AppHandle,
    op_arc: &Arc<Mutex<FileOperation>>,
    cancel_flag: &Arc<AtomicBool>,
    child_id: u32,
) {
    let start = Instant::now();
    let mut last_emit = Instant::now();
    
    // For progress estimation, we assume a 50% average compression ratio for the progress bar
    // to keep it moving even if the output is smaller.
    let estimated_compressed_total = (total_uncompressed_bytes as f64 * 0.5) as u64;

    while !cancel_flag.load(Ordering::Relaxed) {
        if let Ok(meta) = fs::metadata(target) {
            let current_size = meta.len();
            if last_emit.elapsed().as_millis() > 200 {
                let bps = if start.elapsed().as_secs_f64() > 0.05 {
                    (current_size as f64 / start.elapsed().as_secs_f64()) as u64
                } else { 0 };
                
                // We cap progress at 99% during compression because we don't know the exact final size
                let progress_bytes = std::cmp::min(current_size, estimated_compressed_total.saturating_sub(100));
                emit_op_progress(app, op_arc, progress_bytes, total_files, bps);
                last_emit = Instant::now();
            }
        }
        
        // Check if process still alive
        if !is_process_alive(child_id) {
            break;
        }
        
        std::thread::sleep(std::time::Duration::from_millis(200));
    }
}

fn is_process_alive(pid: u32) -> bool {
    // Basic Windows check
    let mut cmd = Command::new("tasklist");
    cmd.arg("/FI").arg(format!("PID eq {}", pid));
    cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
    if let Ok(output) = cmd.output() {
        return String::from_utf8_lossy(&output.stdout).contains(&pid.to_string());
    }
    true
}

// ---------------------------------------------------------------------------
// Public Tauri commands
// ---------------------------------------------------------------------------

#[command]
pub async fn get_supported_archive_formats() -> Vec<String> {
    let mut formats = vec!["zip".to_string(), "tar".to_string(), "zst".to_string()];
    if find_7z_binary().is_some() {
        formats.push("7z".to_string());
    }
    formats
}

#[command]
pub async fn compress_to_archive(
    paths: Vec<String>,
    archive_path: String,
    format: String,
    _quality: String, // Ignored now but kept for API compatibility
    app: AppHandle,
    manager: State<'_, FileOperationManager>,
) -> Result<String, CommandError> {
    let target_path_buf = validate_path(&archive_path)?;
    let sources: Vec<PathBuf> = paths.iter().filter_map(|p| validate_path(p).ok()).collect();

    let (total_bytes, total_files) = count_sources(&sources);

    let mut op = FileOperation::new(FileOpType::Archive, sources, Some(target_path_buf.clone()), None);
    op.total_bytes = total_bytes;
    op.total_files = total_files;

    let op_id = op.id.clone();
    let op_arc = manager.register_op(op);

    let _ = app.emit("file_op_event", op_arc.lock().unwrap().clone());

    info!("Compressing {} items to {:?} (op_id={})", paths.len(), target_path_buf, op_id);

    let app_clone = app.clone();
    let target_clone = target_path_buf.clone();

    tauri::async_runtime::spawn_blocking(move || {
        let cancel_flag = op_arc.lock().unwrap().cancel_flag.clone();

        if cancel_flag.load(Ordering::Relaxed) {
            emit_op_status(&app_clone, &op_arc, OpStatus::Cancelled);
            return;
        }

        emit_op_status(&app_clone, &op_arc, OpStatus::Running);
        let start_time = Instant::now();

        let result = (|| -> Result<(), CommandError> {
            match format.to_lowercase().as_str() {
                "zip" | "tar" => {
                    let mut cmd = Command::new("tar.exe");
                    cmd.arg("-a").arg("-c").arg("-f").arg(&target_clone);
                    
                    // Set current_dir to the parent of the first item to ensure relative paths in the archive
                    if let Some(first_path) = paths.first() {
                        let p = Path::new(first_path);
                        if let Some(parent) = p.parent() {
                            cmd.current_dir(parent);
                            for path in &paths {
                                let pp = Path::new(path);
                                if let Ok(rel) = pp.strip_prefix(parent) {
                                    cmd.arg(rel);
                                } else {
                                    cmd.arg(path);
                                }
                            }
                        } else {
                            for p in &paths { cmd.arg(p); }
                        }
                    } else {
                        for p in &paths { cmd.arg(p); }
                    }
                    
                    cmd.creation_flags(0x08000000); 
                    
                    let mut child = cmd.spawn().map_err(|e| CommandError::IoError(format!("Failed to run tar.exe: {}", e)))?;
                    let child_id = child.id();
                    
                    monitor_external_progress(&target_clone, total_bytes, total_files, &app_clone, &op_arc, &cancel_flag, child_id);
                    
                    if cancel_flag.load(Ordering::Relaxed) {
                        let _ = child.kill();
                        return Err(CommandError::Other("Cancelled".into()));
                    }
                    let status = child.wait().map_err(|e| CommandError::IoError(e.to_string()))?;
                    if !status.success() {
                        return Err(CommandError::ArchiveError(format!("tar.exe exited with status {:?}", status)));
                    }
                    Ok(())
                },
                "7z" => {
                    if let Some(engine) = find_7z_binary() {
                        let mut cmd = Command::new(engine);
                        cmd.arg("a").arg("-mmt=on").arg(&target_clone);
                        
                        // Set current_dir to the parent of the first item to ensure relative paths in the archive
                        if let Some(first_path) = paths.first() {
                            let p = Path::new(first_path);
                            if let Some(parent) = p.parent() {
                                cmd.current_dir(parent);
                                for path in &paths {
                                    let pp = Path::new(path);
                                    if let Ok(rel) = pp.strip_prefix(parent) {
                                        cmd.arg(rel);
                                    } else {
                                        cmd.arg(path);
                                    }
                                }
                            } else {
                                for p in &paths { cmd.arg(p); }
                            }
                        } else {
                            for p in &paths { cmd.arg(p); }
                        }

                        cmd.creation_flags(0x08000000); 
                        
                        let mut child = cmd.spawn().map_err(|e| CommandError::IoError(format!("Failed to run 7z engine: {}", e)))?;
                        let child_id = child.id();
                        
                        monitor_external_progress(&target_clone, total_bytes, total_files, &app_clone, &op_arc, &cancel_flag, child_id);
                        
                        if cancel_flag.load(Ordering::Relaxed) {
                            let _ = child.kill();
                            return Err(CommandError::Other("Cancelled".into()));
                        }
                        let status = child.wait().map_err(|e| CommandError::IoError(e.to_string()))?;
                        if !status.success() {
                            return Err(CommandError::ArchiveError(format!("7z exited with status {:?}", status)));
                        }
                        Ok(())
                    } else {
                        Err(CommandError::ArchiveError("NanaZip or 7-Zip not found".to_string()))
                    }
                },
                "zst" | "tar.zst" => {
                    compress_tar_zst(paths, &target_clone, "normal", &cancel_flag, &app_clone, &op_arc)
                },
                _ => Err(CommandError::ArchiveError("Unsupported format".to_string())),
            }
        })();

        let was_cancelled = cancel_flag.load(Ordering::Relaxed);
        if result.is_err() || was_cancelled {
            let _ = fs::remove_file(&target_clone);
        }

        if was_cancelled {
            emit_op_status(&app_clone, &op_arc, OpStatus::Cancelled);
        } else {
            match result {
                Ok(_) => {
                    let total_secs = start_time.elapsed().as_secs_f64();
                    let final_bps = if total_secs > 0.0 { (total_bytes as f64 / total_secs) as u64 } else { 0 };
                    emit_op_progress(&app_clone, &op_arc, total_bytes, total_files, final_bps);
                    emit_op_status(&app_clone, &op_arc, OpStatus::Completed);
                },
                Err(e) => emit_op_status(&app_clone, &op_arc, OpStatus::Error(e.to_string())),
            }
        }
    });

    Ok(op_id)
}

#[allow(clippy::too_many_arguments)]
#[command]
pub async fn extract_archive(
    archive_path: String,
    target_dir: String,
    app: AppHandle,
    manager: State<'_, FileOperationManager>,
) -> Result<String, CommandError> {
    let path_buf = validate_path(&archive_path)?;
    let target_buf = validate_path(&target_dir)?;
    let format = ArchiveFormat::from_path(&path_buf)
        .ok_or(CommandError::ArchiveError("Unsupported archive format".to_string()))?;

    let sources = vec![path_buf.clone()];
    let (total_bytes, total_files) = match format {
        ArchiveFormat::Zip => {
            // Pre-count from zip metadata
            let f = File::open(&path_buf).map_err(|e| CommandError::IoError(e.to_string()))?;
            let mut archive = ZipArchive::new(f).map_err(|e| CommandError::ArchiveError(e.to_string()))?;
            let count = archive.len();
            let mut bytes: u64 = 0;
            for i in 0..count {
                if let Ok(entry) = archive.by_index_raw(i) {
                    bytes += entry.size();
                }
            }
            (bytes, count)
        }
        ArchiveFormat::SevenZip => {
            let f = File::open(&path_buf).map_err(|e| CommandError::IoError(e.to_string()))?;
            let len = f.metadata().map_err(|e| CommandError::IoError(e.to_string()))?.len();
            let mut reader = sevenz::SevenZReader::new(f, len, "".into())
                .map_err(|e| CommandError::ArchiveError(e.to_string()))?;
            let mut bytes: u64 = 0;
            let mut count: usize = 0;
            reader.for_each_entries(|entry, _| {
                if !entry.is_directory() {
                    bytes += entry.size();
                    count += 1;
                }
                Ok(true)
            }).ok();
            (bytes, count)
        }
        _ => (0, 0), // TAR/ISO: unknown without full scan; progress by file count
    };

    let mut op = FileOperation::new(FileOpType::Extract, sources, Some(target_buf.clone()), None);
    op.total_bytes = total_bytes;
    op.total_files = total_files;

    let op_id = op.id.clone();
    let op_arc = manager.register_op(op);

    let _ = app.emit("file_op_event", op_arc.lock().unwrap().clone());

    info!("Extracting {:?} to {:?} (op_id={})", path_buf, target_buf, op_id);

    let app_clone = app.clone();

    tauri::async_runtime::spawn_blocking(move || {
        let cancel_flag = op_arc.lock().unwrap().cancel_flag.clone();

        if cancel_flag.load(Ordering::Relaxed) {
            emit_op_status(&app_clone, &op_arc, OpStatus::Cancelled);
            return;
        }

        emit_op_status(&app_clone, &op_arc, OpStatus::Running);

        if !target_buf.exists() {
            if let Err(e) = fs::create_dir_all(&target_buf) {
                emit_op_status(&app_clone, &op_arc, OpStatus::Error(e.to_string()));
                return;
            }
        }

        let result = match format {
            ArchiveFormat::Zip => extract_zip_with_progress(&path_buf, &target_buf, &cancel_flag, &app_clone, &op_arc),
            ArchiveFormat::SevenZip => {
                sevenz::decompress_file(&path_buf, &target_buf)
                    .map_err(|e| CommandError::ArchiveError(e.to_string()))
            }
            ArchiveFormat::Tar | ArchiveFormat::TarGz | ArchiveFormat::TarXz | ArchiveFormat::TarZst | ArchiveFormat::TarBz2 => {
                extract_tar_with_progress(&path_buf, &target_buf, format, &cancel_flag, &app_clone, &op_arc)
            }
            ArchiveFormat::Iso => extract_iso(
                &path_buf.to_string_lossy(),
                &target_buf.to_string_lossy(),
            ),
            ArchiveFormat::Rar => Err(CommandError::ArchiveError(
                "Rar extraction requires external tools (like 7-Zip or WinRAR).".to_string(),
            )),
        };

        let was_cancelled = cancel_flag.load(Ordering::Relaxed);
        if was_cancelled {
            emit_op_status(&app_clone, &op_arc, OpStatus::Cancelled);
        } else {
            match result {
                Ok(_) => emit_op_status(&app_clone, &op_arc, OpStatus::Completed),
                Err(e) => emit_op_status(&app_clone, &op_arc, OpStatus::Error(e.to_string())),
            }
        }
    });

    Ok(op_id)
}

#[command]
pub async fn add_to_archive(
    paths: Vec<String>,
    archive_path: String,
    app: AppHandle,
    manager: State<'_, FileOperationManager>,
) -> Result<String, CommandError> {
    let target_path_buf = validate_path(&archive_path)?;
    let format = ArchiveFormat::from_path(&target_path_buf)
        .ok_or(CommandError::ArchiveError("Unsupported archive format".to_string()))?;

    if !matches!(format, ArchiveFormat::Zip) {
        return Err(CommandError::ArchiveError("Adding to this archive format is not supported yet.".to_string()));
    }

    let sources: Vec<PathBuf> = paths.iter().filter_map(|p| validate_path(p).ok()).collect();
    let (total_bytes, total_files) = count_sources(&sources);

    let mut op = FileOperation::new(FileOpType::Archive, sources, Some(target_path_buf.clone()), None);
    op.total_bytes = total_bytes;
    op.total_files = total_files;

    let op_id = op.id.clone();
    let op_arc = manager.register_op(op);

    let _ = app.emit("file_op_event", op_arc.lock().unwrap().clone());

    let app_clone = app.clone();
    let target_clone = target_path_buf.clone();

    tauri::async_runtime::spawn_blocking(move || {
        let cancel_flag = op_arc.lock().unwrap().cancel_flag.clone();

        if cancel_flag.load(Ordering::Relaxed) {
            emit_op_status(&app_clone, &op_arc, OpStatus::Cancelled);
            return;
        }

        emit_op_status(&app_clone, &op_arc, OpStatus::Running);

        let result = add_to_zip(paths, &target_clone, &cancel_flag, &app_clone, &op_arc);

        let was_cancelled = cancel_flag.load(Ordering::Relaxed);
        if was_cancelled {
            emit_op_status(&app_clone, &op_arc, OpStatus::Cancelled);
        } else {
            match result {
                Ok(_) => emit_op_status(&app_clone, &op_arc, OpStatus::Completed),
                Err(e) => emit_op_status(&app_clone, &op_arc, OpStatus::Error(e.to_string())),
            }
        }
    });

    Ok(op_id)
}

// ---------------------------------------------------------------------------
// Extraction helpers
// ---------------------------------------------------------------------------

fn extract_zip_with_progress(
    path: &Path,
    target: &Path,
    cancel_flag: &Arc<AtomicBool>,
    app: &AppHandle,
    op_arc: &Arc<Mutex<FileOperation>>,
) -> Result<(), CommandError> {
    let file = File::open(path).map_err(|e| CommandError::IoError(e.to_string()))?;
    let mut archive = ZipArchive::new(file).map_err(|e| CommandError::ArchiveError(e.to_string()))?;
    let total = archive.len();

    let mut processed_bytes: u64 = 0;
    let mut processed_files: usize = 0;
    let start = Instant::now();

    for i in 0..total {
        if cancel_flag.load(Ordering::Relaxed) {
            return Err(CommandError::Other("Cancelled".into()));
        }

        let mut entry = archive.by_index(i).map_err(|e| CommandError::ArchiveError(e.to_string()))?;
        let entry_size = entry.size();

        let outpath = match entry.enclosed_name() {
            Some(name) => target.join(name),
            None => continue,
        };

        if entry.is_dir() {
            fs::create_dir_all(&outpath).map_err(|e| CommandError::IoError(e.to_string()))?;
        } else {
            if let Some(parent) = outpath.parent() {
                fs::create_dir_all(parent).map_err(|e| CommandError::IoError(e.to_string()))?;
            }
            let mut out = File::create(&outpath).map_err(|e| CommandError::IoError(e.to_string()))?;
            io::copy(&mut entry, &mut out).map_err(|e| CommandError::IoError(e.to_string()))?;
        }

        processed_bytes += entry_size;
        processed_files += 1;
        let elapsed = start.elapsed().as_secs_f64();
        let bps = if elapsed > 0.0 { (processed_bytes as f64 / elapsed) as u64 } else { 0 };
        emit_op_progress(app, op_arc, processed_bytes, processed_files, bps);
    }

    Ok(())
}

fn extract_tar_with_progress(
    path: &Path,
    target: &Path,
    format: ArchiveFormat,
    cancel_flag: &Arc<AtomicBool>,
    app: &AppHandle,
    op_arc: &Arc<Mutex<FileOperation>>,
) -> Result<(), CommandError> {
    let file = File::open(path).map_err(|e| CommandError::IoError(e.to_string()))?;
    let reader: Box<dyn io::Read> = match format {
        ArchiveFormat::TarGz => Box::new(GzDecoder::new(file)),
        ArchiveFormat::TarXz => Box::new(XzDecoder::new(file)),
        ArchiveFormat::TarBz2 => Box::new(BzDecoder::new(file)),
        ArchiveFormat::TarZst => Box::new(ZstdDecoder::new(file).map_err(|e| CommandError::IoError(e.to_string()))?),
        _ => Box::new(file),
    };

    let mut archive = TarArchive::new(reader);
    let mut processed_bytes: u64 = 0;
    let mut processed_files: usize = 0;
    let start = Instant::now();

    for entry in archive.entries().map_err(|e| CommandError::ArchiveError(e.to_string()))? {
        if cancel_flag.load(Ordering::Relaxed) {
            return Err(CommandError::Other("Cancelled".into()));
        }

        let mut entry = entry.map_err(|e| CommandError::ArchiveError(e.to_string()))?;
        let entry_size = entry.header().size().unwrap_or(0);
        entry.unpack_in(target).map_err(|e| CommandError::IoError(e.to_string()))?;

        processed_bytes += entry_size;
        processed_files += 1;
        let elapsed = start.elapsed().as_secs_f64();
        let bps = if elapsed > 0.0 { (processed_bytes as f64 / elapsed) as u64 } else { 0 };
        emit_op_progress(app, op_arc, processed_bytes, processed_files, bps);
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
    target_base: &str,
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

        let display_name = name.split(';').next().unwrap_or(name);
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
                let mut writer = File::create(&target_path).map_err(|e| CommandError::IoError(e.to_string()))?;

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

// ---------------------------------------------------------------------------
// Compression helpers
// ---------------------------------------------------------------------------

fn compress_tar_zst(
    paths: Vec<String>,
    target: &Path,
    quality: &str,
    cancel_flag: &Arc<AtomicBool>,
    app: &AppHandle,
    op_arc: &Arc<Mutex<FileOperation>>,
) -> Result<(), CommandError> {
    let file = File::create(target).map_err(|e| CommandError::IoError(e.to_string()))?;
    let level = match quality { "fast" => 1, "best" => 19, _ => 3 };
    let zstd = ZstdEncoder::new(file, level).map_err(|e| CommandError::IoError(e.to_string()))?.auto_finish();
    let mut tar = tar::Builder::new(zstd);

    let mut processed_bytes: u64 = 0;
    let mut processed_files: usize = 0;
    let start = Instant::now();

    for p in paths {
        if cancel_flag.load(Ordering::Relaxed) { return Err(CommandError::Other("Cancelled".into())); }
        let path = Path::new(&p);
        let parent = path.parent().unwrap_or(path);

        if path.is_dir() {
            for entry in walkdir::WalkDir::new(path) {
                if cancel_flag.load(Ordering::Relaxed) { return Err(CommandError::Other("Cancelled".into())); }
                let entry = entry.map_err(|e| CommandError::IoError(e.to_string()))?;
                let ep = entry.path();
                let name = ep.strip_prefix(parent).map_err(|e| CommandError::PathError(e.to_string()))?;

                if entry.file_type().is_dir() {
                    tar.append_dir(name, ep).map_err(|e| CommandError::ArchiveError(e.to_string()))?;
                } else {
                    let file_size = ep.metadata().map(|m| m.len()).unwrap_or(0);
                    let f = File::open(ep).map_err(|e| CommandError::IoError(e.to_string()))?;
                    let mut header = tar::Header::new_gnu();
                    header.set_path(name).map_err(|e| CommandError::ArchiveError(e.to_string()))?;
                    header.set_size(file_size);
                    header.set_cksum();
                    tar.append(&header, &mut io::BufReader::with_capacity(128 * 1024, f))
                        .map_err(|e| CommandError::ArchiveError(e.to_string()))?;
                    processed_bytes += file_size;
                    processed_files += 1;
                    emit_op_progress(app, op_arc, processed_bytes, processed_files, bps_from_start(&start, processed_bytes));
                }
            }
        } else {
            let name = path.file_name().ok_or(CommandError::PathError("Invalid path".to_string()))?;
            let file_size = path.metadata().map(|m| m.len()).unwrap_or(0);
            let f = File::open(path).map_err(|e| CommandError::IoError(e.to_string()))?;
            let mut header = tar::Header::new_gnu();
            header.set_path(name).map_err(|e| CommandError::ArchiveError(e.to_string()))?;
            header.set_size(file_size);
            header.set_cksum();
            tar.append(&header, &mut io::BufReader::with_capacity(128 * 1024, f))
                .map_err(|e| CommandError::ArchiveError(e.to_string()))?;
            processed_bytes += file_size;
            processed_files += 1;
            emit_op_progress(app, op_arc, processed_bytes, processed_files, bps_from_start(&start, processed_bytes));
        }
    }
    tar.finish().map_err(|e| CommandError::ArchiveError(e.to_string()))?;
    Ok(())
}

fn add_to_zip(
    paths: Vec<String>,
    target: &Path,
    cancel_flag: &Arc<AtomicBool>,
    app: &AppHandle,
    op_arc: &Arc<Mutex<FileOperation>>,
) -> Result<(), CommandError> {
    let file = fs::OpenOptions::new()
        .read(true).write(true)
        .open(target)
        .map_err(|e| CommandError::IoError(e.to_string()))?;

    let mut zip = zip::ZipWriter::new_append(file).map_err(|e| CommandError::ArchiveError(e.to_string()))?;
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .unix_permissions(0o755);

    let mut processed_bytes: u64 = 0;
    let mut processed_files: usize = 0;
    let start = Instant::now();

    for p in paths {
        if cancel_flag.load(Ordering::Relaxed) { return Err(CommandError::Other("Cancelled".into())); }
        let path = Path::new(&p);
        let parent = path.parent().unwrap_or(path);

        if path.is_dir() {
            for entry in walkdir::WalkDir::new(path) {
                if cancel_flag.load(Ordering::Relaxed) { return Err(CommandError::Other("Cancelled".into())); }
                let entry = entry.map_err(|e| CommandError::IoError(e.to_string()))?;
                let ep = entry.path();
                let name_str = ep.strip_prefix(parent).map_err(|e| CommandError::PathError(e.to_string()))?.to_string_lossy().replace('\\', "/");

                if entry.file_type().is_dir() {
                    zip.add_directory(&name_str, options).map_err(|e| CommandError::ArchiveError(e.to_string()))?;
                } else {
                    let file_size = ep.metadata().map(|m| m.len()).unwrap_or(0);
                    zip.start_file(&name_str, options).map_err(|e| CommandError::ArchiveError(e.to_string()))?;
                    let mut f = File::open(ep).map_err(|e| CommandError::IoError(e.to_string()))?;
                    io::copy(&mut f, &mut zip).map_err(|e| CommandError::IoError(e.to_string()))?;
                    processed_bytes += file_size;
                    processed_files += 1;
                    emit_op_progress(app, op_arc, processed_bytes, processed_files, bps_from_start(&start, processed_bytes));
                }
            }
        } else {
            let name = path.file_name().ok_or(CommandError::PathError("Invalid path".to_string()))?;
            let file_size = path.metadata().map(|m| m.len()).unwrap_or(0);
            zip.start_file(name.to_string_lossy().replace('\\', "/"), options).map_err(|e| CommandError::ArchiveError(e.to_string()))?;
            let mut f = File::open(path).map_err(|e| CommandError::IoError(e.to_string()))?;
            io::copy(&mut f, &mut zip).map_err(|e| CommandError::IoError(e.to_string()))?;
            processed_bytes += file_size;
            processed_files += 1;
            emit_op_progress(app, op_arc, processed_bytes, processed_files, bps_from_start(&start, processed_bytes));
        }
    }

    zip.finish().map_err(|e| CommandError::ArchiveError(e.to_string()))?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Shared utility
// ---------------------------------------------------------------------------

fn bps_from_start(start: &Instant, processed_bytes: u64) -> u64 {
    let elapsed = start.elapsed().as_secs_f64();
    if elapsed > 0.0 { (processed_bytes as f64 / elapsed) as u64 } else { 0 }
}

// ---------------------------------------------------------------------------
// Archive modification (remove entries)
// ---------------------------------------------------------------------------

pub fn remove_items_from_archive(archive_path: PathBuf, internal_paths: Vec<String>) -> Result<(), CommandError> {
    let format = ArchiveFormat::from_path(&archive_path)
        .ok_or(CommandError::ArchiveError("Unsupported archive format".to_string()))?;

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
            let normalized_entry = entry_name.replace('\\', "/");
            if normalized_entry == normalized_p { return true; }
            let dir_prefix = if normalized_p.ends_with('/') { normalized_p.clone() } else { format!("{}/", normalized_p) };
            normalized_entry.starts_with(&dir_prefix)
        });

        if !should_remove {
            writer.raw_copy_file(entry).map_err(|e| CommandError::ArchiveError(e.to_string()))?;
        }
    }

    let _tmp = writer.finish().map_err(|e| CommandError::ArchiveError(e.to_string()))?;
    drop(archive);

    std::fs::remove_file(archive_path).map_err(|e| CommandError::IoError(e.to_string()))?;
    std::fs::rename(temp_file_path, archive_path).map_err(|e| CommandError::IoError(e.to_string()))?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Archive listing
// ---------------------------------------------------------------------------

#[command]
pub fn list_archive_contents(archive_path: String, internal_path: String) -> Result<Vec<FileEntry>, CommandError> {
    let path_buf = validate_path(&archive_path)?;
    let path = path_buf.as_path();
    let format = ArchiveFormat::from_path(path)
        .ok_or(CommandError::ArchiveError("Unsupported archive format".to_string()))?;

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

    loop {
        let record = entries_iter.next(&mut iso);
        let Some(rec) = record else { break; };

        let name = match &rec {
            ISO9660Record::Directory(d) => d.identifier(),
            ISO9660Record::File(f) => f.identifier(),
        };

        if name == "." || name == ".." { continue; }

        let display_name = name.split(';').next().unwrap_or(name).to_string();
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
            folders_count: None,
            files_count: None,
            modified: 0,
            is_readonly: true,
            is_protected: false,
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
                folders_count: None,
                files_count: None,
                modified: file.last_modified()
                    .and_then(|dt| { let t: Result<time::OffsetDateTime, _> = dt.try_into(); t.ok() })
                    .map(|ts| ts.unix_timestamp() as u64 * 1000)
                    .unwrap_or(0),
                is_readonly: false,
                is_protected: false,
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
                    let full_virtual_path = if internal_prefix.is_empty() {
                        format!("{}\\{}", path.to_string_lossy(), entry_name)
                    } else {
                        format!("{}\\{}\\{}", path.to_string_lossy(), internal_prefix.replace('/', "\\"), entry_name)
                    }.replace("\\\\", "\\");

                    entries.push(FileEntry {
                        name: entry_name.clone(),
                        path: full_virtual_path,
                        is_dir,
                        is_hidden: false,
                        is_system: false,
                        is_symlink: false,
                        is_junction: false,
                        size: entry.size(),
                        is_calculated: false,
                        folders_count: None,
                        files_count: None,
                        modified: 0,
                        is_readonly: false,
                        is_protected: false,
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

            let full_virtual_path = if internal_prefix.is_empty() {
                format!("{}\\{}", path.to_string_lossy(), entry_name)
            } else {
                format!("{}\\{}\\{}", path.to_string_lossy(), internal_prefix.replace('/', "\\"), entry_name)
            }.replace("\\\\", "\\");

            entries.push(FileEntry {
                name: entry_name,
                path: full_virtual_path,
                is_dir,
                is_hidden: false,
                is_system: false,
                is_symlink: false,
                is_junction: false,
                size: entry.header().size().unwrap_or(0),
                is_calculated: false,
                folders_count: None,
                files_count: None,
                modified: entry.header().mtime().unwrap_or(0) * 1000,
                is_readonly: false,
                is_protected: false,
                original_path: None,
                deleted_time: None,
            });
        }
    }
    Ok(entries)
}
