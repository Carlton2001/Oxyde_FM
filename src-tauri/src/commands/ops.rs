use crate::models::{get_file_entry_from_path, ConflictEntry, ConflictResponse, TrashEntry, CommandError, Transaction, TransactionType, TransactionDetails, HistoryManager, ProgressEvent};
use crate::utils::path_security::validate_path;
use std::path::PathBuf;
use tauri::{AppHandle, Emitter, State}; // Emitter needed for legacy progress emit
use std::sync::{Arc};
use std::sync::atomic::{AtomicBool, Ordering};
use log::{info, warn};

use crate::systems::file_ops::{FileOperation, FileOperationManager, FileOpType};

// Legacy FileOpState struct - keeping for now just in case, or removing if unused?
// If the whole file uses the new system, we can remove it.
// But cancel implementation was using it.
// Let's remove it if I replace all usages.
// pub struct FileOpState... removed


#[tauri::command]
pub fn cancel_file_operation(manager: State<'_, FileOperationManager>, id: String) -> Result<(), CommandError> {
    manager.cancel_operation(&id);
    Ok(())
}

#[tauri::command]
pub fn pause_file_operation(manager: State<'_, FileOperationManager>, id: String) -> Result<(), CommandError> {
    if let Some(op) = manager.get_operation(&id) {
        // We need a way to pause. The lock inside FileOperation needs to be toggled.
        // But FileOperationManager::get_operation returns a CLONE of FileOperation struct (snapshot).
        // Wait, the struct holds Arc<AtomicBool>, so cloning the struct CLONES THE ARC.
        // So modifying the atomic bool in the clone affects the running task. 
        op.pause_flag.store(true, Ordering::Relaxed);
        Ok(())
    } else {
        Err(CommandError::Other("Operation not found".to_string()))
    }
}

#[tauri::command]
pub fn resume_file_operation(manager: State<'_, FileOperationManager>, id: String) -> Result<(), CommandError> {
    if let Some(op) = manager.get_operation(&id) {
        op.pause_flag.store(false, Ordering::Relaxed);
        Ok(())
    } else {
        Err(CommandError::Other("Operation not found".to_string()))
    }
}

#[tauri::command]
pub fn toggle_turbo(app: AppHandle, manager: State<'_, FileOperationManager>, id: String, enabled: bool) -> Result<(), CommandError> {
    manager.set_turbo(&app, &id, enabled);
    Ok(())
}

#[tauri::command]
pub fn get_op_status(manager: State<'_, FileOperationManager>, id: String) -> Result<Option<FileOperation>, CommandError> {
    Ok(manager.get_operation(&id))
}

struct RestorationPaths {
    crate_path: PathBuf,
    corrected_path: PathBuf,
}

fn get_restoration_paths(item: &trash::TrashItem) -> RestorationPaths {
    let crate_path = item.original_path();
    let mut corrected_path = crate_path.clone();

    let trash_path = PathBuf::from(&item.id);
    if let Some(ext) = trash_path.extension() {
        let ext_str = ext.to_string_lossy().to_lowercase();
        if ext_str == "lnk" || ext_str == "url" {
            let name = corrected_path
                .file_name()
                .map(|n| n.to_string_lossy().to_lowercase())
                .unwrap_or_default();
            if !name.ends_with(&format!(".{}", ext_str)) {
                let mut new_name = corrected_path
                    .file_name()
                    .unwrap_or_default()
                    .to_os_string();
                new_name.push(".");
                new_name.push(ext);
                corrected_path.set_file_name(new_name);
            }
        }
    }

    RestorationPaths {
        crate_path,
        corrected_path,
    }
}

/// List all items in the Windows Recycle Bin
#[tauri::command]
pub fn list_trash() -> Result<Vec<TrashEntry>, CommandError> {
    let trash_items = trash::os_limited::list().map_err(|e| CommandError::TrashError(e.to_string()))?;

    let mut entries: Vec<TrashEntry> = trash_items
        .into_iter()
        .map(|item| {
            let original_path = item.original_path();
            let mut name = original_path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| original_path.to_string_lossy().to_string());

            // On Windows, Shell API sometimes returns names without .lnk even if original_path had it.
            // Check the real trash file extension to be sure.
            let trash_path = PathBuf::from(&item.id);
            if let Some(ext) = trash_path.extension() {
                let ext_str = ext.to_string_lossy().to_lowercase();
                if (ext_str == "lnk" || ext_str == "url")
                    && !name.to_lowercase().ends_with(&format!(".{}", ext_str))
                {
                    name.push_str(&format!(".{}", ext_str));
                }
            }
            let deleted_time = (item.time_deleted.max(0) as u64) * 1000;

            // Convert OsString id to PathBuf for metadata access
            let trash_path = PathBuf::from(&item.id);

            // Check if it's a directory
            let is_dir = std::fs::metadata(&trash_path)
                .map(|m| m.is_dir())
                .unwrap_or(false);

            let size = if is_dir {
                0
            } else {
                std::fs::metadata(&trash_path).map(|m| m.len()).unwrap_or(0)
            };

            // Get the file's modification time
            let modified = std::fs::metadata(&trash_path)
                .and_then(|m| m.modified())
                .map(|t| {
                    t.duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_millis() as u64
                })
                .unwrap_or(deleted_time);

            TrashEntry {
                name,
                // Use the internal trash path (item.id) as unique identifier
                path: trash_path.to_string_lossy().to_string(),
                original_path: original_path.to_string_lossy().to_string(),
                is_dir,
                size,
                deleted_time,
                modified,
            }
        })
        .collect();

    // Sort by name (folders first, then files)
    entries.sort_by(|a, b| {
        if a.is_dir == b.is_dir {
            a.name.to_lowercase().cmp(&b.name.to_lowercase())
        } else {
            b.is_dir.cmp(&a.is_dir)
        }
    });

    Ok(entries)
}

/// Empty the Recycle Bin (permanently delete all items)
#[tauri::command]
pub async fn empty_trash() -> Result<(), CommandError> {
    info!("Emptying recycle bin...");
    
    #[cfg(target_os = "windows")]
    {
        use windows::core::PCWSTR;
        use windows::Win32::UI::Shell::{SHEmptyRecycleBinW, SHERB_NOCONFIRMATION, SHERB_NOPROGRESSUI, SHERB_NOSOUND};
        use windows::Win32::Foundation::HWND;

        unsafe {
            SHEmptyRecycleBinW(Some(HWND::default()), PCWSTR::default(), SHERB_NOCONFIRMATION | SHERB_NOPROGRESSUI | SHERB_NOSOUND)
                .map_err(|e| CommandError::TrashError(format!("Failed to empty recycle bin: {}", e)))?;
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        trash::os_limited::purge_all(trash::os_limited::list().map_err(|e| CommandError::TrashError(e.to_string()))?)
            .map_err(|e| CommandError::TrashError(e.to_string()))?;
    }

    Ok(())
}

/// Permanently delete specific items from the Recycle Bin
#[tauri::command]
pub async fn purge_recycle_bin(paths: Vec<String>) -> Result<(), CommandError> {
    let trash_items = trash::os_limited::list().map_err(|e| CommandError::TrashError(e.to_string()))?;

    let normalize = |p: &std::path::Path| -> String {
        p.to_string_lossy()
            .to_lowercase()
            .replace("/", "\\")
            .trim_start_matches("\\\\?\\")
            .to_string()
    };

    use std::collections::HashMap;
    let mut items_to_purge = Vec::new();
    let trash_map: HashMap<String, trash::TrashItem> = trash_items
        .into_iter()
        .map(|item| (normalize(&PathBuf::from(&item.id)), item))
        .collect();

    for path_str in paths {
        let path = PathBuf::from(&path_str);
        let normalized_target = normalize(&path);

        if let Some(item) = trash_map.get(&normalized_target) {
            items_to_purge.push(item.clone());
        }
    }

    if !items_to_purge.is_empty() {
        #[cfg(target_os = "windows")]
        {
            use windows::Win32::UI::Shell::{SHFileOperationW, SHFILEOPSTRUCTW, FO_DELETE, FOF_NOCONFIRMATION, FOF_NOERRORUI, FOF_SILENT};
            use windows::core::PCWSTR;
            use windows::Win32::Foundation::HWND;

            let mut buffer: Vec<u16> = Vec::new();
            for item in &items_to_purge {
                let path_str = PathBuf::from(item.id.clone()).to_string_lossy().replace("/", "\\");
                buffer.extend(path_str.encode_utf16());
                buffer.push(0);
            }
            buffer.push(0);

            let mut sh_op = SHFILEOPSTRUCTW {
                hwnd: HWND(std::ptr::null_mut()),
                wFunc: FO_DELETE,
                pFrom: PCWSTR(buffer.as_ptr()),
                pTo: PCWSTR(std::ptr::null()),
                fFlags: (FOF_NOCONFIRMATION.0 | FOF_NOERRORUI.0 | FOF_SILENT.0) as u16,
                fAnyOperationsAborted: Default::default(),
                hNameMappings: std::ptr::null_mut(),
                lpszProgressTitle: PCWSTR(std::ptr::null()),
            };

            unsafe {
                let _ = SHFileOperationW(&mut sh_op);
            }
        }

        #[cfg(not(target_os = "windows"))]
        {
            trash::os_limited::purge_all(items_to_purge).map_err(|e| CommandError::TrashError(e.to_string()))?;
        }
    }

    Ok(())
}

#[tauri::command]
pub async fn check_conflicts(
    paths: Vec<String>,
    target_dir: String,
) -> Result<ConflictResponse, CommandError> {
    let mut conflicts = Vec::new();
    let target_base = validate_path(&target_dir)?;

    let mut is_cross_volume = false;
    let mut total_size = 0;
    let mut total_files = 0;
    let mut likely_large = false;

    let target_root = target_base.components().next();
    let start_time = std::time::Instant::now();

    for path_str in paths {
        let source_path = validate_path(&path_str)?;

        // Cross-volume check
        if let Some(t_root) = target_root {
            if let Some(s_root) = source_path.components().next() {
                if t_root != s_root {
                    is_cross_volume = true;
                }
            }
        }

        // Quick capped estimation walk
        if !likely_large {
            if source_path.is_dir() {
                for entry in walkdir::WalkDir::new(&source_path).into_iter().filter_map(|e| e.ok()) {
                    if entry.file_type().is_file() {
                        total_size += entry.metadata().map(|m| m.len()).unwrap_or(0);
                        total_files += 1;
                    }
                    // Cap at 200 items or 50ms to keep UI responsive
                    if total_files > 200 || start_time.elapsed().as_millis() > 50 {
                        likely_large = true;
                        break;
                    }
                }
            } else {
                total_size += std::fs::metadata(&source_path).map(|m| m.len()).unwrap_or(0);
                total_files += 1;
            }
        }

        // Conflict check
        if let Some(file_name) = source_path.file_name() {
            let target_path = target_base.join(file_name);
            if target_path.exists() {
                let source_entry = get_file_entry_from_path(&source_path)?;
                let target_entry = get_file_entry_from_path(&target_path)?;
                conflicts.push(ConflictEntry {
                    name: file_name.to_string_lossy().to_string(),
                    source: source_entry,
                    target: target_entry,
                });
            }
        }
    }

    Ok(ConflictResponse {
        conflicts,
        total_size,
        total_files,
        is_cross_volume,
        likely_large,
    })
}





#[tauri::command]
pub async fn delete_items(app: AppHandle, manager: State<'_, FileOperationManager>, paths: Vec<String>, turbo: Option<bool>) -> Result<String, CommandError> {
    info!("Moving items to trash: {:?}", paths);
    let mut paths_validated = Vec::new();
    for p in paths {
        paths_validated.push(validate_path(&p)?);
    }
    
    let mut op = FileOperation::new(FileOpType::Trash, paths_validated, None);
    if let Some(t) = turbo {
        op.turbo = t;
        op.turbo_flag.store(t, Ordering::Relaxed);
    }
    let id = manager.queue_operation(app, op);
    
    Ok(id)
}

#[tauri::command]
pub async fn purge_items(app: AppHandle, manager: State<'_, FileOperationManager>, paths: Vec<String>, turbo: Option<bool>) -> Result<String, CommandError> {
    info!("Permanently deleting items: {:?}", paths);
    let mut paths_validated = Vec::new();
    for p in paths {
        paths_validated.push(validate_path(&p)?);
    }
    
    let mut op = FileOperation::new(FileOpType::Delete, paths_validated, None);
    if let Some(t) = turbo {
        op.turbo = t;
        op.turbo_flag.store(t, Ordering::Relaxed);
    }
    let id = manager.queue_operation(app, op);
    
    Ok(id)
}

#[tauri::command]
pub async fn copy_items(
    app: AppHandle, 
    manager: State<'_, FileOperationManager>, 
    paths: Vec<String>, 
    target_dir: String, 
    turbo: Option<bool>,
    total_size: Option<u64>,
    total_files: Option<usize>,
    is_cross_volume: Option<bool>,
) -> Result<String, CommandError> {
    let target_dir_validated = validate_path(&target_dir)?;
    let paths_validated: Vec<PathBuf> = paths.iter()
        .map(|p| validate_path(p))
        .collect::<Result<Vec<PathBuf>, CommandError>>()?;

    let mut op = FileOperation::new(FileOpType::Copy, paths_validated, Some(target_dir_validated));
    if let Some(t) = turbo {
        op.turbo = t;
        op.turbo_flag.store(t, Ordering::Relaxed);
    }
    if let Some(s) = total_size { op.total_bytes = s; }
    if let Some(f) = total_files { op.total_files = f; }
    if let Some(cv) = is_cross_volume { op.is_cross_volume = cv; }
    
    let id = manager.queue_operation(app, op);
    
    Ok(id)
}

#[tauri::command]
pub async fn move_items(
    app: AppHandle, 
    manager: State<'_, FileOperationManager>, 
    paths: Vec<String>, 
    target_dir: String, 
    turbo: Option<bool>,
    total_size: Option<u64>,
    total_files: Option<usize>,
    is_cross_volume: Option<bool>,
) -> Result<String, CommandError> {
    let target_dir_validated = validate_path(&target_dir)?;
    let paths_validated: Vec<PathBuf> = paths.iter()
        .map(|p| validate_path(p))
        .collect::<Result<Vec<PathBuf>, CommandError>>()?;
    
    let mut op = FileOperation::new(FileOpType::Move, paths_validated, Some(target_dir_validated));
    if let Some(t) = turbo {
        op.turbo = t;
        op.turbo_flag.store(t, Ordering::Relaxed);
    }
    if let Some(s) = total_size { op.total_bytes = s; }
    if let Some(f) = total_files { op.total_files = f; }
    if let Some(cv) = is_cross_volume { op.is_cross_volume = cv; }
    
    let id = manager.queue_operation(app, op);

    Ok(id)
}

#[tauri::command]
pub async fn restore_items(paths: Vec<String>) -> Result<Vec<String>, CommandError> {
    info!("Restoring items: {:?}", paths);
    let trash_items = trash::os_limited::list().map_err(|e| CommandError::TrashError(e.to_string()))?;

    let mut restoration_tasks = Vec::new();
    let mut restored_paths = Vec::new();

    let normalize = |p: &std::path::Path| -> String {
        p.to_string_lossy()
            .to_lowercase()
            .replace("/", "\\")
            .trim_start_matches("\\\\?\\")
            .to_string()
    };

    for path_str in paths {
        let path = PathBuf::from(&path_str);
        let normalized_target = normalize(&path);

        info!(
            "Looking for target by id or original path: {}",
            normalized_target
        );

        // Match by internal trash id OR original path
        let found = trash_items.iter().find(|item| {
            let item_id = normalize(&PathBuf::from(&item.id));
            if item_id == normalized_target {
                return true;
            }

            let original_path = normalize(&item.original_path());
            if original_path == normalized_target {
                return true;
            }

            // Special case for shortcuts/urls where extension might be missing in original_path
            if normalized_target.ends_with(".lnk") || normalized_target.ends_with(".url") {
                let stem = normalized_target
                    .rsplit_once('.')
                    .map(|(s, _)| s)
                    .unwrap_or(&normalized_target);
                if original_path == stem {
                    return true;
                }
            }

            false
        });

        if let Some(item) = found {
            info!("Found match: {:?}", item.original_path());
            restoration_tasks.push((item.clone(), path));
        } else {
            warn!("No match found for: {}", path_str);
        }
    }

    if restoration_tasks.is_empty() {
        warn!("No matching items found to restore.");
        return Err(CommandError::TrashError("No matching items found in Recycle Bin".to_string()));
    }

    for (item, intended_path) in restoration_tasks {
        let original_path = item.original_path(); // Capture original_path before restore
        let original_path_str = original_path.to_string_lossy().to_string();
        
        trash::os_limited::restore_all(vec![item.clone()]).map_err(|e| CommandError::TrashError(e.to_string()))?;
        restored_paths.push(original_path_str);

        // Check if we restored by ID (from Trash View) or by Original Path (Undo)
        // If intended_path matches the internal trash ID, we should NOT try to rename the result to intended_path.
        let intended_normalized = normalize(&intended_path);
        let id_normalized = normalize(&PathBuf::from(&item.id));

        if intended_normalized == id_normalized {
            continue;
        }

        // After restore, if it didn't land at intended_path, check for truncated landing
        if !intended_path.exists() {
            if original_path.exists() { // Use the captured original_path
                info!(
                    "Fixing extension after restore (original_path exists): {:?} -> {:?}",
                    original_path, intended_path
                );
                let _ = std::fs::rename(&original_path, &intended_path);
            } else {
                // Try corrected path as well just in case
                let res_paths = get_restoration_paths(&item);
                if res_paths.crate_path != intended_path && res_paths.crate_path.exists() {
                    info!(
                        "Fixing extension after restore (res_paths exists): {:?} -> {:?}",
                        res_paths.crate_path, intended_path
                    );
                    let _ = std::fs::rename(&res_paths.crate_path, &intended_path);
                }
            }
        }
    }

    Ok(restored_paths)
}

/// Move items from trash to a target directory safely
/// Uses temp directory as intermediate to avoid overwriting files at original location
#[tauri::command]
pub async fn move_from_trash(app: AppHandle, paths: Vec<String>, target_dir: String) -> Result<(), CommandError> {
    let trash_items = trash::os_limited::list().map_err(|e| CommandError::TrashError(e.to_string()))?;

    let normalize = |p: &std::path::Path| -> String {
        p.to_string_lossy()
            .to_lowercase()
            .replace("/", "\\")
            .trim_start_matches("\\\\?\\")
            .to_string()
    };

    let target_base = validate_path(&target_dir)?;

    // Create a unique temp directory for this operation
    let temp_dir = std::env::temp_dir().join(format!("biluf_trash_restore_{}", std::process::id()));
    if !temp_dir.exists() {
        std::fs::create_dir_all(&temp_dir)?;
    }

    let mut restored_files = Vec::new();

    for path_str in &paths {
        let path = validate_path(path_str)?;
        let normalized_target = normalize(&path);

        // Find the trash item by internal ID
        let found = trash_items.iter().find(|item| {
            let item_id = normalize(&PathBuf::from(&item.id));
            item_id == normalized_target
        });

        if let Some(item) = found {
            let res_paths = get_restoration_paths(item);
            let original_name = res_paths
                .corrected_path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| item.name.clone());

            let original_path = res_paths.crate_path.clone();

            // Check if a file with the same name exists at original location
            let conflict_at_original =
                res_paths.crate_path.exists() || res_paths.corrected_path.exists();
            let mut backup_path: Option<PathBuf> = None;

            // If there's a conflict, temporarily move the existing file
            if conflict_at_original {
                backup_path = Some(temp_dir.join(format!("backup_{}", original_name)));
                let options = fs_extra::dir::CopyOptions::new().overwrite(true);
                fs_extra::move_items(&[&original_path], &temp_dir, &options)
                    .map_err(|e| CommandError::IoError(format!("Failed to backup existing file: {}", e)))?;
                // Rename the backup to include prefix
                let moved_to = temp_dir.join(&original_name);
                if moved_to.exists() {
                    std::fs::rename(&moved_to, backup_path.as_ref().ok_or(CommandError::Other("Backup path missing".to_string()))?)
                        .map_err(|e| CommandError::IoError(format!("Failed to rename backup: {}", e)))?;
                }
            }

            // Restore the item from trash (goes to original location)
            trash::os_limited::restore_all(vec![item.clone()]).map_err(|e| CommandError::TrashError(e.to_string()))?;

            // Determine where it actually landed and ensure extension is correct
            let actual_restored_path = if res_paths.corrected_path.exists() {
                res_paths.corrected_path
            } else if res_paths.crate_path.exists() {
                if res_paths.crate_path != res_paths.corrected_path {
                    let _ = std::fs::rename(&res_paths.crate_path, &res_paths.corrected_path);
                    res_paths.corrected_path
                } else {
                    res_paths.crate_path
                }
            } else {
                res_paths.crate_path
            };

            // Move from original location to target
            let dest_path = target_base.join(&original_name);

            if actual_restored_path.exists() && actual_restored_path != dest_path {
                let options = fs_extra::dir::CopyOptions::new().overwrite(true);
                fs_extra::move_items(&[&actual_restored_path], &target_base, &options)
                    .map_err(|e| CommandError::IoError(e.to_string()))?;
            }
            
            if dest_path.exists() {
                restored_files.push(dest_path.to_string_lossy().to_string());
            }

            // Restore the backup if there was a conflict
            if let Some(backup) = backup_path {
                if backup.exists() {
                    let options = fs_extra::dir::CopyOptions::new().overwrite(true);
                    // Move back to original location with original name
                    let restore_target = if original_path.exists() {
                        // If something took the crate_path, we still try to restore the backup
                        original_path
                    } else {
                        // Prefer the path it was originally at
                        actual_restored_path
                    };
                    let parent = restore_target.parent().unwrap_or(&restore_target);
                    std::fs::rename(&backup, &restore_target)
                        .or_else(|_| {
                            fs_extra::move_items(&[&backup], parent, &options)
                                .map(|_| ())
                                .map_err(|e| {
                                    std::io::Error::other(e.to_string())
                                })
                        })
                        .map_err(|e| CommandError::IoError(format!("Failed to restore backup: {}", e)))?;
                }
            }
        }
    }

    // Clean up temp dir
    let _ = std::fs::remove_dir_all(&temp_dir);

    // Record History
    use tauri::Manager;
    let history = app.state::<HistoryManager>();
    let abs_restored: Vec<String> = restored_files.to_vec();
    let target_abs = target_base.to_string_lossy().to_string();

    if !abs_restored.is_empty() {
        let tx_details = TransactionDetails {
            paths: vec![], // Source paths in trash are not reliable for redo usually
            target_dir: Some(target_abs),
            old_path: None,
            new_path: None,
            created_files: Some(abs_restored),
        };
        history.push(Transaction::new(TransactionType::Restore, tx_details));
    }

    Ok(())
}

#[tauri::command]
pub fn get_history(history: State<'_, HistoryManager>) -> Result<crate::models::history::HistoryState, CommandError> {
    Ok(history.get_state())
}

fn fast_trash(paths: Vec<PathBuf>) -> Result<(), CommandError> {
    if paths.is_empty() { return Ok(()); }
    
    #[cfg(target_os = "windows")]
    {
        use windows::Win32::UI::Shell::{SHFileOperationW, SHFILEOPSTRUCTW, FO_DELETE, FOF_ALLOWUNDO, FOF_NOCONFIRMATION, FOF_NOERRORUI, FOF_SILENT};
        use windows::core::PCWSTR;
        use windows::Win32::Foundation::HWND;

        let mut buffer: Vec<u16> = Vec::new();
        for src in paths {
            let path_str = src.to_string_lossy().replace("/", "\\");
            buffer.extend(path_str.encode_utf16());
            buffer.push(0);
        }
        buffer.push(0);

        let mut sh_op = SHFILEOPSTRUCTW {
            hwnd: HWND(std::ptr::null_mut()),
            wFunc: FO_DELETE,
            pFrom: PCWSTR(buffer.as_ptr()),
            pTo: PCWSTR(std::ptr::null()),
            fFlags: (FOF_ALLOWUNDO.0 | FOF_NOCONFIRMATION.0 | FOF_NOERRORUI.0 | FOF_SILENT.0) as u16,
            fAnyOperationsAborted: Default::default(),
            hNameMappings: std::ptr::null_mut(),
            lpszProgressTitle: PCWSTR(std::ptr::null()),
        };

        unsafe {
            let result = SHFileOperationW(&mut sh_op);
            if result != 0 {
                return Err(CommandError::TrashError(format!("Windows Shell Error (0x{:X}).", result)));
            }
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        trash::delete_all(paths).map_err(|e| CommandError::TrashError(e.to_string()))?;
    }
    
    Ok(())
}

#[tauri::command]
pub async fn undo_last_action(app: AppHandle, history: State<'_, HistoryManager>) -> Result<Option<Transaction>, CommandError> {
    let transaction = history.pop_undo();
    
    if let Some(ref tx) = transaction {
        info!("Undoing transaction: {:?}", tx.op_type);
        match tx.op_type {
            TransactionType::Copy => {
                // Undo Copy = Delete the copied files at target
                if let Some(ref target_dir) = tx.details.target_dir {
                    let mut files_to_delete = Vec::new();
                    for src_path in &tx.details.paths {
                        let path = PathBuf::from(&src_path);
                        if let Some(name) = path.file_name() {
                            let dest_path = PathBuf::from(&target_dir).join(name);
                            if dest_path.exists() {
                                files_to_delete.push(dest_path);
                            }
                        }
                    }
                    if !files_to_delete.is_empty() {
                         fast_trash(files_to_delete)?;
                    }
                }
            },
            TransactionType::Move => {
                // Undo Move = Move files back from target to source
                if let Some(ref target_dir) = tx.details.target_dir {
                     let mut files_to_copy_delete = Vec::new();
                     let mut total_size = 0;
                     let target_path_base = PathBuf::from(&target_dir);

                     for src_path_str in &tx.details.paths {
                         let src_path = PathBuf::from(src_path_str);
                         if let Some(name) = src_path.file_name() {
                             let current_loc = target_path_base.join(name);
                             if current_loc.exists() {
                                 // Try atomic rename first
                                 match std::fs::rename(&current_loc, &src_path) {
                                     Ok(_) => {
                                         info!("Undo Move: Fast-moved back {:?} to {:?}", current_loc, src_path);
                                     },
                                     Err(_) => {
                                         // Fallback to copy-delete
                                         if let Ok((collected, size)) = collect_files(&[current_loc], &src_path.parent().unwrap_or(&src_path)) {
                                             files_to_copy_delete.extend(collected);
                                             total_size += size;
                                         }
                                     }
                                 }
                             }
                         }
                     }
                     if !files_to_copy_delete.is_empty() {
                         let cancel_flag = Arc::new(AtomicBool::new(false));
                         perform_copy_with_progress(&app, files_to_copy_delete, total_size, "undo_move", true, cancel_flag)?;
                     }
                }
            },
            TransactionType::Delete => {
                // Undo Delete = Restore from Trash
                restore_items(tx.details.paths.clone()).await?;
            },
            TransactionType::Rename => {
                // Undo Rename = Rename new_path back to old_path
                if let (Some(old), Some(new)) = (tx.details.old_path.as_ref(), tx.details.new_path.as_ref()) {
                    let old_pb = PathBuf::from(old);
                    let new_pb = PathBuf::from(new);
                    if new_pb.exists() {
                        std::fs::rename(new_pb, old_pb).map_err(|e| CommandError::IoError(e.to_string()))?;
                    }
                }
            },
            TransactionType::NewFolder => {
                // Undo NewFolder = Delete the folder (move to trash)
                let mut files_to_delete = Vec::new();
                for path_str in &tx.details.paths {
                    let path = PathBuf::from(path_str);
                    if path.exists() {
                        files_to_delete.push(path);
                    }
                }
                if !files_to_delete.is_empty() {
                    fast_trash(files_to_delete)?;
                }
            },
            TransactionType::Restore => {
                // Undo Restore = Delete the restored files (move back to trash)
                if let Some(ref created) = tx.details.created_files {
                    let mut files_to_delete = Vec::new();
                    for path_str in created {
                        let path = PathBuf::from(path_str);
                        if path.exists() {
                            files_to_delete.push(path);
                        }
                    }
                    if !files_to_delete.is_empty() {
                         fast_trash(files_to_delete)?;
                    }
                }
            },
        }
        
        // Push to Redo stack?
        history.push_redo(tx.clone());
    } // else nothing to undo
    
    Ok(transaction)
}

#[tauri::command]
pub async fn redo_last_action(app: AppHandle, history: State<'_, HistoryManager>) -> Result<Option<Transaction>, CommandError> {
    let transaction = history.pop_redo();

    if let Some(ref tx) = transaction {
        info!("Redoing transaction: {:?}", tx.op_type);
        match tx.op_type {
            TransactionType::Delete => {
                // Redo Delete = Delete again (Recycle Bin)
                let paths: Vec<PathBuf> = tx.details.paths.iter().map(PathBuf::from).collect();
                fast_trash(paths)?;
            },
            TransactionType::Restore => {
                if let Some(ref target_dir) = tx.details.target_dir {
                    let paths = tx.details.paths.clone();
                    move_from_trash(app.clone(), paths, target_dir.clone()).await?;
                }
            },
            TransactionType::Copy => {
                if let Some(ref target_dir) = tx.details.target_dir {
                    // Re-execute Copy
                    // Collect files from Source (paths) to Target
                     let target_base = PathBuf::from(target_dir);
                     let paths: Vec<PathBuf> = tx.details.paths.iter().map(PathBuf::from).collect();
                     if let Ok((files, total_bytes)) = collect_files(&paths, &target_base) {
                          let cancel_flag = Arc::new(AtomicBool::new(false));
                          perform_copy_with_progress(&app, files, total_bytes, "redo_copy", false, cancel_flag)?;
                     }
                }
            },
            TransactionType::Move => {
                if let Some(ref target_dir) = tx.details.target_dir {
                     // Re-execute Move
                     let mut files_to_copy_delete = Vec::new();
                     let mut total_size = 0;
                     let target_path_base = PathBuf::from(&target_dir);
                     let paths: Vec<PathBuf> = tx.details.paths.iter().map(PathBuf::from).collect();

                     for src_path in paths {
                         if src_path.exists() {
                             let file_name = src_path.file_name().unwrap_or_default();
                             let dest_path = target_path_base.join(file_name);
                             
                             // Try atomic rename first
                             match std::fs::rename(&src_path, &dest_path) {
                                 Ok(_) => {
                                     info!("Redo Move: Fast-moved {:?} to {:?}", src_path, dest_path);
                                 },
                                 Err(_) => {
                                     // Fallback to copy-delete
                                     if let Ok((collected, size)) = collect_files(&[src_path], &target_path_base) {
                                         files_to_copy_delete.extend(collected);
                                         total_size += size;
                                     }
                                 }
                             }
                         }
                     }

                     if !files_to_copy_delete.is_empty() {
                          let cancel_flag = Arc::new(AtomicBool::new(false));
                          perform_copy_with_progress(&app, files_to_copy_delete, total_size, "redo_move", true, cancel_flag)?;
                     }
                }
            },
            TransactionType::Rename => {
                // Redo Rename = Rename old_path back to new_path
                if let (Some(old), Some(new)) = (tx.details.old_path.as_ref(), tx.details.new_path.as_ref()) {
                    let old_pb = PathBuf::from(old);
                    let new_pb = PathBuf::from(new);
                    if old_pb.exists() {
                        std::fs::rename(old_pb, new_pb).map_err(|e| CommandError::IoError(e.to_string()))?;
                    }
                }
            },
            TransactionType::NewFolder => {
                // Redo NewFolder = Re-create the folder
                for path_str in &tx.details.paths {
                    let path = PathBuf::from(path_str);
                    if !path.exists() {
                        std::fs::create_dir_all(path).map_err(|e| CommandError::IoError(e.to_string()))?;
                    }
                }
            },
        }
        
        // Push back to Undo stack (raw push to avoid clearing redo stack, though we just popped one)
        history.push_undo_raw(tx.clone());
    }
    
    Ok(transaction)
}

// Helper to collect files and calculate size recursively
fn collect_files(paths: &[PathBuf], target_base: &std::path::Path) -> Result<(Vec<(PathBuf, PathBuf)>, u64), CommandError> {
    use walkdir::WalkDir;
    let mut total_bytes: u64 = 0;
    let mut files_to_copy: Vec<(PathBuf, PathBuf)> = Vec::new();

    for path in paths {
        if !path.exists() { continue; }

        let file_name = path.file_name().ok_or(CommandError::PathError("Invalid path".to_string()))?;
        let dest_root = target_base.join(file_name);

        if path.is_dir() {
            for entry in WalkDir::new(path) {
                let entry = entry.map_err(|e| CommandError::IoError(e.to_string()))?;
                let entry_path = entry.path();
                
                let relative = entry_path.strip_prefix(path).map_err(|_| CommandError::PathError("Strip prefix failed".to_string()))?;
                let dest_path = dest_root.join(relative);

                if entry_path.is_dir() {
                    files_to_copy.push((entry_path.to_path_buf(), dest_path));
                } else {
                    let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
                    total_bytes += size;
                    files_to_copy.push((entry_path.to_path_buf(), dest_path));
                }
            }
        } else {
            let size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
            total_bytes += size;
            files_to_copy.push((path.clone(), dest_root));
        }
    }
    Ok((files_to_copy, total_bytes))
}

// Helper to perform copy with progress
fn perform_copy_with_progress(
    app: &AppHandle, 
    files: Vec<(PathBuf, PathBuf)>, 
    total_bytes: u64, 
    task_name: &str,
    move_op: bool, // If true, delete source after copy
    cancel_flag: Arc<AtomicBool>
) -> Result<(), CommandError> {
    use std::fs;
    use std::io::{Read, Write};
    use std::time::Instant;

    let mut processed_global: u64 = 0;
    let mut last_emit = Instant::now();
    let op_id = format!("{}_op", task_name);

    for (source, dest) in &files {
         if cancel_flag.load(Ordering::Relaxed) {
             let _ = app.emit("progress", ProgressEvent {
                id: op_id.clone(),
                task: task_name.to_string(),
                current: processed_global,
                total: total_bytes,
                status: "cancelled".to_string(),
                filename: None,
            });
             return Ok(());
         }

         if source.is_dir() {
             if !dest.exists() {
                 fs::create_dir_all(&dest).map_err(|e| CommandError::IoError(e.to_string()))?;
             }
             continue;
         }

         // Create parent if needed
         if let Some(parent) = dest.parent() {
             if !parent.exists() {
                 fs::create_dir_all(parent).map_err(|e| CommandError::IoError(e.to_string()))?;
             }
         }

         let mut file_in = fs::File::open(&source).map_err(|e| CommandError::IoError(e.to_string()))?;
         let mut file_out = fs::File::create(&dest).map_err(|e| CommandError::IoError(e.to_string()))?;
         
         let mut buffer = [0u8; 81920]; 
         loop {
             if cancel_flag.load(Ordering::Relaxed) {
                 // Clean up partial destination file to avoid leaving corrupted data
                 drop(file_out);
                 let _ = fs::remove_file(&dest);
                 return Ok(());
             }

             let n = file_in.read(&mut buffer).map_err(|e| CommandError::IoError(e.to_string()))?;
             if n == 0 { break; }
             file_out.write_all(&buffer[..n]).map_err(|e| CommandError::IoError(e.to_string()))?;
             
             processed_global += n as u64;

             if last_emit.elapsed().as_millis() > 100 {
                 let _ = app.emit("progress", ProgressEvent {
                    id: op_id.clone(),
                    task: task_name.to_string(),
                    current: processed_global,
                    total: total_bytes,
                    status: "running".to_string(),
                    filename: source.file_name().map(|s| s.to_string_lossy().to_string()),
                });
                last_emit = Instant::now();
             }
         }

          if move_op && !source.is_dir() {
              let _ = fs::remove_file(&source);
          }
    }
    
    // Final cleanup for directories if move_op is true
    if move_op {
        // We don't have the original top-level sources easily here without changing the signature.
        // But we can infer them from the files list if we want, or just rely on the fact that
        // directories are now empty and should be removed.
        // Actually, collect_files includes directories in the list.
        // We should delete directories BOTTOM-UP.
        let mut dirs: Vec<_> = files.iter().filter(|(s, _)| s.is_dir()).map(|(s, _)| s).collect();
        dirs.sort_by_key(|d| std::cmp::Reverse(d.components().count()));
        for d in dirs {
            let _ = fs::remove_dir(d); // Only remove if empty
        }
    }
    
    let _ = app.emit("progress", ProgressEvent {
        id: op_id,
        task: task_name.to_string(),
        current: total_bytes,
        total: total_bytes,
        status: "completed".to_string(),
        filename: None,
    });

    Ok(())
}
