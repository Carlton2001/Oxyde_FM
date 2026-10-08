use crate::models::{
    get_file_entry_from_path, CommandError, ConflictEntry, HistoryManager,
    Transaction, TransactionType, TrashEntry,
};
use crate::systems::history_move::move_file as move_history_file;
use crate::utils::path_security::validate_path;
use log::{info, warn};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
fn rollback_moved_paths(completed: &[(PathBuf, PathBuf)]) -> Vec<String> {
    let mut errors = Vec::new();
    for (source, destination) in completed.iter().rev() {
        if let Err(error) = move_history_file(destination, source, None) {
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
    let mut current = path;
    while !current.exists() {
        missing.push(current.to_path_buf());
        current = current.parent().ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "The filesystem root is unavailable",
            )
        })?;
    }
    missing.reverse();
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

fn get_restoration_paths(item: &trash::TrashItem) -> RestorationPaths {
    let crate_path = crate::platform::platform()
        .trash_original_path(std::path::Path::new(&item.id))
        .unwrap_or_else(|| item.original_path());
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

/// Shares the original identity used by trash listings, restore, and properties.
pub(crate) fn original_trash_path(item: &trash::TrashItem) -> PathBuf {
    get_restoration_paths(item).corrected_path
}

fn item_with_correct_restore_name(item: &trash::TrashItem) -> trash::TrashItem {
    let mut item = item.clone();
    let original_path = original_trash_path(&item);
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
        crate::platform::platform().list_trash_items().map_err(CommandError::TrashError)?;

    let mut entries: Vec<TrashEntry> = trash_items
        .into_iter()
        .map(|item| {
            let original_path = original_trash_path(&item);
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

    crate::platform::platform()
        .empty_trash()
        .map_err(CommandError::TrashError)?;

    Ok(())
}

/// Permanently delete specific items from the Recycle Bin
pub async fn purge_recycle_bin(paths: Vec<String>) -> Result<(), CommandError> {
    let trash_items = crate::platform::platform()
        .list_trash_items()
        .map_err(CommandError::TrashError)?;

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
        crate::platform::platform()
            .purge_trash_items(items_to_purge)
            .map_err(CommandError::TrashError)?;
    }

    Ok(())
}

pub async fn check_conflicts(
    paths: Vec<String>,
    target_dir: String,
) -> Result<Vec<ConflictEntry>, CommandError> {
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
        for conflict in &mut response {
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
) -> Result<Vec<ConflictEntry>, CommandError> {
    let mut conflicts = Vec::new();
    let target_base = validate_path(&target_dir)?;

    let sources: Vec<_> = paths.iter().map(PathBuf::from).collect();
    let trash_entries = crate::platform::platform()
        .trash_transfer_entries(&sources)
        .map_err(CommandError::TrashError)?;
    for path_str in paths {
        let source_path = validate_path(&path_str)?;

        // Conflict check
        let display_path = trash_entries
            .iter()
            .find(|entry| entry.path == source_path)
            .map(|entry| entry.original_path.as_path())
            .unwrap_or(&source_path);
        if let Some(file_name) = display_path.file_name() {
            let target_path = target_base.join(file_name);
            if target_path.exists() {
                // Initial top-level conflict found
                add_conflict_recursive(&source_path, &target_path, &mut conflicts)?;
            }
        }
    }

    Ok(conflicts)
}

fn add_conflict_recursive(
    source_path: &std::path::Path,
    target_path: &std::path::Path,
    conflicts: &mut Vec<ConflictEntry>,
) -> Result<(), CommandError> {
    let mut source_entry = get_file_entry_from_path(source_path)?;
    let mut target_entry = get_file_entry_from_path(target_path)?;
    source_entry.name = target_entry.name.clone();

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
            name: target_path
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
            name: target_path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default(),
            source: source_entry,
            target: target_entry,
        });
    }

    Ok(())
}

#[derive(Debug, Clone)]
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
        crate::platform::platform().list_trash_items().map_err(CommandError::TrashError)?;

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
        let original_path = original_trash_path(&item);
        let original_path_str = original_path.to_string_lossy().to_string();

        crate::platform::platform().restore_trash_items(vec![item.clone()])
            .map_err(CommandError::TrashError)?;
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

/// Replay the same system deletion policy used by the operation queue.
/// Recycle Bin capacity is decided by the OS, without a fixed size or item limit.
fn recycle_history_paths(paths: Vec<PathBuf>) -> Result<(), CommandError> {
    if paths.is_empty() {
        return Ok(());
    }
    crate::platform::platform()
        .shell_delete(&paths, true)
        .map_err(CommandError::TrashError)
}

pub async fn undo_last_action(
    history: &HistoryManager,
) -> Result<Option<Transaction>, CommandError> {
    let Some(operation) = history
        .begin_undo()
        .map_err(|error| CommandError::Other(error.to_string()))?
    else {
        return Ok(None);
    };
    let transaction = Some(operation.transaction().clone());

    if let Some(ref tx) = transaction {
        info!("Undoing transaction: {:?}", tx.op_type);
        if tx.undo_blocked_by_overwrite() {
            return Err(CommandError::Other(
                "This copy overwrote existing files and cannot be undone".to_string(),
            ));
        }
        match tx.op_type {
            TransactionType::Copy => {
                crate::systems::history_file_undo::undo_copy(&tx.details)?;
            }
            TransactionType::Move => {
                crate::systems::history_file_undo::undo(&tx.details)?;
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
                    recycle_history_paths(files_to_delete)?;
                }
            }
        }
        operation.complete();
    }
    Ok(transaction)
}

pub async fn redo_last_action(
    history: &HistoryManager,
) -> Result<Option<Transaction>, CommandError> {
    let Some(operation) = history
        .begin_redo()
        .map_err(|error| CommandError::Other(error.to_string()))?
    else {
        return Ok(None);
    };
    let transaction = Some(operation.transaction().clone());

    if let Some(ref tx) = transaction {
        info!("Redoing transaction: {:?}", tx.op_type);
        match tx.op_type {
            TransactionType::Delete => {
                let paths: Vec<PathBuf> = tx.details.paths.iter().map(PathBuf::from).collect();
                recycle_history_paths(paths)?;
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
                    for (source, _) in &files {
                        std::fs::metadata(source)
                            .map_err(|error| CommandError::IoError(error.to_string()))?;
                    }
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
                                let rollback_errors = rollback_moved_paths(&completed);
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
                    let expected = tx.details.copied_file_hashes
                        .as_deref()
                        .unwrap_or_default()
                        .iter()
                        .find(|(path, _)| path == &destination.to_string_lossy())
                        .map(|(_, hash)| hash.as_str());
                    if let Err(error) = move_history_file(&source, &destination, expected) {
                        let rollback_errors = rollback_moved_paths(&completed);
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
                            let rollback_errors = rollback_moved_paths(&completed);
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
                                let rollback_errors = rollback_moved_paths(&completed);
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
        operation.complete();
    }
    Ok(transaction)
}

// Copies files with cancellation and rollback on failure.
fn perform_copy_with_progress(
    files: Vec<(PathBuf, PathBuf)>,
    cancel_flag: Arc<AtomicBool>,
) -> Result<(), CommandError> {
    let mut published: Vec<(PathBuf, String)> = Vec::new();
    for (source, destination) in files {
        let result = (|| {
            if let Some(parent) = destination.parent() {
                std::fs::create_dir_all(parent)?;
            }
            crate::systems::file_copy::copy(&source, &destination, false, &mut |_| {
                if cancel_flag.load(Ordering::Relaxed) {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::Interrupted,
                        "Redo copy was cancelled",
                    ));
                }
                Ok(())
            })
        })();
        match result {
            Ok(hash) => published.push((destination, hash)),
            Err(error) => {
                let rollback_errors: Vec<_> = published
                    .iter()
                    .rev()
                    .filter_map(|(path, hash)| {
                        crate::systems::file_copy::remove_verified_output(path, hash)
                            .err()
                            .map(|error| format!("{}: {error}", path.display()))
                    })
                    .collect();
                return Err(CommandError::IoError(if rollback_errors.is_empty() {
                    error.to_string()
                } else {
                    format!(
                        "{error}; redo copy rollback incomplete: {}",
                        rollback_errors.join("; ")
                    )
                }));
            }
        }
    }
    Ok(())
}
#[cfg(test)]
#[cfg(target_os = "windows")]
mod trash_restore_tests {
    use super::*;

    #[test]
    #[ignore = "purges only one generated fixture and restores its generated sibling"]
    fn native_purge_only_removes_selected_trash_payload() {
        let root = tempfile::tempdir().unwrap();
        let selected = root.path().join("Supprimer école.txt");
        let sibling = root.path().join("Garder école.txt");
        let ordinary = root.path().join("Hors corbeille.txt");
        for path in [&selected, &sibling, &ordinary] {
            std::fs::write(path, b"purge fixture").unwrap();
        }
        crate::platform::platform()
            .shell_delete(&[selected.clone(), sibling.clone()], true)
            .unwrap();
        let entries = list_trash().unwrap();
        let selected_entry = entries
            .iter()
            .find(|entry| PathBuf::from(&entry.original_path) == selected)
            .unwrap();
        let sibling_entry = entries
            .iter()
            .find(|entry| PathBuf::from(&entry.original_path) == sibling)
            .unwrap();
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let purged = runtime.block_on(purge_recycle_bin(vec![
            selected_entry.path.clone(),
            ordinary.to_string_lossy().into_owned(),
        ]));
        let selected_still_present = PathBuf::from(&selected_entry.path).exists();
        let sibling_still_present = PathBuf::from(&sibling_entry.path).exists();
        // Restore generated survivors before assertions, without touching user entries.
        let remaining: Vec<_> = [selected_entry, sibling_entry]
            .into_iter()
            .filter(|entry| PathBuf::from(&entry.path).exists())
            .map(|entry| entry.path.clone())
            .collect();
        let restored = runtime.block_on(restore_items(remaining));
        purged.unwrap();
        restored.unwrap();
        assert!(!selected_still_present);
        assert!(sibling_still_present);
        assert!(!selected.exists());
        assert_eq!(std::fs::read(sibling).unwrap(), b"purge fixture");
        assert_eq!(std::fs::read(ordinary).unwrap(), b"purge fixture");
    }

    #[test]
    #[ignore = "uses only a generated fixture to check a native Recycle Bin restore conflict"]
    fn native_restore_conflict_preserves_existing_file_and_trash_item() {
        let root = tempfile::tempdir().unwrap();
        let file = root.path().join("Conflit école.txt");
        std::fs::write(&file, b"original fixture").unwrap();
        crate::platform::platform()
            .shell_delete(std::slice::from_ref(&file), true)
            .unwrap();
        let entry = list_trash()
            .unwrap()
            .into_iter()
            .find(|entry| PathBuf::from(&entry.original_path) == file)
            .unwrap();
        std::fs::write(&file, b"existing fixture").unwrap();
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let result = runtime.block_on(restore_items(vec![entry.path.clone()]));
        let existing = std::fs::read(&file).unwrap();
        let still_in_trash = list_trash()
            .unwrap()
            .iter()
            .any(|item| item.path == entry.path);

        // Remove only the generated blocking file, then restore the fixture
        // before assertions so an assertion failure does not leave it in trash.
        let restored = if still_in_trash {
            std::fs::remove_file(&file).unwrap();
            runtime.block_on(restore_items(vec![entry.path]))
        } else {
            Ok(Vec::new())
        };
        assert!(result.is_err(), "a restore conflict must be reported");
        assert_eq!(existing, b"existing fixture");
        assert!(still_in_trash, "the original fixture must remain restorable");
        assert_eq!(restored.unwrap(), vec![file.to_string_lossy().to_string()]);
        assert_eq!(std::fs::read(&file).unwrap(), b"original fixture");
    }

    #[test]
    #[ignore = "sends only generated fixtures to the native Windows Recycle Bin and restores them"]
    fn native_restore_preserves_original_names() {
        let root = tempfile::tempdir().unwrap();
        let files = [
            root.path().join("Document école – version 2.txt"),
            root.path().join("Raccourci école.lnk"),
            root.path().join("Lien école.url"),
        ];
        for file in &files {
            std::fs::write(file, b"restoration fixture").unwrap();
        }
        let folder = root.path().join("Dossier école");
        std::fs::create_dir(&folder).unwrap();
        std::fs::write(folder.join("contenu.txt"), b"nested fixture").unwrap();
        let sources: Vec<_> = files.iter().cloned().chain([folder.clone()]).collect();
        crate::platform::platform()
            .shell_delete(&sources, true)
            .unwrap();
        let entries: Vec<_> = list_trash()
            .unwrap()
            .into_iter()
            .filter(|entry| {
                // Identify only this test's fixtures, even if a metadata path has a
                // trailing terminator. Validate the unmodified names after restore.
                PathBuf::from(entry.original_path.trim_end_matches('\0')).parent()
                    == Some(root.path())
            })
            .collect();
        let ids = entries.iter().map(|entry| entry.path.clone()).collect();
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let restored = runtime.block_on(restore_items(ids)).unwrap();

        for file in &files {
            assert_eq!(std::fs::read(file).unwrap(), b"restoration fixture");
        }
        assert_eq!(
            std::fs::read(folder.join("contenu.txt")).unwrap(),
            b"nested fixture"
        );
        assert_eq!(entries.len(), sources.len());
        for entry in &entries {
            let expected = sources
                .iter()
                .find(|source| {
                    **source == PathBuf::from(entry.original_path.trim_end_matches('\0'))
                })
                .unwrap();
            assert_eq!(entry.name, expected.file_name().unwrap().to_string_lossy());
            assert_eq!(PathBuf::from(&entry.original_path), *expected);
        }
        let mut restored: Vec<_> = restored.into_iter().map(PathBuf::from).collect();
        let mut expected = sources;
        restored.sort();
        expected.sort();
        assert_eq!(restored, expected);
        assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 4);
    }
}

#[cfg(test)]
mod copy_redo_tests {
    use super::{perform_copy_with_progress, undo_last_action};
    use crate::models::transaction::{Transaction, TransactionDetails, TransactionType};
    use crate::models::HistoryManager;
    use std::sync::atomic::AtomicBool;
    use std::sync::Arc;

    #[test]
    fn undo_copy_directory_error_preserves_outputs_and_allows_retry() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source.txt");
        let destination = root.path().join("copy.txt");
        let blocker = root.path().join("directory");
        std::fs::write(&source, b"copied content").unwrap();
        std::fs::write(&destination, b"copied content").unwrap();
        std::fs::write(&blocker, b"foreign content").unwrap();
        let name = |path: &std::path::Path| path.to_string_lossy().into_owned();
        let history = HistoryManager::new();
        history.push(Transaction::new(
            TransactionType::Copy,
            TransactionDetails {
                paths: vec![name(&source)],
                target_dir: None,
                old_path: None,
                new_path: None,
                created_files: Some(vec![name(&destination)]),
                created_dirs: Some(vec![name(&blocker)]),
                overwritten_files: Some(vec![]),
                moved_files: None,
                trash_metadata_backups: None,
                moved_dirs: None,
                copied_files: Some(vec![(name(&source), name(&destination))]),
                copied_file_hashes: Some(vec![(
                    name(&destination),
                    blake3::hash(b"copied content").to_hex().to_string(),
                )]),
            },
        ));
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        assert!(runtime.block_on(undo_last_action(&history)).is_err());
        assert_eq!(std::fs::read(&destination).unwrap(), b"copied content");
        assert_eq!(std::fs::read(&blocker).unwrap(), b"foreign content");
        assert_eq!(history.get_state().undo_stack.len(), 1);
        assert!(history.get_state().redo_stack.is_empty());
        std::fs::remove_file(blocker).unwrap();
        runtime.block_on(undo_last_action(&history)).unwrap();
        assert!(!destination.exists());
        assert_eq!(std::fs::read(source).unwrap(), b"copied content");
        assert_eq!(history.get_state().redo_stack.len(), 1);
    }

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
                overwritten_files: Some(Vec::new()),
                moved_files: None,
                trash_metadata_backups: None,
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
}

#[cfg(test)]
mod history_concurrency_tests {
    use super::{redo_last_action, undo_last_action};
    use crate::models::transaction::{Transaction, TransactionDetails, TransactionType};
    use crate::models::HistoryManager;

    fn rename_transaction(old: &std::path::Path, new: &std::path::Path) -> Transaction {
        Transaction::new(
            TransactionType::Rename,
            TransactionDetails {
                paths: vec![],
                target_dir: None,
                old_path: Some(old.to_string_lossy().into_owned()),
                new_path: Some(new.to_string_lossy().into_owned()),
                created_files: None,
                created_dirs: None,
                overwritten_files: None,
                moved_files: None,
                trash_metadata_backups: None,
                moved_dirs: None,
                copied_files: None,
                copied_file_hashes: None,
            },
        )
    }

    #[test]
    fn concurrent_commands_do_not_touch_files_and_can_retry_after_release() {
        let temp = tempfile::tempdir().unwrap();
        let old = temp.path().join("old.txt");
        let new = temp.path().join("new.txt");
        std::fs::write(&new, b"fixture content").unwrap();
        let history = HistoryManager::new();
        let tx = rename_transaction(&old, &new);
        history.push(tx.clone());
        let selected = history.begin_undo().unwrap().unwrap();
        std::thread::scope(|scope| {
            scope
                .spawn(|| {
                    let runtime = tokio::runtime::Builder::new_current_thread()
                        .build()
                        .unwrap();
                    assert!(runtime.block_on(undo_last_action(&history)).is_err());
                    assert!(runtime.block_on(redo_last_action(&history)).is_err());
                    assert!(!old.exists());
                    assert_eq!(std::fs::read(&new).unwrap(), b"fixture content");
                })
                .join()
                .unwrap();
        });
        drop(selected);
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        assert_eq!(
            runtime
                .block_on(undo_last_action(&history))
                .unwrap()
                .unwrap()
                .id,
            tx.id
        );
        assert!(!new.exists());
        assert_eq!(std::fs::read(&old).unwrap(), b"fixture content");
        let selected = history.begin_redo().unwrap().unwrap();
        assert!(runtime.block_on(redo_last_action(&history)).is_err());
        assert!(runtime.block_on(undo_last_action(&history)).is_err());
        assert!(old.exists());
        assert!(!new.exists());
        drop(selected);
        assert_eq!(
            runtime
                .block_on(redo_last_action(&history))
                .unwrap()
                .unwrap()
                .id,
            tx.id
        );
        assert!(!old.exists());
        assert_eq!(std::fs::read(&new).unwrap(), b"fixture content");
        assert_eq!(history.get_state().undo_stack[0].id, tx.id);
        assert!(history.get_state().redo_stack.is_empty());
    }

    #[test]
    fn io_error_keeps_history_and_allows_another_attempt() {
        let temp = tempfile::tempdir().unwrap();
        let old = temp.path().join("missing-parent").join("old.txt");
        let new = temp.path().join("new.txt");
        std::fs::write(&new, b"fixture content").unwrap();
        let history = HistoryManager::new();
        let tx = rename_transaction(&old, &new);
        history.push(tx.clone());
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        assert!(runtime.block_on(undo_last_action(&history)).is_err());
        assert_eq!(history.get_state().undo_stack[0].id, tx.id);
        assert!(history.get_state().redo_stack.is_empty());
        assert_eq!(std::fs::read(&new).unwrap(), b"fixture content");
        std::fs::create_dir(old.parent().unwrap()).unwrap();
        assert!(runtime
            .block_on(undo_last_action(&history))
            .unwrap()
            .is_some());
        assert_eq!(std::fs::read(&old).unwrap(), b"fixture content");
        assert!(history.get_state().undo_stack.is_empty());
        assert_eq!(history.get_state().redo_stack[0].id, tx.id);
    }
}

#[cfg(test)]
mod tracked_directory_tests {
    #[test]
    fn recreates_nested_directories_below_canonical_root_without_tracking_drive_prefix() {
        let root = tempfile::tempdir().unwrap();
        let canonical = std::fs::canonicalize(root.path()).unwrap();
        let target = canonical.join("one/two");
        let mut created = Vec::new();
        super::create_dir_all_tracked(&target, &mut created).unwrap();
        assert_eq!(created, vec![canonical.join("one"), target.clone()]);
        assert!(target.is_dir());
        super::create_dir_all_tracked(&target, &mut created).unwrap();
        assert_eq!(created.len(), 2);
    }
}

#[cfg(test)]
mod move_undo_rollback_tests {
    use super::*;
    use crate::models::TransactionDetails;

    fn details(source: &std::path::Path, destination: &std::path::Path) -> TransactionDetails {
        TransactionDetails {
            paths: vec![],
            target_dir: None,
            old_path: None,
            new_path: None,
            created_files: Some(vec![]),
            created_dirs: Some(vec![]),
            overwritten_files: Some(vec![]),
            moved_files: Some(vec![(
                source.to_string_lossy().into_owned(),
                destination.to_string_lossy().into_owned(),
            )]),
            trash_metadata_backups: None,
            moved_dirs: Some(vec![]),
            copied_files: Some(vec![]),
            copied_file_hashes: Some(vec![(
                destination.to_string_lossy().into_owned(),
                blake3::hash(b"moving file").to_hex().to_string(),
            )]),
        }
    }

    fn check_directory_failure(child_of_blocker: bool) {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source/file.txt");
        let destination = root.path().join("destination.txt");
        std::fs::write(&destination, b"moving file").unwrap();
        let blocker = root.path().join("directory-blocker");
        std::fs::write(&blocker, b"external file").unwrap();
        let mut journal = details(&source, &destination);
        let directory = if child_of_blocker {
            blocker.join("child")
        } else {
            blocker.clone()
        };
        journal.moved_dirs = Some(vec![(
            directory.to_string_lossy().into_owned(),
            root.path()
                .join("old-directory")
                .to_string_lossy()
                .into_owned(),
        )]);
        let history = HistoryManager::new();
        history.push(Transaction::new(TransactionType::Move, journal));
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        assert!(runtime.block_on(undo_last_action(&history)).is_err());
        assert_eq!(std::fs::read(&destination).unwrap(), b"moving file");
        assert!(!source.exists());
        assert_eq!(std::fs::read(&blocker).unwrap(), b"external file");
        assert_eq!(history.get_state().undo_stack.len(), 1);
        assert!(history.get_state().redo_stack.is_empty());
        std::fs::remove_file(blocker).unwrap();
        runtime.block_on(undo_last_action(&history)).unwrap();
        assert_eq!(std::fs::read(source).unwrap(), b"moving file");
        assert!(!destination.exists());
        assert!(history.get_state().undo_stack.is_empty());
    }

    #[test]
    fn directory_failure_keeps_files_and_history_for_retry() {
        check_directory_failure(true);
    }

    #[test]
    fn source_directory_cannot_be_replaced_by_a_file() {
        check_directory_failure(false);
    }

    #[test]
    fn missing_destination_is_refused_before_history_changes() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source.txt");
        let destination = root.path().join("missing.txt");
        let history = HistoryManager::new();
        history.push(Transaction::new(
            TransactionType::Move,
            details(&source, &destination),
        ));
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        assert!(runtime.block_on(undo_last_action(&history)).is_err());
        assert_eq!(history.get_state().undo_stack.len(), 1);
        assert!(history.get_state().redo_stack.is_empty());
        assert!(!source.exists());
    }
}
