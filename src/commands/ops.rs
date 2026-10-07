use crate::models::{
    get_file_entry_from_path, CommandError, ConflictEntry, ConflictResponse, HistoryManager,
    Transaction, TransactionDetails, TransactionType, TrashEntry,
};
use crate::utils::path_security::validate_path;
use log::{info, warn};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
fn rollback_renamed_paths(completed: &[(PathBuf, PathBuf)]) -> Vec<String> {
    let mut errors = Vec::new();
    for (source, destination) in completed.iter().rev() {
        if let Err(error) = std::fs::rename(destination, source) {
            errors.push(format!(
                "{} → {}: {error}",
                destination.display(),
                source.display()
            ));
        }
    }
    errors
}

fn remove_empty_dirs(paths: &[PathBuf]) -> Vec<String> {
    let mut errors = Vec::new();
    for path in paths.iter().rev() {
        match std::fs::remove_dir(path) {
            Ok(()) => {}
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::DirectoryNotEmpty | std::io::ErrorKind::NotFound
                ) => {}
            Err(error) => errors.push(format!("{}: {error}", path.display())),
        }
    }
    errors
}

fn create_dir_all_tracked(
    path: &std::path::Path,
    created: &mut Vec<PathBuf>,
) -> std::io::Result<()> {
    let mut missing = Vec::new();
    let mut current = PathBuf::new();
    for component in path.components() {
        current.push(component);
        if !current.exists() {
            missing.push(current.clone());
        }
    }
    for directory in missing {
        match std::fs::create_dir(&directory) {
            Ok(()) => created.push(directory),
            Err(error)
                if error.kind() == std::io::ErrorKind::AlreadyExists && directory.is_dir() => {}
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

fn hash_file(path: &std::path::Path) -> Result<String, CommandError> {
    use std::io::Read;
    let mut file =
        std::fs::File::open(path).map_err(|error| CommandError::IoError(error.to_string()))?;
    let mut hasher = blake3::Hasher::new();
    let mut buffer = [0u8; 128 * 1024];
    loop {
        let count = file
            .read(&mut buffer)
            .map_err(|error| CommandError::IoError(error.to_string()))?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }
    Ok(hasher.finalize().to_hex().to_string())
}

struct RestorationPaths {
    crate_path: PathBuf,
    corrected_path: PathBuf,
}

/// Windows stores the original path in the `$I…` sidecar of each `$R…` item.
/// The Shell item name can be the opaque recycle-bin identifier, so the `trash`
/// crate may otherwise use that identifier as the restored filename.
#[cfg(target_os = "windows")]
fn windows_original_path(item: &trash::TrashItem) -> Option<PathBuf> {
    use std::os::windows::ffi::OsStringExt;

    let id_path = PathBuf::from(&item.id);
    let file_name = id_path.file_name()?.to_string_lossy();
    if file_name.len() < 3 || !file_name[..2].eq_ignore_ascii_case("$R") {
        return None;
    }
    let suffix = file_name.get(2..)?;

    let metadata_path = id_path.with_file_name(format!("$I{suffix}"));
    let bytes = std::fs::read(metadata_path).ok()?;
    if bytes.len() < 28 {
        return None;
    }

    let version = u64::from_le_bytes(bytes.get(..8)?.try_into().ok()?);
    if version != 1 && version != 2 {
        return None;
    }
    let char_count = u32::from_le_bytes(bytes.get(24..28)?.try_into().ok()?) as usize;
    let path_bytes = bytes.get(28..28usize.checked_add(char_count.checked_mul(2)?)?)?;
    let wide_path: Vec<u16> = path_bytes
        .chunks_exact(2)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .collect();
    let path = PathBuf::from(std::ffi::OsString::from_wide(&wide_path));
    path.is_absolute().then_some(path)
}

fn get_restoration_paths(item: &trash::TrashItem) -> RestorationPaths {
    #[cfg(target_os = "windows")]
    let crate_path = windows_original_path(item).unwrap_or_else(|| item.original_path());
    #[cfg(not(target_os = "windows"))]
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

fn item_with_correct_restore_name(item: &trash::TrashItem) -> trash::TrashItem {
    let mut item = item.clone();
    let original_path = get_restoration_paths(&item).corrected_path;
    if let Some(name) = original_path.file_name() {
        item.name = name.to_os_string();
        if let Some(parent) = original_path.parent() {
            item.original_parent = parent.to_path_buf();
        }
    }
    item
}

/// List all items in the Windows Recycle Bin
pub fn list_trash() -> Result<Vec<TrashEntry>, CommandError> {
    let trash_items =
        trash::os_limited::list().map_err(|e| CommandError::TrashError(e.to_string()))?;

    let mut entries: Vec<TrashEntry> = trash_items
        .into_iter()
        .map(|item| {
            let original_path = get_restoration_paths(&item).corrected_path;
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
pub async fn empty_trash() -> Result<(), CommandError> {
    info!("Emptying recycle bin...");

    #[cfg(target_os = "windows")]
    {
        use windows::core::PCWSTR;
        use windows::Win32::Foundation::HWND;
        use windows::Win32::UI::Shell::{
            SHEmptyRecycleBinW, SHERB_NOCONFIRMATION, SHERB_NOPROGRESSUI, SHERB_NOSOUND,
        };

        unsafe {
            SHEmptyRecycleBinW(
                Some(HWND::default()),
                PCWSTR::default(),
                SHERB_NOCONFIRMATION | SHERB_NOPROGRESSUI | SHERB_NOSOUND,
            )
            .map_err(|e| CommandError::TrashError(format!("Failed to empty recycle bin: {}", e)))?;
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        trash::os_limited::purge_all(
            trash::os_limited::list().map_err(|e| CommandError::TrashError(e.to_string()))?,
        )
        .map_err(|e| CommandError::TrashError(e.to_string()))?;
    }

    Ok(())
}

/// Permanently delete specific items from the Recycle Bin
pub async fn purge_recycle_bin(paths: Vec<String>) -> Result<(), CommandError> {
    let trash_items =
        trash::os_limited::list().map_err(|e| CommandError::TrashError(e.to_string()))?;

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
            use windows::core::PCWSTR;
            use windows::Win32::Foundation::HWND;
            use windows::Win32::UI::Shell::{
                SHFileOperationW, FOF_NOCONFIRMATION, FOF_NOERRORUI, FOF_SILENT, FO_DELETE,
                SHFILEOPSTRUCTW,
            };

            let mut buffer: Vec<u16> = Vec::new();
            for item in &items_to_purge {
                let path_str = PathBuf::from(item.id.clone())
                    .to_string_lossy()
                    .replace("/", "\\");
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
            trash::os_limited::purge_all(items_to_purge)
                .map_err(|e| CommandError::TrashError(e.to_string()))?;
        }
    }

    Ok(())
}

pub async fn check_conflicts(
    paths: Vec<String>,
    target_dir: String,
) -> Result<ConflictResponse, CommandError> {
    use crate::systems::archive_workspace::{
        virtual_destination, virtual_source, ArchiveWorkspaces,
    };
    if paths
        .iter()
        .any(|p| virtual_source(std::path::Path::new(p)).is_some())
        || virtual_destination(std::path::Path::new(&target_dir)).is_some()
    {
        let mut workspaces = ArchiveWorkspaces::default();
        let target = workspaces
            .resolve(std::path::Path::new(&target_dir), true)
            .map_err(CommandError::ArchiveError)?;
        let sources = paths
            .iter()
            .map(|p| {
                workspaces
                    .resolve(std::path::Path::new(p), false)
                    .map(|p| p.to_string_lossy().into_owned())
            })
            .collect::<Result<Vec<_>, _>>()
            .map_err(CommandError::ArchiveError)?;
        let mut response = check_conflicts_real(sources, target.to_string_lossy().into_owned())?;
        for conflict in &mut response.conflicts {
            conflict.source.path = workspaces
                .display(std::path::Path::new(&conflict.source.path))
                .to_string_lossy()
                .into_owned();
            conflict.target.path = workspaces
                .display(std::path::Path::new(&conflict.target.path))
                .to_string_lossy()
                .into_owned();
        }
        return Ok(response);
    }
    check_conflicts_real(paths, target_dir)
}

fn check_conflicts_real(
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
                for entry in walkdir::WalkDir::new(&source_path)
                    .into_iter()
                    .filter_map(|e| e.ok())
                {
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
                total_size += std::fs::metadata(&source_path)
                    .map(|m| m.len())
                    .unwrap_or(0);
                total_files += 1;
            }
        }

        // Conflict check
        if let Some(file_name) = source_path.file_name() {
            let target_path = target_base.join(file_name);
            if target_path.exists() {
                // Initial top-level conflict found
                add_conflict_recursive(&source_path, &target_path, &mut conflicts)?;
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

fn add_conflict_recursive(
    source_path: &std::path::Path,
    target_path: &std::path::Path,
    conflicts: &mut Vec<ConflictEntry>,
) -> Result<(), CommandError> {
    let mut source_entry = get_file_entry_from_path(source_path)?;
    let mut target_entry = get_file_entry_from_path(target_path)?;

    if source_entry.is_dir {
        // Calculate size and item count for the folder itself
        let mut s = 0;
        let mut f: u64 = 0;
        let mut i = 0;
        for entry in walkdir::WalkDir::new(source_path)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            if entry.file_type().is_dir() {
                f += 1;
            } else {
                i += 1;
                s += entry.metadata().map(|m| m.len()).unwrap_or(0);
            }
        }
        f = f.saturating_sub(1);
        source_entry.size = s;
        source_entry.folders_count = Some(f);
        source_entry.files_count = Some(i);

        // Do the same for target folder
        let mut ts = 0;
        let mut tf: u64 = 0;
        let mut ti = 0;
        for entry in walkdir::WalkDir::new(target_path)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            if entry.file_type().is_dir() {
                tf += 1;
            } else {
                ti += 1;
                ts += entry.metadata().map(|m| m.len()).unwrap_or(0);
            }
        }
        tf = tf.saturating_sub(1);
        target_entry.size = ts;
        target_entry.folders_count = Some(tf);
        target_entry.files_count = Some(ti);

        // Add the folder conflict itself
        conflicts.push(ConflictEntry {
            name: source_path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default(),
            source: source_entry,
            target: target_entry,
        });

        // Now recursively check children for conflicts
        for entry in std::fs::read_dir(source_path)? {
            let entry = entry?;
            let entry_path = entry.path();
            let entry_name = entry.file_name();
            let target_child_path = target_path.join(&entry_name);

            if target_child_path.exists() {
                add_conflict_recursive(&entry_path, &target_child_path, conflicts)?;
            }
        }
    } else {
        // Individual file conflict
        conflicts.push(ConflictEntry {
            name: source_path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default(),
            source: source_entry,
            target: target_entry,
        });
    }

    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrashSystemConfig {
    pub drive_path: String,
    pub max_size_mb: u64,
    pub nuke_on_delete: bool,
}

pub async fn get_total_recycle_bin_usage() -> Result<(u64, u64), CommandError> {
    crate::platform::platform().recycle_bin_usage().map_err(CommandError::SystemError)
}

pub async fn restore_items(paths: Vec<String>) -> Result<Vec<String>, CommandError> {
    info!("Restoring items: {:?}", paths);
    let trash_items =
        trash::os_limited::list().map_err(|e| CommandError::TrashError(e.to_string()))?;

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
        return Err(CommandError::TrashError(
            "No matching items found in Recycle Bin".to_string(),
        ));
    }

    for (item, intended_path) in restoration_tasks {
        let item = item_with_correct_restore_name(&item);
        let original_path = get_restoration_paths(&item).corrected_path;
        let original_path_str = original_path.to_string_lossy().to_string();

        trash::os_limited::restore_all(vec![item.clone()])
            .map_err(|e| CommandError::TrashError(e.to_string()))?;
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
            if original_path.exists() {
                // Use the captured original_path
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
pub async fn move_from_trash(
    history: &HistoryManager,
    paths: Vec<String>,
    target_dir: String,
) -> Result<(), CommandError> {
    let trash_items =
        trash::os_limited::list().map_err(|e| CommandError::TrashError(e.to_string()))?;

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
                .unwrap_or_else(|| item.name.to_string_lossy().into_owned());

            let original_path = res_paths.crate_path.clone();

            // Check if a file with the same name exists at original location
            let conflict_at_original =
                res_paths.crate_path.exists() || res_paths.corrected_path.exists();
            let mut backup_path: Option<PathBuf> = None;

            // If there's a conflict, temporarily move the existing file
            if conflict_at_original {
                backup_path = Some(temp_dir.join(format!("backup_{}", original_name)));
                let options = fs_extra::dir::CopyOptions::new().overwrite(true);
                fs_extra::move_items(&[&original_path], &temp_dir, &options).map_err(|e| {
                    CommandError::IoError(format!("Failed to backup existing file: {}", e))
                })?;
                // Rename the backup to include prefix
                let moved_to = temp_dir.join(&original_name);
                if moved_to.exists() {
                    std::fs::rename(
                        &moved_to,
                        backup_path
                            .as_ref()
                            .ok_or(CommandError::Other("Backup path missing".to_string()))?,
                    )
                    .map_err(|e| {
                        CommandError::IoError(format!("Failed to rename backup: {}", e))
                    })?;
                }
            }

            // Restore the item from trash (goes to original location)
            trash::os_limited::restore_all(vec![item_with_correct_restore_name(item)])
                .map_err(|e| CommandError::TrashError(e.to_string()))?;

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
                                .map_err(|e| std::io::Error::other(e.to_string()))
                        })
                        .map_err(|e| {
                            CommandError::IoError(format!("Failed to restore backup: {}", e))
                        })?;
                }
            }
        }
    }

    // Clean up temp dir
    let _ = std::fs::remove_dir_all(&temp_dir);

    // Record History
    let abs_restored: Vec<String> = restored_files.to_vec();
    let target_abs = target_base.to_string_lossy().to_string();

    if !abs_restored.is_empty() {
        let tx_details = TransactionDetails {
            paths: vec![], // Source paths in trash are not reliable for redo usually
            target_dir: Some(target_abs),
            old_path: None,
            new_path: None,
            created_files: Some(abs_restored),
            created_dirs: None,
            replaced_files: None,
            overwritten_files: None,
            moved_files: None,
            moved_dirs: None,
            copied_files: None,
            copied_file_hashes: None,
        };
        history.push(Transaction::new(TransactionType::Restore, tx_details));
    }

    Ok(())
}

fn fast_trash(paths: Vec<PathBuf>) -> Result<(), CommandError> {
    if paths.is_empty() {
        return Ok(());
    }

    #[cfg(target_os = "windows")]
    {
        use windows::core::PCWSTR;
        use windows::Win32::Foundation::HWND;
        use windows::Win32::UI::Shell::{
            SHFileOperationW, FOF_ALLOWUNDO, FOF_NOCONFIRMATION, FOF_NOERRORUI, FOF_SILENT,
            FO_DELETE, SHFILEOPSTRUCTW,
        };

        // Heuristic: Check size before calling Shell if single large file or small set
        // to avoid triggering UAC for something that will fail anyway.
        let mut total_size = 0;
        let mut count = 0;
        for p in &paths {
            if let Ok(m) = std::fs::metadata(p) {
                if m.is_file() {
                    total_size += m.len();
                } else {
                    // Quick check for directories: if it's already > 4GB or we already counted 500 files, stop counting
                    // and just assume it might be large.
                    total_size += 1024 * 1024; // Dummy average for dir entry
                }
            }
            count += 1;
            if total_size > 4 * 1024 * 1024 * 1024 || count > 1000 {
                return Err(CommandError::TrashFull(
                    "Items potentially too large for Recycle Bin.".to_string(),
                ));
            }
        }

        // 4GB is a very safe "might be too large" threshold for default Recycle Bin settings on many drives.
        if total_size > 4 * 1024 * 1024 * 1024 {
            return Err(CommandError::TrashFull(
                "Fichiers trop volumineux pour la corbeille.".to_string(),
            ));
        }

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
            fFlags: (FOF_ALLOWUNDO.0 | FOF_NOCONFIRMATION.0 | FOF_NOERRORUI.0 | FOF_SILENT.0)
                as u16,
            fAnyOperationsAborted: Default::default(),
            hNameMappings: std::ptr::null_mut(),
            lpszProgressTitle: PCWSTR(std::ptr::null()),
        };

        unsafe {
            let result = SHFileOperationW(&mut sh_op);
            if result != 0 {
                if result == 0x4C7 || result == 0x80070005u32 as i32 {
                    // 0x4C7 = Aborted, 0x80070005 = Access Denied (often after UAC refuse)
                    return Err(CommandError::TrashFull(
                        "Opération corbeille impossible ou annulée.".to_string(),
                    ));
                }
                return Err(CommandError::TrashError(format!(
                    "Windows Shell Error (0x{:X}).",
                    result
                )));
            }
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        trash::delete_all(paths).map_err(|e| CommandError::TrashError(e.to_string()))?;
    }

    Ok(())
}

/// Remove files created by a copy operation during Undo. These are verified against
/// the operation journal immediately before this call, so routing them through the
/// Windows Recycle Bin is unnecessary and can fail for otherwise valid paths.
fn remove_undo_copy_files(paths: Vec<PathBuf>) -> Result<(), CommandError> {
    for path in paths {
        std::fs::remove_file(&path).map_err(|error| {
            CommandError::IoError(format!(
                "Cannot remove copied file {}: {error}",
                path.display()
            ))
        })?;
    }
    Ok(())
}
pub async fn undo_last_action(
    history: &HistoryManager,
) -> Result<Option<Transaction>, CommandError> {
    let transaction = history.last_undo();

    if let Some(ref tx) = transaction {
        info!("Undoing transaction: {:?}", tx.op_type);
        if tx.undo_blocked_by_overwrite() {
            return Err(CommandError::Other(
                "This copy overwrote existing files and cannot be undone".to_string(),
            ));
        }
        match tx.op_type {
            TransactionType::Copy => {
                if tx.details.created_files.is_none()
                    || tx.details.created_dirs.is_none()
                    || tx.details.replaced_files.is_none()
                    || tx.details.copied_file_hashes.is_none()
                {
                    return Err(CommandError::Other(
                        "This copy predates the effect journal and cannot be undone safely"
                            .to_string(),
                    ));
                }
                let expected_hashes: HashMap<_, _> = tx
                    .details
                    .copied_file_hashes
                    .as_deref()
                    .unwrap_or_default()
                    .iter()
                    .map(|(path, hash)| (path.as_str(), hash.as_str()))
                    .collect();
                let undoable_files: std::collections::HashSet<_> = tx
                    .details
                    .created_files
                    .as_deref()
                    .unwrap_or_default()
                    .iter()
                    .map(String::as_str)
                    .chain(
                        tx.details
                            .replaced_files
                            .as_deref()
                            .unwrap_or_default()
                            .iter()
                            .map(|(path, _)| path.as_str()),
                    )
                    .collect();
                for (path, expected) in &expected_hashes {
                    if !undoable_files.contains(path) {
                        continue;
                    }
                    let path = PathBuf::from(path);
                    if path.exists() && hash_file(&path)? != *expected {
                        return Err(CommandError::Other(format!(
                            "Cannot undo copy because the destination changed since the operation: {}",
                            path.display()
                        )));
                    }
                }
                if let Some(replaced) = &tx.details.replaced_files {
                    if let Some((_, missing_backup)) = replaced
                        .iter()
                        .find(|(_, backup)| !PathBuf::from(backup).exists())
                    {
                        return Err(CommandError::IoError(format!(
                            "Cannot restore replacement backup: {}",
                            missing_backup
                        )));
                    }
                }
                if let Some(created) = &tx.details.created_files {
                    if let Some(missing) = created.iter().find(|path| !PathBuf::from(path).exists())
                    {
                        return Err(CommandError::Other(format!(
                            "Cannot undo copy because the copied file is missing: {missing}"
                        )));
                    }
                    let files_to_delete: Vec<PathBuf> = created.iter().map(PathBuf::from).collect();
                    if !files_to_delete.is_empty() {
                        remove_undo_copy_files(files_to_delete)?;
                    }
                }
                if let Some(replaced) = &tx.details.replaced_files {
                    for (destination, backup) in replaced.iter().rev() {
                        let destination = PathBuf::from(destination);
                        let backup = PathBuf::from(backup);
                        if backup.exists() {
                            if destination.exists() {
                                remove_undo_copy_files(vec![destination.clone()])?;
                            }
                            std::fs::rename(&backup, &destination)
                                .map_err(|e| CommandError::IoError(e.to_string()))?;
                        }
                    }
                }
                if let Some(created_dirs) = &tx.details.created_dirs {
                    for directory in created_dirs.iter().rev().map(PathBuf::from) {
                        match std::fs::remove_dir(&directory) {
                            Ok(()) => {}
                            Err(error)
                                if matches!(
                                    error.kind(),
                                    std::io::ErrorKind::DirectoryNotEmpty
                                        | std::io::ErrorKind::NotFound
                                ) => {}
                            Err(error) => {
                                return Err(CommandError::IoError(error.to_string()));
                            }
                        }
                    }
                }
            }
            TransactionType::Move => {
                if tx.details.created_files.is_none()
                    || tx.details.created_dirs.is_none()
                    || tx.details.replaced_files.is_none()
                    || tx.details.moved_files.is_none()
                    || tx.details.moved_dirs.is_none()
                    || tx.details.copied_file_hashes.is_none()
                {
                    return Err(CommandError::Other(
                        "This move predates the effect journal and cannot be undone safely"
                            .to_string(),
                    ));
                }
                let expected_hashes: HashMap<_, _> = tx
                    .details
                    .copied_file_hashes
                    .as_deref()
                    .unwrap_or_default()
                    .iter()
                    .map(|(path, hash)| (path.as_str(), hash.as_str()))
                    .collect();
                if let Some(replaced) = &tx.details.replaced_files {
                    if let Some((_, missing_backup)) = replaced
                        .iter()
                        .find(|(_, backup)| !PathBuf::from(backup).exists())
                    {
                        return Err(CommandError::IoError(format!(
                            "Cannot restore replacement backup: {}",
                            missing_backup
                        )));
                    }
                }
                if let Some(moved) = &tx.details.moved_files {
                    for (source, destination) in moved.iter().rev() {
                        let source = PathBuf::from(source);
                        let destination = PathBuf::from(destination);
                        if destination.exists() {
                            if source.exists() {
                                return Err(CommandError::IoError(format!(
                                    "Cannot undo move because the original path now exists: {}",
                                    source.display()
                                )));
                            }
                            if let Some(expected) =
                                expected_hashes.get(destination.to_string_lossy().as_ref())
                            {
                                if hash_file(&destination)? != **expected {
                                    return Err(CommandError::Other(format!(
                                        "Cannot undo move because the destination changed since the operation: {}",
                                        destination.display()
                                    )));
                                }
                            }
                        }
                    }
                }
                if let Some(moved) = &tx.details.moved_files {
                    let mut completed = Vec::new();
                    let mut created_source_dirs = Vec::new();
                    for (source, destination) in moved.iter().rev() {
                        let source = PathBuf::from(source);
                        let destination = PathBuf::from(destination);
                        if !destination.exists() {
                            continue;
                        }
                        if source.exists() {
                            return Err(CommandError::IoError(format!(
                                "Cannot undo move because the original path now exists: {}",
                                source.display()
                            )));
                        }
                        if let Some(parent) = source.parent() {
                            if let Err(error) =
                                create_dir_all_tracked(parent, &mut created_source_dirs)
                            {
                                let rollback_errors = rollback_renamed_paths(&completed);
                                let cleanup_errors = remove_empty_dirs(&created_source_dirs);
                                return Err(CommandError::IoError(format!(
                                    "{error}; move rollback errors: {}; source directory cleanup errors: {}",
                                    rollback_errors.join(", "),
                                    cleanup_errors.join(", "),
                                )));
                            }
                        }
                        if let Err(error) = std::fs::rename(&destination, &source) {
                            let rollback_errors = rollback_renamed_paths(&completed);
                            let cleanup_errors = remove_empty_dirs(&created_source_dirs);
                            return Err(CommandError::IoError(format!(
                                "{error}; move rollback errors: {}; source directory cleanup errors: {}",
                                rollback_errors.join(", "),
                                cleanup_errors.join(", "),
                            )));
                        }
                        completed.push((destination, source));
                    }
                }
                if let Some(moved_dirs) = &tx.details.moved_dirs {
                    for (source, destination) in moved_dirs {
                        let source = PathBuf::from(source);
                        let destination = PathBuf::from(destination);
                        if !source.exists() {
                            std::fs::create_dir_all(&source)
                                .map_err(|e| CommandError::IoError(e.to_string()))?;
                        }
                        let was_created = tx.details.created_dirs.as_ref().is_some_and(|created| {
                            created
                                .iter()
                                .any(|path| PathBuf::from(path) == destination)
                        });
                        if was_created && destination.exists() {
                            match std::fs::remove_dir(&destination) {
                                Ok(()) => {}
                                Err(error)
                                    if error.kind() == std::io::ErrorKind::DirectoryNotEmpty => {}
                                Err(error) => {
                                    return Err(CommandError::IoError(error.to_string()));
                                }
                            }
                        }
                    }
                }
                if let Some(created) = &tx.details.created_files {
                    let files_to_delete: Vec<PathBuf> = created
                        .iter()
                        .map(PathBuf::from)
                        .filter(|path| path.exists())
                        .collect();
                    if !files_to_delete.is_empty() {
                        remove_undo_copy_files(files_to_delete)?;
                    }
                }
                if let Some(replaced) = &tx.details.replaced_files {
                    for (destination, backup) in replaced.iter().rev() {
                        let destination = PathBuf::from(destination);
                        let backup = PathBuf::from(backup);
                        if backup.exists() {
                            if destination.exists() {
                                remove_undo_copy_files(vec![destination.clone()])?;
                            }
                            std::fs::rename(&backup, &destination)
                                .map_err(|e| CommandError::IoError(e.to_string()))?;
                        }
                    }
                }
                if let Some(created_dirs) = &tx.details.created_dirs {
                    for directory in created_dirs.iter().rev().map(PathBuf::from) {
                        match std::fs::remove_dir(&directory) {
                            Ok(()) => {}
                            Err(error)
                                if matches!(
                                    error.kind(),
                                    std::io::ErrorKind::DirectoryNotEmpty
                                        | std::io::ErrorKind::NotFound
                                ) => {}
                            Err(error) => {
                                return Err(CommandError::IoError(error.to_string()));
                            }
                        }
                    }
                }
            }
            TransactionType::Delete => {
                restore_items(tx.details.paths.clone()).await?;
            }
            TransactionType::Rename => {
                if let (Some(old), Some(new)) =
                    (tx.details.old_path.as_ref(), tx.details.new_path.as_ref())
                {
                    let old_pb = PathBuf::from(old);
                    let new_pb = PathBuf::from(new);
                    if new_pb.exists() {
                        std::fs::rename(new_pb, old_pb)
                            .map_err(|e| CommandError::IoError(e.to_string()))?;
                    }
                }
            }
            TransactionType::NewFolder => {
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
            }
            TransactionType::Restore => {
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
            }
        }
        if !history.complete_undo(tx) {
            return Err(CommandError::Other(
                "Undo history changed while the operation was running".to_string(),
            ));
        }
    }
    Ok(transaction)
}

pub async fn redo_last_action(
    history: &HistoryManager,
) -> Result<Option<Transaction>, CommandError> {
    let transaction = history.last_redo();

    if let Some(ref tx) = transaction {
        info!("Redoing transaction: {:?}", tx.op_type);
        match tx.op_type {
            TransactionType::Delete => {
                let paths: Vec<PathBuf> = tx.details.paths.iter().map(PathBuf::from).collect();
                fast_trash(paths)?;
            }
            TransactionType::Restore => {
                if let Some(ref target_dir) = tx.details.target_dir {
                    let paths = tx.details.paths.clone();
                    move_from_trash(history, paths, target_dir.clone()).await?;
                }
            }
            TransactionType::Copy => {
                if tx.details.copied_files.is_none()
                    || tx.details.created_dirs.is_none()
                    || tx.details.copied_file_hashes.is_none()
                {
                    return Err(CommandError::Other(
                        "This copy predates the effect journal and cannot be redone safely"
                            .to_string(),
                    ));
                }
                if tx
                    .details
                    .replaced_files
                    .as_ref()
                    .is_some_and(|files| !files.is_empty())
                    || tx
                        .details
                        .overwritten_files
                        .as_ref()
                        .is_some_and(|files| !files.is_empty())
                {
                    return Err(CommandError::Other(
                        "Redo for replacement operations requires a new conflict resolution"
                            .to_string(),
                    ));
                }
                if let Some(files) = &tx.details.copied_files {
                    let files: Vec<(PathBuf, PathBuf)> = files
                        .iter()
                        .map(|(source, destination)| {
                            (PathBuf::from(source), PathBuf::from(destination))
                        })
                        .collect();
                    for (source, destination) in &files {
                        if !source.exists() {
                            return Err(CommandError::IoError(format!(
                                "Cannot redo copy; source is missing: {}",
                                source.display()
                            )));
                        }
                        if destination.exists() {
                            return Err(CommandError::IoError(format!(
                                "Cannot redo copy; destination already exists: {}",
                                destination.display()
                            )));
                        }
                        let expected_hash = tx
                            .details
                            .copied_file_hashes
                            .as_deref()
                            .unwrap_or_default()
                            .iter()
                            .find(|(path, _)| path == &destination.to_string_lossy())
                            .map(|(_, hash)| hash.as_str());
                        if let Some(expected_hash) = expected_hash {
                            if hash_file(source)? != expected_hash {
                                return Err(CommandError::Other(format!(
                                    "Cannot redo copy because the source changed since the operation: {}",
                                    source.display()
                                )));
                            }
                        }
                    }
                    let total_bytes = files.iter().try_fold(0u64, |total, (source, _)| {
                        std::fs::metadata(source)
                            .map(|metadata| total.saturating_add(metadata.len()))
                            .map_err(|error| CommandError::IoError(error.to_string()))
                    })?;
                    let mut created_dirs: Vec<PathBuf> = tx
                        .details
                        .created_dirs
                        .as_deref()
                        .unwrap_or_default()
                        .iter()
                        .map(PathBuf::from)
                        .collect();
                    if let Some(existing) = created_dirs.iter().find(|directory| directory.exists())
                    {
                        return Err(CommandError::IoError(format!(
                            "Cannot redo copy; destination directory already exists: {}",
                            existing.display()
                        )));
                    }
                    created_dirs.sort_by_key(|directory| directory.components().count());
                    let mut made_dirs = Vec::new();
                    for directory in &created_dirs {
                        if let Err(error) = std::fs::create_dir(directory) {
                            for made in made_dirs.iter().rev() {
                                let _ = std::fs::remove_dir(made);
                            }
                            return Err(CommandError::IoError(error.to_string()));
                        }
                        made_dirs.push(directory.clone());
                    }
                    if !files.is_empty() {
                        if let Err(error) = perform_copy_with_progress(
                            files,
                            total_bytes,
                            "redo_copy",
                            Arc::new(AtomicBool::new(false)),
                        ) {
                            for directory in made_dirs.iter().rev() {
                                let _ = std::fs::remove_dir(directory);
                            }
                            return Err(error);
                        }
                    }
                }
            }
            TransactionType::Move => {
                if tx.details.moved_files.is_none()
                    || tx.details.moved_dirs.is_none()
                    || tx.details.copied_file_hashes.is_none()
                {
                    return Err(CommandError::Other(
                        "This move predates the effect journal and cannot be redone safely"
                            .to_string(),
                    ));
                }
                if tx
                    .details
                    .replaced_files
                    .as_ref()
                    .is_some_and(|files| !files.is_empty())
                    || tx
                        .details
                        .overwritten_files
                        .as_ref()
                        .is_some_and(|files| !files.is_empty())
                {
                    return Err(CommandError::Other(
                        "Redo for replacement operations requires a new conflict resolution"
                            .to_string(),
                    ));
                }
                let moved = tx.details.moved_files.as_deref().unwrap_or_default();
                for (source, destination) in moved {
                    let source = PathBuf::from(source);
                    let destination = PathBuf::from(destination);
                    if !source.exists() {
                        return Err(CommandError::IoError(format!(
                            "Cannot redo move; source is missing: {}",
                            source.display()
                        )));
                    }
                    if destination.exists() {
                        return Err(CommandError::IoError(format!(
                            "Cannot redo move; destination already exists: {}",
                            destination.display()
                        )));
                    }
                    let expected_hash = tx
                        .details
                        .copied_file_hashes
                        .as_deref()
                        .unwrap_or_default()
                        .iter()
                        .find(|(path, _)| path == &destination.to_string_lossy())
                        .map(|(_, hash)| hash.as_str());
                    if let Some(expected_hash) = expected_hash {
                        if hash_file(&source)? != expected_hash {
                            return Err(CommandError::Other(format!(
                                "Cannot redo move because the source changed since the operation: {}",
                                source.display()
                            )));
                        }
                    }
                }
                let mut completed = Vec::new();
                let mut recreated_dirs: Vec<PathBuf> = tx
                    .details
                    .created_dirs
                    .as_deref()
                    .unwrap_or_default()
                    .iter()
                    .map(PathBuf::from)
                    .collect();
                if let Some(existing) = recreated_dirs.iter().find(|directory| directory.exists()) {
                    return Err(CommandError::IoError(format!(
                        "Cannot redo move; destination directory already exists: {}",
                        existing.display()
                    )));
                }
                recreated_dirs.sort_by_key(|directory| directory.components().count());
                let mut made_dirs = Vec::new();
                for directory in &recreated_dirs {
                    if let Err(error) = create_dir_all_tracked(directory, &mut made_dirs) {
                        let cleanup_errors = remove_empty_dirs(&made_dirs);
                        let detail = if cleanup_errors.is_empty() {
                            String::new()
                        } else {
                            format!("; rollback incomplete: {}", cleanup_errors.join(", "))
                        };
                        return Err(CommandError::IoError(format!("{error}{detail}")));
                    }
                    made_dirs.push(directory.clone());
                }
                for (source, destination) in moved {
                    let source = PathBuf::from(source);
                    let destination = PathBuf::from(destination);
                    if let Some(parent) = destination.parent() {
                        if !parent.exists() {
                            if let Err(error) = create_dir_all_tracked(parent, &mut made_dirs) {
                                let rollback_errors = rollback_renamed_paths(&completed);
                                let cleanup_errors = remove_empty_dirs(&made_dirs);
                                return Err(CommandError::IoError(format!(
                                    "{error}; rollback errors: {}{}",
                                    rollback_errors.join(", "),
                                    if cleanup_errors.is_empty() {
                                        String::new()
                                    } else {
                                        format!(
                                            "; directory cleanup: {}",
                                            cleanup_errors.join(", ")
                                        )
                                    }
                                )));
                            }
                        }
                    }
                    if let Err(error) = std::fs::rename(&source, &destination) {
                        let rollback_errors = rollback_renamed_paths(&completed);
                        let cleanup_errors = remove_empty_dirs(&made_dirs);
                        return Err(CommandError::IoError(format!(
                            "{error}; rollback errors: {}{}",
                            rollback_errors.join(", "),
                            if cleanup_errors.is_empty() {
                                String::new()
                            } else {
                                format!("; directory cleanup: {}", cleanup_errors.join(", "))
                            }
                        )));
                    }
                    completed.push((source, destination));
                }
                if let Some(directories) = &tx.details.moved_dirs {
                    let mut directories = directories.clone();
                    directories.sort_by_key(|(source, _)| {
                        std::cmp::Reverse(PathBuf::from(source).components().count())
                    });
                    let mut removed_sources = Vec::new();
                    for (source, destination) in directories {
                        if let Err(error) = create_dir_all_tracked(
                            std::path::Path::new(&destination),
                            &mut made_dirs,
                        ) {
                            let rollback_errors = rollback_renamed_paths(&completed);
                            let cleanup_errors = remove_empty_dirs(&made_dirs);
                            return Err(CommandError::IoError(format!(
                                "{error}; move rollback errors: {}; directory cleanup errors: {}",
                                rollback_errors.join(", "),
                                cleanup_errors.join(", ")
                            )));
                        }
                        match std::fs::remove_dir(&source) {
                            Ok(()) => removed_sources.push(PathBuf::from(source)),
                            Err(error)
                                if matches!(
                                    error.kind(),
                                    std::io::ErrorKind::DirectoryNotEmpty
                                        | std::io::ErrorKind::NotFound
                                ) => {}
                            Err(error) => {
                                for removed in removed_sources.iter().rev() {
                                    let _ = std::fs::create_dir_all(removed);
                                }
                                let rollback_errors = rollback_renamed_paths(&completed);
                                let cleanup_errors = remove_empty_dirs(&made_dirs);
                                return Err(CommandError::IoError(format!(
                                    "{error}; move rollback errors: {}; directory cleanup errors: {}",
                                    rollback_errors.join(", "),
                                    cleanup_errors.join(", ")
                                )));
                            }
                        }
                    }
                }
            }
            TransactionType::Rename => {
                if let (Some(old), Some(new)) =
                    (tx.details.old_path.as_ref(), tx.details.new_path.as_ref())
                {
                    let old_pb = PathBuf::from(old);
                    let new_pb = PathBuf::from(new);
                    if old_pb.exists() {
                        std::fs::rename(old_pb, new_pb)
                            .map_err(|e| CommandError::IoError(e.to_string()))?;
                    }
                }
            }
            TransactionType::NewFolder => {
                for path_str in &tx.details.paths {
                    let path = PathBuf::from(path_str);
                    if !path.exists() {
                        std::fs::create_dir_all(path)
                            .map_err(|e| CommandError::IoError(e.to_string()))?;
                    }
                }
            }
        }
        if !history.complete_redo(tx) {
            return Err(CommandError::Other(
                "Redo history changed while the operation was running".to_string(),
            ));
        }
    }
    Ok(transaction)
}

// Helper to perform copy with progress
fn perform_copy_with_progress(
    files: Vec<(PathBuf, PathBuf)>,
    _total_bytes: u64,
    _task_name: &str,
    cancel_flag: Arc<AtomicBool>,
) -> Result<(), CommandError> {
    use std::fs;
    use std::io::{Read, Write};

    struct PendingFile(Option<PathBuf>);
    impl Drop for PendingFile {
        fn drop(&mut self) {
            if let Some(path) = self.0.take() {
                let _ = fs::remove_file(path);
            }
        }
    }

    let mut published = Vec::new();
    for (source, dest) in &files {
        if cancel_flag.load(Ordering::Relaxed) {
            for path in published.iter().rev() {
                let _ = fs::remove_file(path);
            }
            return Err(CommandError::Other("Redo copy was cancelled".to_string()));
        }

        if source.is_dir() {
            continue;
        }

        if dest.exists() {
            for path in published.iter().rev() {
                let _ = fs::remove_file(path);
            }
            return Err(CommandError::IoError(format!(
                "Cannot redo copy; destination already exists: {}",
                dest.display()
            )));
        }

        let result = (|| {
            if let Some(parent) = dest.parent() {
                if !parent.exists() {
                    fs::create_dir_all(parent).map_err(|e| CommandError::IoError(e.to_string()))?;
                }
            }
            let mut file_in =
                fs::File::open(source).map_err(|e| CommandError::IoError(e.to_string()))?;
            let mut temporary = None;
            let mut file_out = None;
            for _ in 0..8 {
                let candidate =
                    dest.with_extension(format!("oxyde-redo-{}.tmp", uuid::Uuid::new_v4()));
                match fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&candidate)
                {
                    Ok(file) => {
                        temporary = Some(PendingFile(Some(candidate)));
                        file_out = Some(file);
                        break;
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                    Err(error) => return Err(CommandError::IoError(error.to_string())),
                }
            }
            let mut temporary = temporary.ok_or_else(|| {
                CommandError::IoError("Could not allocate a temporary redo file".to_string())
            })?;
            let mut file_out = file_out.take().expect("temporary file handle is allocated");
            let mut buffer = [0u8; 81920];
            loop {
                if cancel_flag.load(Ordering::Relaxed) {
                    return Err(CommandError::Other("Redo copy was cancelled".to_string()));
                }
                let count = file_in
                    .read(&mut buffer)
                    .map_err(|e| CommandError::IoError(e.to_string()))?;
                if count == 0 {
                    break;
                }
                file_out
                    .write_all(&buffer[..count])
                    .map_err(|e| CommandError::IoError(e.to_string()))?;
            }
            file_out
                .sync_all()
                .map_err(|e| CommandError::IoError(e.to_string()))?;
            drop(file_out);
            if dest.exists() {
                return Err(CommandError::IoError(format!(
                    "Cannot redo copy; destination already exists: {}",
                    dest.display()
                )));
            }
            fs::rename(
                temporary.0.as_ref().expect("temporary path is allocated"),
                dest,
            )
            .map_err(|e| CommandError::IoError(e.to_string()))?;
            temporary.0 = None;
            Ok(())
        })();
        if let Err(error) = result {
            for path in published.iter().rev() {
                let _ = fs::remove_file(path);
            }
            return Err(error);
        }
        published.push(dest.clone());
    }

    Ok(())
}

#[cfg(test)]
mod copy_redo_tests {
    use super::{perform_copy_with_progress, undo_last_action};
    use crate::models::transaction::{Transaction, TransactionDetails, TransactionType};
    use crate::models::HistoryManager;
    use std::sync::atomic::AtomicBool;
    use std::sync::Arc;

    #[test]
    fn redo_copy_rolls_back_new_files_when_a_later_destination_conflicts() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("source");
        let first_source = source.join("first.txt");
        let second_source = source.join("second.txt");
        let destination = temp.path().join("destination");
        let first_destination = destination.join("first.txt");
        let second_destination = destination.join("second.txt");
        std::fs::create_dir(&source).unwrap();
        std::fs::create_dir(&destination).unwrap();
        std::fs::write(&first_source, b"new file").unwrap();
        std::fs::write(&second_source, b"source version").unwrap();
        std::fs::write(&second_destination, b"existing version").unwrap();

        let result = perform_copy_with_progress(
            vec![
                (first_source, first_destination.clone()),
                (second_source, second_destination.clone()),
            ],
            0,
            "test",
            Arc::new(AtomicBool::new(false)),
        );

        assert!(result.is_err());
        assert!(!first_destination.exists());
        assert_eq!(
            std::fs::read(second_destination).unwrap(),
            b"existing version"
        );
    }

    #[test]
    fn undo_copy_removes_its_verified_output_without_shell_trash() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("source.txt");
        let destination = temp.path().join("destination.txt");
        std::fs::write(&source, b"copied content").unwrap();
        std::fs::write(&destination, b"copied content").unwrap();
        let hash = blake3::hash(b"copied content").to_hex().to_string();
        let history = HistoryManager::new();
        history.push(Transaction::new(
            TransactionType::Copy,
            TransactionDetails {
                paths: vec![source.to_string_lossy().into_owned()],
                target_dir: Some(temp.path().to_string_lossy().into_owned()),
                old_path: None,
                new_path: None,
                created_files: Some(vec![destination.to_string_lossy().into_owned()]),
                created_dirs: Some(Vec::new()),
                replaced_files: Some(Vec::new()),
                overwritten_files: Some(Vec::new()),
                moved_files: None,
                moved_dirs: None,
                copied_files: Some(vec![(
                    source.to_string_lossy().into_owned(),
                    destination.to_string_lossy().into_owned(),
                )]),
                copied_file_hashes: Some(vec![(destination.to_string_lossy().into_owned(), hash)]),
            },
        ));
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();

        runtime.block_on(undo_last_action(&history)).unwrap();

        assert!(!destination.exists());
        assert!(history.get_state().undo_stack.is_empty());
        assert_eq!(history.get_state().redo_stack.len(), 1);
    }

    #[test]
    fn undo_replacement_restores_the_hidden_backup() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("source.txt");
        let destination = temp.path().join("destination.txt");
        let backup = temp.path().join(".oxyde-previous.bak");
        std::fs::write(&source, b"new content").unwrap();
        std::fs::write(&destination, b"new content").unwrap();
        std::fs::write(&backup, b"old content").unwrap();
        let hash = blake3::hash(b"new content").to_hex().to_string();
        let history = HistoryManager::new();
        history.push(Transaction::new(
            TransactionType::Copy,
            TransactionDetails {
                paths: vec![source.to_string_lossy().into_owned()],
                target_dir: Some(temp.path().to_string_lossy().into_owned()),
                old_path: None,
                new_path: None,
                created_files: Some(Vec::new()),
                created_dirs: Some(Vec::new()),
                replaced_files: Some(vec![(
                    destination.to_string_lossy().into_owned(),
                    backup.to_string_lossy().into_owned(),
                )]),
                overwritten_files: Some(Vec::new()),
                moved_files: None,
                moved_dirs: None,
                copied_files: Some(vec![(
                    source.to_string_lossy().into_owned(),
                    destination.to_string_lossy().into_owned(),
                )]),
                copied_file_hashes: Some(vec![(destination.to_string_lossy().into_owned(), hash)]),
            },
        ));
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();

        runtime.block_on(undo_last_action(&history)).unwrap();

        assert_eq!(std::fs::read(destination).unwrap(), b"old content");
        assert!(!backup.exists());
    }
}
