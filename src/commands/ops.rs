use crate::models::{
    get_file_entry_from_path, CommandError, ConflictEntry, HistoryManager,
    Transaction, TransactionType, TrashEntry,
};
use crate::utils::path_security::validate_path;
use log::{info, warn};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
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
                    crate::systems::item_rename::rename(&new_pb, &old_pb)?;
                }
            }
            TransactionType::NewFolder => {
                crate::systems::new_folder_history::undo(&tx.details)?;
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
    let mut transaction = Some(operation.transaction().clone());

    if let Some(ref mut tx) = transaction {
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
                    let files: Vec<(PathBuf, PathBuf, String)> = files
                        .iter()
                        .map(|(source, destination)| {
                            let hash = tx
                                .details
                                .copied_file_hashes
                                .as_deref()
                                .unwrap_or_default()
                                .iter()
                                .find(|(path, _)| path == destination)
                                .map(|(_, hash)| hash.clone())
                                .ok_or_else(|| {
                                    CommandError::HistoryCopy(
                                        crate::models::error::HistoryCopyFailure {
                                            issue:
                                                crate::models::error::HistoryCopyIssue::Unverified,
                                            path: source.clone(),
                                        },
                                    )
                                })?;
                            Ok((PathBuf::from(source), PathBuf::from(destination), hash))
                        })
                        .collect::<Result<_, CommandError>>()?;
                    for (source, destination, _) in &files {
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
                    }
                    for (source, _, _) in &files {
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
                    let mut directories =
                        crate::systems::history_directories::Directories::prepare(
                            &created_dirs,
                            &created_dirs,
                        )?;
                    let mut identities = Vec::new();
                    for directory in &created_dirs {
                        match directories.create(directory, false) {
                            Ok(file) => {
                                made_dirs.push(directory.clone());
                                match crate::models::transaction::HistoryDirectoryIdentity::from_file(file) {
                                    Ok(identity) => identities.push((directory.to_string_lossy().into_owned(), identity)),
                                    Err(error) => log::warn!("Redo directory identity unavailable for {}: {error}", directory.display()),
                                }
                            }
                            Err(error) => {
                                return Err(rollback_copy_directories(
                                    &mut directories,
                                    &made_dirs,
                                    CommandError::from(error),
                                ));
                            }
                        }
                    }
                    tx.details.copied_directory_identities = Some(identities);
                    if !files.is_empty() {
                        match perform_copy_with_progress(files, Arc::new(AtomicBool::new(false))) {
                            Ok(identities) => tx.details.copied_file_identities = Some(identities),
                            Err(error) => {
                                return Err(rollback_copy_directories(
                                    &mut directories,
                                    &made_dirs,
                                    error,
                                ));
                            }
                        }
                    }
                }
            }
            TransactionType::Move => {
                crate::systems::history_move_redo::redo(&tx.details)?;
            }
            TransactionType::Rename => {
                if let (Some(old), Some(new)) =
                    (tx.details.old_path.as_ref(), tx.details.new_path.as_ref())
                {
                    let old_pb = PathBuf::from(old);
                    let new_pb = PathBuf::from(new);
                    crate::systems::item_rename::rename(&old_pb, &new_pb)?;
                }
            }
            TransactionType::NewFolder => {
                crate::systems::new_folder_history::redo(&mut tx.details)?;
            }
        }
        operation.complete_with_details(tx.details.clone());
    }
    Ok(transaction)
}

// Clean only retained directories of a failed Redo; report preserved recovery.
fn rollback_copy_directories(
    directories: &mut crate::systems::history_directories::Directories,
    created: &[PathBuf],
    error: CommandError,
) -> CommandError {
    let errors: Vec<_> = created
        .iter()
        .rev()
        .filter_map(|path| match directories.remove(path) {
            Ok(crate::systems::history_directories::Removal::NotEmpty) => Some(format!(
                "{}: recovery content was preserved",
                path.display()
            )),
            Ok(_) => None,
            Err(error) => Some(format!("{}: {error}", path.display())),
        })
        .collect();
    if errors.is_empty() {
        error
    } else {
        CommandError::IoError(format!(
            "{error}; rollback incomplete: {}",
            errors.join("; ")
        ))
    }
}

// Copies files with cancellation and rollback on failure.
fn perform_copy_with_progress(
    files: Vec<(PathBuf, PathBuf, String)>,
    cancel_flag: Arc<AtomicBool>,
) -> Result<Vec<(String, crate::models::transaction::HistoryFileIdentity)>, CommandError> {
    let mut published: Vec<(
        PathBuf,
        String,
        Option<crate::models::transaction::HistoryFileIdentity>,
    )> = Vec::new();
    let mut identities = Vec::new();
    for (source, destination, expected) in files {
        let result = (|| {
            if let Some(parent) = destination.parent() {
                std::fs::create_dir_all(parent)?;
            }
            crate::systems::file_copy::copy_verified(&source, &destination, &expected, &mut |_| {
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
            Ok((hash, identity)) => {
                if let Some(identity) = identity {
                    identities.push((destination.to_string_lossy().into_owned(), identity));
                }
                published.push((destination, hash, identity));
            }
            Err(error) => {
                let rollback_errors: Vec<_> = published
                    .iter()
                    .rev()
                    .filter_map(|(path, hash, identity)| {
                        crate::systems::file_copy::remove_verified_output_identity(
                            path, hash, *identity,
                        )
                        .err()
                        .map(|error| format!("{}: {error}", path.display()))
                    })
                    .collect();
                return Err(if rollback_errors.is_empty() {
                    CommandError::from(error)
                } else {
                    CommandError::IoError(format!(
                        "{error}; redo copy rollback incomplete: {}",
                        rollback_errors.join("; ")
                    ))
                });
            }
        }
    }
    Ok(identities)
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
    use super::{perform_copy_with_progress, redo_last_action, undo_last_action};
    use crate::models::HistoryManager;
    use crate::models::transaction::{Transaction, TransactionDetails, TransactionType};
    use std::sync::Arc;
    use std::sync::atomic::AtomicBool;

    #[test]
    fn redo_copy_directory_cleanup_preserves_a_late_empty_replacement() {
        let root = tempfile::tempdir().unwrap();
        let selected = root.path().join("created-directory");
        let displaced = root.path().join("original-directory");
        let mut directories = crate::systems::history_directories::Directories::prepare(
            std::slice::from_ref(&selected),
            std::slice::from_ref(&selected),
        )
        .unwrap();
        directories.create(&selected, false).unwrap();
        std::fs::rename(&selected, &displaced).unwrap();
        std::fs::create_dir(&selected).unwrap();
        let error = super::rollback_copy_directories(
            &mut directories,
            std::slice::from_ref(&selected),
            crate::models::CommandError::Other("injected copy failure".into()),
        );
        assert!(error.to_string().contains("rollback incomplete"), "{error}");
        assert!(
            error.to_string().contains(selected.to_str().unwrap()),
            "{error}"
        );
        assert!(selected.is_dir());
        assert!(displaced.is_dir());
        assert_eq!(std::fs::read_dir(&selected).unwrap().count(), 0);
        assert_eq!(std::fs::read_dir(&displaced).unwrap().count(), 0);
    }

    #[test]
    fn undo_copy_preserves_a_replacement_with_identical_contents() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source");
        let destination = root.path().join("copied");
        let displaced = root.path().join("original-copy");
        std::fs::write(&source, b"same contents").unwrap();
        std::fs::copy(&source, &destination).unwrap();
        let identity = crate::platform::platform()
            .file_identity(&std::fs::File::open(&destination).unwrap())
            .unwrap();
        let name = |path: &std::path::Path| path.to_string_lossy().into_owned();
        let transaction = Transaction::new(
            TransactionType::Copy,
            TransactionDetails {
                new_folder_identity: None,
                moved_file_identities: None,
                copied_directory_identities: None,
                copied_file_identities: Some(vec![(
                    destination.to_string_lossy().into_owned(),
                    crate::models::transaction::HistoryFileIdentity::from_file(
                        &std::fs::File::open(&destination).unwrap(),
                    )
                    .unwrap(),
                )]),
                paths: vec![name(&source)],
                target_dir: None,
                old_path: None,
                new_path: None,
                created_files: Some(vec![name(&destination)]),
                created_dirs: Some(vec![]),
                overwritten_files: Some(vec![]),
                moved_files: None,
                trash_metadata_backups: None,
                moved_dirs: None,
                copied_files: Some(vec![(name(&source), name(&destination))]),
                copied_file_hashes: Some(vec![(
                    name(&destination),
                    blake3::hash(b"same contents").to_hex().to_string(),
                )]),
            },
        );
        std::fs::rename(&destination, &displaced).unwrap();
        std::fs::write(&destination, b"same contents").unwrap();
        assert_ne!(
            crate::platform::platform()
                .file_identity(&std::fs::File::open(&destination).unwrap())
                .unwrap(),
            identity
        );
        let history = HistoryManager::new();
        history.push(transaction);
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        let result = runtime.block_on(undo_last_action(&history));
        assert!(
            matches!(result, Err(crate::models::CommandError::HistoryIdentity(failure)) if failure.issue == crate::models::error::HistoryIdentityIssue::Changed),
            "Undo must preserve a distinct object with matching contents"
        );
        assert_eq!(std::fs::read(&destination).unwrap(), b"same contents");
        assert_eq!(std::fs::read(&displaced).unwrap(), b"same contents");
        assert_eq!(
            (
                history.get_state().undo_stack.len(),
                history.get_state().redo_stack.len()
            ),
            (1, 0)
        );
        assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 3);
        std::fs::remove_file(&destination).unwrap();
        std::fs::rename(&displaced, &destination).unwrap();
        runtime
            .block_on(undo_last_action(&history))
            .unwrap()
            .unwrap();
        assert!(!destination.exists());
        assert_eq!(
            (
                history.get_state().undo_stack.len(),
                history.get_state().redo_stack.len()
            ),
            (0, 1)
        );
    }

    #[test]
    fn redo_copy_rejects_incomplete_or_changed_journals_and_can_retry() {
        use crate::models::error::HistoryCopyIssue;
        for missing_hash in [false, true] {
            let root = tempfile::tempdir().unwrap();
            let source = root.path().join("source.txt");
            let output_directory = root.path().join("copied-folder");
            std::fs::create_dir(&output_directory).unwrap();
            let destination = output_directory.join("copy.txt");
            std::fs::write(&source, b"original copy").unwrap();
            std::fs::write(&destination, b"original copy").unwrap();
            let name = |path: &std::path::Path| path.to_string_lossy().into_owned();
            let history = HistoryManager::new();
            history.push(Transaction::new(
                TransactionType::Copy,
                TransactionDetails {
                    new_folder_identity: None,
                    moved_file_identities: None,
                    copied_directory_identities: Some(vec![(
                        name(&output_directory),
                        crate::models::transaction::HistoryDirectoryIdentity::from_file(
                            &crate::platform::platform()
                                .hold_resolved_operation_directory(
                                    &std::fs::canonicalize(&output_directory).unwrap(),
                                )
                                .unwrap(),
                        )
                        .unwrap(),
                    )]),
                    copied_file_identities: Some(vec![(
                        destination.to_string_lossy().into_owned(),
                        crate::models::transaction::HistoryFileIdentity::from_file(
                            &std::fs::File::open(&destination).unwrap(),
                        )
                        .unwrap(),
                    )]),
                    paths: vec![name(&source)],
                    target_dir: None,
                    old_path: None,
                    new_path: None,
                    created_files: Some(vec![name(&destination)]),
                    created_dirs: Some(vec![name(&output_directory)]),
                    overwritten_files: Some(vec![]),
                    moved_files: None,
                    trash_metadata_backups: None,
                    moved_dirs: None,
                    copied_files: Some(vec![(name(&source), name(&destination))]),
                    copied_file_hashes: Some(if missing_hash {
                        vec![]
                    } else {
                        vec![(
                            name(&destination),
                            blake3::hash(b"original copy").to_hex().to_string(),
                        )]
                    }),
                },
            ));
            // Missing-proof fixture reserves the prior phase without guessing an Undo.
            let runtime = tokio::runtime::Builder::new_current_thread()
                .build()
                .unwrap();
            if missing_hash {
                history.begin_undo().unwrap().unwrap().complete();
                std::fs::remove_file(&destination).unwrap();
                std::fs::remove_dir(&output_directory).unwrap();
            } else {
                runtime.block_on(undo_last_action(&history)).unwrap();
                std::fs::write(&source, b"independent change").unwrap();
            }
            let error = runtime.block_on(redo_last_action(&history)).unwrap_err();
            assert!(
                matches!(error, crate::models::CommandError::HistoryCopy(failure) if failure.issue == if missing_hash { HistoryCopyIssue::Unverified } else { HistoryCopyIssue::SourceChanged })
            );
            assert!(!destination.exists());
            assert!(!output_directory.exists());
            assert_eq!(
                (
                    history.get_state().undo_stack.len(),
                    history.get_state().redo_stack.len()
                ),
                (0, 1)
            );
            if !missing_hash {
                assert_eq!(std::fs::read(&source).unwrap(), b"independent change");
                std::fs::write(&source, b"original copy").unwrap();
                runtime.block_on(redo_last_action(&history)).unwrap();
                assert_eq!(std::fs::read(&destination).unwrap(), b"original copy");
                assert_eq!(
                    (
                        history.get_state().undo_stack.len(),
                        history.get_state().redo_stack.len()
                    ),
                    (1, 0)
                );
                runtime.block_on(undo_last_action(&history)).unwrap();
                assert!(!destination.exists());
                runtime.block_on(redo_last_action(&history)).unwrap();
                assert_eq!(std::fs::read(&destination).unwrap(), b"original copy");
            } else {
                assert_eq!(std::fs::read(&source).unwrap(), b"original copy");
            }
        }
    }

    #[test]
    fn redo_copy_rolls_back_earlier_outputs_when_a_later_source_changed() {
        let root = tempfile::tempdir().unwrap();
        let first = root.path().join("first");
        let second = root.path().join("second");
        let first_destination = root.path().join("first-output");
        let second_destination = root.path().join("second-output");
        std::fs::write(&first, b"first copy").unwrap();
        std::fs::write(&second, b"changed second copy").unwrap();
        let result = perform_copy_with_progress(
            vec![
                (
                    first.clone(),
                    first_destination.clone(),
                    blake3::hash(b"first copy").to_hex().to_string(),
                ),
                (
                    second.clone(),
                    second_destination.clone(),
                    blake3::hash(b"old second copy").to_hex().to_string(),
                ),
            ],
            Arc::new(AtomicBool::new(false)),
        );
        assert!(matches!(
            result,
            Err(crate::models::CommandError::HistoryCopy(_))
        ));
        assert!(!first_destination.exists());
        assert!(!second_destination.exists());
        assert_eq!(std::fs::read(first).unwrap(), b"first copy");
        assert_eq!(std::fs::read(second).unwrap(), b"changed second copy");
        assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 2);
    }

    #[test]
    fn undo_copy_directory_error_preserves_outputs_and_allows_retry() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source.txt");
        let destination = root.path().join("copy.txt");
        let blocker = root.path().join("directory");
        std::fs::write(&source, b"copied content").unwrap();
        std::fs::write(&destination, b"copied content").unwrap();
        std::fs::create_dir(&blocker).unwrap();
        let displaced = root.path().join("original-directory");
        let name = |path: &std::path::Path| path.to_string_lossy().into_owned();
        let history = HistoryManager::new();
        history.push(Transaction::new(
            TransactionType::Copy,
            TransactionDetails {
                new_folder_identity: None,
                moved_file_identities: None,
                copied_directory_identities: Some(vec![(
                    name(&blocker),
                    crate::models::transaction::HistoryDirectoryIdentity::from_file(
                        &crate::platform::platform()
                            .hold_resolved_operation_directory(
                                &std::fs::canonicalize(&blocker).unwrap(),
                            )
                            .unwrap(),
                    )
                    .unwrap(),
                )]),
                copied_file_identities: Some(vec![(
                    destination.to_string_lossy().into_owned(),
                    crate::models::transaction::HistoryFileIdentity::from_file(
                        &std::fs::File::open(&destination).unwrap(),
                    )
                    .unwrap(),
                )]),
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
        std::fs::rename(&blocker, &displaced).unwrap();
        std::fs::write(&blocker, b"foreign content").unwrap();
        assert!(runtime.block_on(undo_last_action(&history)).is_err());
        assert_eq!(std::fs::read(&destination).unwrap(), b"copied content");
        assert_eq!(std::fs::read(&blocker).unwrap(), b"foreign content");
        assert_eq!(history.get_state().undo_stack.len(), 1);
        assert!(history.get_state().redo_stack.is_empty());
        std::fs::remove_file(&blocker).unwrap();
        std::fs::rename(&displaced, &blocker).unwrap();
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
                (
                    first_source,
                    first_destination.clone(),
                    blake3::hash(b"new file").to_hex().to_string(),
                ),
                (
                    second_source,
                    second_destination.clone(),
                    blake3::hash(b"source version").to_hex().to_string(),
                ),
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
                new_folder_identity: None,
                moved_file_identities: None,
                copied_directory_identities: None,
                copied_file_identities: Some(vec![(
                    destination.to_string_lossy().into_owned(),
                    crate::models::transaction::HistoryFileIdentity::from_file(
                        &std::fs::File::open(&destination).unwrap(),
                    )
                    .unwrap(),
                )]),
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
    use crate::models::HistoryManager;
    use crate::models::transaction::{Transaction, TransactionDetails, TransactionType};

    #[test]
    fn rename_history_keeps_missing_sources_retryable() {
        for redo in [false, true] {
            let temp = tempfile::tempdir().unwrap();
            let old = temp.path().join("old");
            let new = temp.path().join("new");
            let history = HistoryManager::new();
            history.push(rename_transaction(&old, &new));
            if redo {
                history.begin_undo().unwrap().unwrap().complete();
            }
            let runtime = tokio::runtime::Builder::new_current_thread()
                .build()
                .unwrap();
            let result = if redo {
                runtime.block_on(redo_last_action(&history))
            } else {
                runtime.block_on(undo_last_action(&history))
            };
            assert!(result.is_err());
            let state = history.get_state();
            assert_eq!(
                (state.undo_stack.len(), state.redo_stack.len()),
                if redo { (0, 1) } else { (1, 0) }
            );
            assert!(!old.exists());
            assert!(!new.exists());
            // Restore only the benign test source, then retry the same reservation.
            let (source, destination) = if redo { (&old, &new) } else { (&new, &old) };
            std::fs::create_dir(source).unwrap();
            std::fs::write(source.join("child"), b"directory child").unwrap();
            if redo {
                runtime.block_on(redo_last_action(&history)).unwrap();
            } else {
                runtime.block_on(undo_last_action(&history)).unwrap();
            }
            assert!(!source.exists());
            assert_eq!(
                std::fs::read(destination.join("child")).unwrap(),
                b"directory child"
            );
            let state = history.get_state();
            assert_eq!(
                (state.undo_stack.len(), state.redo_stack.len()),
                if redo { (1, 0) } else { (0, 1) }
            );
        }
    }

    #[test]
    fn rename_history_preserves_destinations_created_after_the_original_action() {
        let mut rejected = Vec::new();
        for redo in [false, true] {
            let temp = tempfile::tempdir().unwrap();
            let old = temp.path().join("old.txt");
            let new = temp.path().join("new.txt");
            let history = HistoryManager::new();
            let (source, destination) = if redo { (&old, &new) } else { (&new, &old) };
            std::fs::write(source, b"selected file").unwrap();
            std::fs::write(destination, b"independent later file").unwrap();
            history.push(rename_transaction(&old, &new));
            if redo {
                history.begin_undo().unwrap().unwrap().complete();
            }
            let runtime = tokio::runtime::Builder::new_current_thread()
                .build()
                .unwrap();
            let result = if redo {
                runtime.block_on(redo_last_action(&history))
            } else {
                runtime.block_on(undo_last_action(&history))
            };
            rejected.push((
                redo,
                result.is_err(),
                std::fs::read(destination).unwrap() == b"independent later file",
            ));
            if result.is_err() {
                assert_eq!(std::fs::read(source).unwrap(), b"selected file");
                let state = history.get_state();
                assert_eq!(
                    (state.undo_stack.len(), state.redo_stack.len()),
                    if redo { (0, 1) } else { (1, 0) }
                );
            }
        }
        assert!(
            rejected
                .iter()
                .all(|(_, failed, preserved)| *failed && *preserved),
            "rename history rejection: {rejected:?}"
        );
    }

    fn rename_transaction(old: &std::path::Path, new: &std::path::Path) -> Transaction {
        Transaction::new(
            TransactionType::Rename,
            TransactionDetails {
                new_folder_identity: None,
                moved_file_identities: None,
                copied_directory_identities: None,
                copied_file_identities: None,
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
        assert!(
            runtime
                .block_on(undo_last_action(&history))
                .unwrap()
                .is_some()
        );
        assert_eq!(std::fs::read(&old).unwrap(), b"fixture content");
        assert!(history.get_state().undo_stack.is_empty());
        assert_eq!(history.get_state().redo_stack[0].id, tx.id);
    }
}

#[cfg(test)]
mod move_undo_rollback_tests {
    use super::*;
    use crate::models::TransactionDetails;

    #[test]
    fn move_history_preserves_identical_replacements_before_undo_and_redo() {
        for redo in [false, true] {
            let root = tempfile::tempdir().unwrap();
            let source = root.path().join("source.txt");
            let destination = root.path().join("destination.txt");
            std::fs::write(&destination, b"moving file").unwrap();
            let history = HistoryManager::new();
            history.push(Transaction::new(
                TransactionType::Move,
                details(&source, &destination),
            ));
            let runtime = tokio::runtime::Builder::new_current_thread()
                .build()
                .unwrap();
            if redo {
                runtime.block_on(undo_last_action(&history)).unwrap();
            }
            let selected = if redo { &source } else { &destination };
            let saved = root.path().join("selected-original.txt");
            let timestamp = std::fs::metadata(selected).unwrap().modified().unwrap();
            std::fs::rename(selected, &saved).unwrap();
            std::fs::write(selected, b"moving file").unwrap();
            std::fs::File::options()
                .write(true)
                .open(selected)
                .unwrap()
                .set_modified(timestamp)
                .unwrap();
            let result = if redo {
                runtime.block_on(redo_last_action(&history))
            } else {
                runtime.block_on(undo_last_action(&history))
            };
            assert!(
                matches!(result, Err(CommandError::HistoryMove(failure)) if failure.issue == crate::models::error::HistoryMoveIssue::Changed)
            );
            assert_eq!(std::fs::read(selected).unwrap(), b"moving file");
            assert_eq!(std::fs::read(&saved).unwrap(), b"moving file");
            assert!(!(if redo { &destination } else { &source }).exists());
            assert_eq!(history.get_state().undo_stack.len(), usize::from(!redo));
            assert_eq!(history.get_state().redo_stack.len(), usize::from(redo));
            std::fs::remove_file(selected).unwrap();
            std::fs::rename(saved, selected).unwrap();
            if redo {
                runtime.block_on(redo_last_action(&history)).unwrap();
            } else {
                runtime.block_on(undo_last_action(&history)).unwrap();
            }
        }
    }

    #[test]
    fn move_history_without_identity_preserves_data_and_stack() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source.txt");
        let destination = root.path().join("destination.txt");
        std::fs::write(&destination, b"moving file").unwrap();
        let mut journal = details(&source, &destination);
        journal.moved_file_identities = None;
        let history = HistoryManager::new();
        history.push(Transaction::new(TransactionType::Move, journal));
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        let error = runtime.block_on(undo_last_action(&history)).unwrap_err();
        assert!(
            matches!(error, CommandError::HistoryMove(failure) if failure.issue == crate::models::error::HistoryMoveIssue::Unverified)
        );
        assert_eq!(std::fs::read(&destination).unwrap(), b"moving file");
        assert!(!source.exists());
        assert_eq!(history.get_state().undo_stack.len(), 1);
        assert!(history.get_state().redo_stack.is_empty());
    }

    fn details(source: &std::path::Path, destination: &std::path::Path) -> TransactionDetails {
        TransactionDetails {
            new_folder_identity: None,
            moved_file_identities: std::fs::File::open(destination).ok().map(|file| {
                vec![(
                    destination.to_string_lossy().into_owned(),
                    crate::models::transaction::HistoryMoveIdentity::from_file(&file).unwrap(),
                )]
            }),
            copied_directory_identities: None,
            copied_file_identities: None,
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

    #[cfg(target_os = "windows")]
    #[test]
    fn redo_move_roundtrips_nested_directory_history_twice() {
        let root = tempfile::tempdir().unwrap();
        let source_root = root.path().join("source");
        let source = source_root.join("child/file.txt");
        let destination_root = root.path().join("destination");
        let destination = destination_root.join("child/file.txt");
        std::fs::create_dir_all(destination.parent().unwrap()).unwrap();
        std::fs::write(&destination, b"moving file").unwrap();
        let mut journal = details(&source, &destination);
        journal.created_dirs = Some(vec![
            destination_root.to_string_lossy().into_owned(),
            destination.parent().unwrap().to_string_lossy().into_owned(),
        ]);
        journal.moved_dirs = Some(vec![
            (
                source_root.to_string_lossy().into_owned(),
                destination_root.to_string_lossy().into_owned(),
            ),
            (
                source.parent().unwrap().to_string_lossy().into_owned(),
                destination.parent().unwrap().to_string_lossy().into_owned(),
            ),
        ]);
        let transaction = Transaction::new(TransactionType::Move, journal);
        let history = HistoryManager::new();
        history.push(transaction.clone());
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        for _ in 0..2 {
            runtime.block_on(undo_last_action(&history)).unwrap();
            assert_eq!(std::fs::read(&source).unwrap(), b"moving file");
            assert!(!destination_root.exists());
            assert_eq!(history.get_state().redo_stack[0].id, transaction.id);
            runtime.block_on(redo_last_action(&history)).unwrap();
            assert_eq!(std::fs::read(&destination).unwrap(), b"moving file");
            assert!(!source_root.exists());
            assert_eq!(history.get_state().undo_stack[0].id, transaction.id);
        }
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn redo_move_destination_conflict_keeps_history_and_can_retry() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source.txt");
        let destination = root.path().join("destination.txt");
        std::fs::write(&destination, b"moving file").unwrap();
        let transaction = Transaction::new(TransactionType::Move, details(&source, &destination));
        let history = HistoryManager::new();
        history.push(transaction.clone());
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        runtime.block_on(undo_last_action(&history)).unwrap();
        std::fs::write(&destination, b"independent").unwrap();
        let error = runtime.block_on(redo_last_action(&history)).unwrap_err();
        assert!(
            matches!(error, CommandError::HistoryMove(failure) if failure.issue == crate::models::error::HistoryMoveIssue::Conflict)
        );
        assert_eq!(std::fs::read(&source).unwrap(), b"moving file");
        assert_eq!(std::fs::read(&destination).unwrap(), b"independent");
        assert_eq!(history.get_state().redo_stack[0].id, transaction.id);
        assert!(history.get_state().undo_stack.is_empty());
        std::fs::remove_file(&destination).unwrap();
        runtime.block_on(redo_last_action(&history)).unwrap();
        assert_eq!(std::fs::read(destination).unwrap(), b"moving file");
        assert!(!source.exists());
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

#[cfg(all(test, target_os = "windows"))]
mod new_folder_history_tests {
    use super::*;
    use crate::models::{TransactionDetails, TransactionType};

    fn details(path: &std::path::Path) -> TransactionDetails {
        TransactionDetails {
            new_folder_identity: None,
            moved_file_identities: None,
            paths: vec![path.to_string_lossy().into_owned()],
            target_dir: None,
            old_path: None,
            new_path: None,
            created_files: None,
            created_dirs: None,
            overwritten_files: None,
            moved_files: None,
            trash_metadata_backups: None,
            moved_dirs: None,
            copied_files: None,
            copied_file_hashes: None,
            copied_file_identities: None,
            copied_directory_identities: None,
        }
    }

    #[test]
    fn new_folder_undo_refuses_a_nonempty_replacement_without_recycling_it() {
        let root = tempfile::tempdir().unwrap();
        let selected = root.path().join("new-folder");
        let displaced = root.path().join("original-folder");
        let simulated_recycle = root.path().join("simulated-recycle");
        std::fs::create_dir(&selected).unwrap();
        let mut journal = details(&selected);
        journal.new_folder_identity = crate::models::transaction::HistoryFileIdentity::from_file(
            &crate::platform::platform()
                .hold_resolved_operation_directory(&std::fs::canonicalize(&selected).unwrap())
                .unwrap(),
        )
        .ok();
        std::fs::rename(&selected, &displaced).unwrap();
        std::fs::create_dir(&selected).unwrap();
        std::fs::write(selected.join("independent.txt"), b"independent contents").unwrap();
        let history = HistoryManager::new();
        history.push(Transaction::new(TransactionType::NewFolder, journal));
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        let result = runtime.block_on(undo_last_action(&history));
        assert!(
            matches!(result, Err(CommandError::NewFolderHistory(failure)) if failure.issue == crate::models::error::NewFolderHistoryIssue::Changed)
        );
        assert_eq!(history.get_state().undo_stack.len(), 1);
        assert!(history.get_state().redo_stack.is_empty());
        assert_eq!(
            std::fs::read(selected.join("independent.txt")).unwrap(),
            b"independent contents"
        );
        assert!(displaced.is_dir());
        assert!(!simulated_recycle.exists());
    }
    fn created_transaction(path: &std::path::Path) -> Transaction {
        let mut journal = details(path);
        journal.new_folder_identity = crate::commands::io::create_local_folder(path).unwrap();
        Transaction::new(TransactionType::NewFolder, journal)
    }

    fn runtime() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap()
    }

    #[test]
    fn new_folder_undo_preserves_later_children_and_can_retry() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("created");
        let history = HistoryManager::new();
        let transaction = created_transaction(&path);
        let identity = transaction.details.new_folder_identity.unwrap();
        history.push(transaction.clone());
        std::fs::write(path.join("later-file.txt"), b"later contents").unwrap();
        std::fs::create_dir(path.join("later-directory")).unwrap();
        let runtime = runtime();
        let error = runtime.block_on(undo_last_action(&history)).unwrap_err();
        assert!(
            matches!(error, CommandError::NewFolderHistory(failure) if failure.issue == crate::models::error::NewFolderHistoryIssue::NotEmpty)
        );
        assert_eq!(
            std::fs::read(path.join("later-file.txt")).unwrap(),
            b"later contents"
        );
        assert!(path.join("later-directory").is_dir());
        assert_eq!(history.get_state().undo_stack[0].id, transaction.id);
        assert!(history.get_state().redo_stack.is_empty());
        assert!(
            identity
                .matches(
                    &crate::platform::platform()
                        .hold_resolved_operation_directory(&std::fs::canonicalize(&path).unwrap())
                        .unwrap()
                )
                .unwrap()
        );
        std::fs::remove_file(path.join("later-file.txt")).unwrap();
        std::fs::remove_dir(path.join("later-directory")).unwrap();
        runtime.block_on(undo_last_action(&history)).unwrap();
        assert!(!path.exists());
        assert_eq!(history.get_state().redo_stack[0].id, transaction.id);
    }

    #[test]
    fn new_folder_history_roundtrips_and_tracks_the_immediate_rename() {
        let root = tempfile::tempdir().unwrap();
        let initial = root.path().join("Nouveau dossier");
        let renamed = root.path().join("Mes données_日本語");
        let history = HistoryManager::new();
        let transaction = created_transaction(&initial);
        history.push(transaction.clone());
        let runtime = runtime();
        runtime
            .block_on(crate::commands::io::rename_item_controlled(
                initial.to_string_lossy().into_owned(),
                renamed.to_string_lossy().into_owned(),
                "normal".into(),
                Arc::new(AtomicBool::new(false)),
            ))
            .unwrap();
        assert!(crate::commands::io::record_created_folder_rename(
            &history,
            &initial.to_string_lossy(),
            &renamed.to_string_lossy()
        ));
        assert_eq!(
            history.get_state().undo_stack[0].details.paths,
            vec![renamed.to_string_lossy().into_owned()]
        );
        for _ in 0..2 {
            let state = history.get_state();
            assert_eq!(state.undo_stack[0].id, transaction.id);
            assert!(
                state.undo_stack[0]
                    .details
                    .new_folder_identity
                    .unwrap()
                    .matches(
                        &crate::platform::platform()
                            .hold_resolved_operation_directory(
                                &std::fs::canonicalize(&renamed).unwrap()
                            )
                            .unwrap()
                    )
                    .unwrap()
            );
            // No journal handle locks the folder between commands.
            std::fs::rename(&renamed, &initial).unwrap();
            std::fs::rename(&initial, &renamed).unwrap();
            runtime.block_on(undo_last_action(&history)).unwrap();
            assert!(!renamed.exists());
            assert!(!initial.exists());
            runtime.block_on(redo_last_action(&history)).unwrap();
            assert!(renamed.is_dir());
            assert!(!initial.exists());
        }
        runtime.block_on(undo_last_action(&history)).unwrap();
        assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 0);
    }

    #[test]
    fn new_folder_rename_tracking_never_adopts_a_replacement_or_changes_a_reservation() {
        let root = tempfile::tempdir().unwrap();
        let initial = root.path().join("created");
        let renamed = root.path().join("renamed");
        let displaced = root.path().join("original");
        let history = HistoryManager::new();
        history.push(created_transaction(&initial));
        std::fs::rename(&initial, &renamed).unwrap();
        let selected = history.begin_undo().unwrap().unwrap();
        assert!(!crate::commands::io::record_created_folder_rename(
            &history,
            &initial.to_string_lossy(),
            &renamed.to_string_lossy()
        ));
        assert_eq!(
            selected.transaction().details.paths,
            vec![initial.to_string_lossy().into_owned()]
        );
        drop(selected);
        std::fs::rename(&renamed, &displaced).unwrap();
        std::fs::create_dir(&renamed).unwrap();
        assert!(!crate::commands::io::record_created_folder_rename(
            &history,
            &initial.to_string_lossy(),
            &renamed.to_string_lossy()
        ));
        assert_eq!(
            history.get_state().undo_stack[0].details.paths,
            vec![initial.to_string_lossy().into_owned()]
        );
        assert!(renamed.is_dir());
        assert!(displaced.is_dir());
    }

    #[test]
    fn new_folder_redo_preserves_occupied_destinations_and_keeps_the_action_retryable() {
        for directory in [false, true] {
            let root = tempfile::tempdir().unwrap();
            let path = root.path().join("created");
            let history = HistoryManager::new();
            let transaction = created_transaction(&path);
            history.push(transaction.clone());
            let runtime = runtime();
            runtime.block_on(undo_last_action(&history)).unwrap();
            if directory {
                std::fs::create_dir(&path).unwrap();
            }
            let witness = if directory {
                path.join("independent.txt")
            } else {
                path.clone()
            };
            std::fs::write(&witness, b"independent contents").unwrap();
            let error = runtime.block_on(redo_last_action(&history)).unwrap_err();
            assert!(
                matches!(error, CommandError::NewFolderHistory(failure) if failure.issue == crate::models::error::NewFolderHistoryIssue::Conflict)
            );
            assert_eq!(std::fs::read(&witness).unwrap(), b"independent contents");
            assert_eq!(history.get_state().redo_stack[0].id, transaction.id);
            assert!(history.get_state().undo_stack.is_empty());
            std::fs::remove_file(&witness).unwrap();
            if directory {
                std::fs::remove_dir(&path).unwrap();
            }
            runtime.block_on(redo_last_action(&history)).unwrap();
            runtime.block_on(undo_last_action(&history)).unwrap();
            assert!(!path.exists());
        }
    }

    #[test]
    fn new_folder_history_requires_a_proof_and_a_single_created_path() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("created");
        let other = root.path().join("other");
        let mut transaction = created_transaction(&path);
        std::fs::create_dir(&other).unwrap();
        let proof = transaction.details.new_folder_identity.take();
        let history = HistoryManager::new();
        history.push(transaction.clone());
        let runtime = runtime();
        let error = runtime.block_on(undo_last_action(&history)).unwrap_err();
        assert!(
            matches!(error, CommandError::NewFolderHistory(failure) if failure.issue == crate::models::error::NewFolderHistoryIssue::Unverified)
        );
        assert!(path.is_dir());
        assert!(other.is_dir());
        history.clear();
        transaction.details.new_folder_identity = proof;
        transaction
            .details
            .paths
            .push(other.to_string_lossy().into_owned());
        history.push(transaction);
        let error = runtime.block_on(undo_last_action(&history)).unwrap_err();
        assert!(
            matches!(error, CommandError::NewFolderHistory(failure) if failure.issue == crate::models::error::NewFolderHistoryIssue::Unverified)
        );
        assert!(path.is_dir());
        assert!(other.is_dir());
        assert_eq!(history.get_state().undo_stack.len(), 1);
    }

    #[test]
    fn new_folder_history_preserves_missing_sources_and_does_not_recreate_parents() {
        let root = tempfile::tempdir().unwrap();
        let parent = root.path().join("parent");
        std::fs::create_dir(&parent).unwrap();
        let path = parent.join("created");
        let displaced = root.path().join("original");
        let history = HistoryManager::new();
        history.push(created_transaction(&path));
        std::fs::rename(&path, &displaced).unwrap();
        let runtime = runtime();
        let error = runtime.block_on(undo_last_action(&history)).unwrap_err();
        assert!(
            matches!(error, CommandError::NewFolderHistory(failure) if failure.issue == crate::models::error::NewFolderHistoryIssue::Missing)
        );
        assert!(displaced.is_dir());
        assert!(!path.exists());
        assert_eq!(history.get_state().undo_stack.len(), 1);
        std::fs::rename(&displaced, &path).unwrap();
        runtime.block_on(undo_last_action(&history)).unwrap();
        std::fs::remove_dir(&parent).unwrap();
        let error = runtime.block_on(redo_last_action(&history)).unwrap_err();
        assert!(
            matches!(error, CommandError::NewFolderHistory(failure) if failure.issue == crate::models::error::NewFolderHistoryIssue::ParentMissing)
        );
        assert!(!parent.exists());
        assert_eq!(history.get_state().redo_stack.len(), 1);
        std::fs::create_dir(&parent).unwrap();
        runtime.block_on(redo_last_action(&history)).unwrap();
        runtime.block_on(undo_last_action(&history)).unwrap();
        assert!(!path.exists());
    }
}
