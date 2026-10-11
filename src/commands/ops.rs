use crate::models::{
    get_file_entry_from_path, CommandError, ConflictEntry, HistoryManager,
    Transaction, TransactionType, TrashEntry,
};
use crate::utils::path_security::validate_path;
use log::info;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
struct RestorationPaths {
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

    RestorationPaths { corrected_path }
}

/// Shares the original identity used by trash listings, restore, and properties.
pub(crate) fn original_trash_path(item: &trash::TrashItem) -> PathBuf {
    get_restoration_paths(item).corrected_path
}

pub(crate) fn item_with_correct_restore_name(item: &trash::TrashItem) -> trash::TrashItem {
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

/// UI preparation only: the executor repeats this classification on held sources.
pub fn is_self_transfer(source: &std::path::Path, target_dir: &std::path::Path) -> bool {
    source.file_name().is_some_and(|name| {
        crate::systems::transfer_naming::same_entry(source, &target_dir.join(name))
    })
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
            if crate::systems::transfer_naming::same_entry(&source_path, &target_path) {
                continue;
            }
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

fn normalize_restore_path(path: &std::path::Path) -> String {
    path.to_string_lossy()
        .to_lowercase()
        .replace('/', "\\")
        .trim_start_matches("\\\\?\\")
        .to_string()
}
fn select_restoration_tasks(
    items: &[trash::TrashItem],
    paths: Vec<String>,
) -> Result<Vec<trash::TrashItem>, CommandError> {
    use crate::models::error::{TrashRestoreFailure, TrashRestoreIssue};
    let mut selected = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for path in paths {
        let normalized = normalize_restore_path(std::path::Path::new(&path));
        // An explicit ID always wins over another item's original path.
        let exact = items
            .iter()
            .filter(|item| normalize_restore_path(std::path::Path::new(&item.id)) == normalized)
            .collect::<Vec<_>>();
        let candidates = if !exact.is_empty() {
            exact
        } else {
            items
                .iter()
                .filter(|item| normalize_restore_path(&item.original_path()) == normalized)
                .collect::<Vec<_>>()
        };
        if candidates.len() != 1 {
            return Err(CommandError::TrashError(
                TrashRestoreFailure {
                    issue: if candidates.is_empty() {
                        TrashRestoreIssue::Unavailable
                    } else {
                        TrashRestoreIssue::Ambiguous
                    },
                    path,
                    detail: "Selection does not identify exactly one trash item".into(),
                }
                .to_string(),
            ));
        }
        let item = candidates[0];
        if seen.insert(normalize_restore_path(std::path::Path::new(&item.id))) {
            selected.push(item.clone());
        }
    }
    Ok(selected)
}
pub async fn restore_items(paths: Vec<String>) -> Result<Vec<String>, CommandError> {
    info!("Restoring items: {:?}", paths);
    let items = crate::platform::platform()
        .list_trash_items()
        .map_err(CommandError::TrashError)?
        .iter()
        .map(item_with_correct_restore_name)
        .collect::<Vec<_>>();
    let tasks = select_restoration_tasks(&items, paths)?;
    if tasks.is_empty() {
        return Err(CommandError::TrashError(
            "No matching items found in Recycle Bin".into(),
        ));
    }
    let restored = tasks
        .iter()
        .map(|item| item.original_path().to_string_lossy().into_owned())
        .collect();
    crate::platform::platform()
        .restore_trash_items(tasks)
        .map_err(CommandError::TrashError)?;
    Ok(restored)
}

/// Replay the same system deletion policy used by the operation queue.
/// Recycle Bin capacity is decided by the OS, without a fixed size or item limit.
#[cfg(not(target_os = "windows"))]
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
                #[cfg(target_os = "windows")]
                crate::systems::history_delete::undo(&tx.details)?;
                #[cfg(not(target_os = "windows"))]
                restore_items(tx.details.paths.clone()).await?;
            }
            TransactionType::Rename => {
                let (Some(old), Some(new)) =
                    (tx.details.old_path.as_ref(), tx.details.new_path.as_ref())
                else {
                    return Err(CommandError::HistoryRename(crate::models::error::HistoryRenameFailure {
                        issue: crate::models::error::HistoryIdentityIssue::Unverified,
                        path: tx.details.paths.first().cloned().unwrap_or_default(),
                    }));
                };
                let old_pb = PathBuf::from(old);
                let new_pb = PathBuf::from(new);
                crate::systems::item_rename::rename_from_history(&new_pb, &old_pb, tx.details.renamed_item_identity)?;
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
                #[cfg(target_os = "windows")]
                crate::systems::history_delete::redo(&tx.details)?;
                #[cfg(not(target_os = "windows"))]
                {
                    let paths: Vec<PathBuf> = tx.details.paths.iter().map(PathBuf::from).collect();
                    recycle_history_paths(paths)?;
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
                let (Some(old), Some(new)) =
                    (tx.details.old_path.as_ref(), tx.details.new_path.as_ref())
                else {
                    return Err(CommandError::HistoryRename(crate::models::error::HistoryRenameFailure {
                        issue: crate::models::error::HistoryIdentityIssue::Unverified,
                        path: tx.details.paths.first().cloned().unwrap_or_default(),
                    }));
                };
                let old_pb = PathBuf::from(old);
                let new_pb = PathBuf::from(new);
                crate::systems::item_rename::rename_from_history(&old_pb, &new_pb, tx.details.renamed_item_identity)?;
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
#[path = "ops/tests/trash_restore_tests.rs"]
mod trash_restore_tests;

#[cfg(test)]
#[path = "ops/tests/copy_redo_tests.rs"]
mod copy_redo_tests;

#[cfg(test)]
#[path = "ops/tests/history_concurrency_tests.rs"]
mod history_concurrency_tests;

#[cfg(test)]
#[path = "ops/tests/move_undo_rollback_tests.rs"]
mod move_undo_rollback_tests;

#[cfg(all(test, target_os = "windows"))]
#[path = "ops/tests/new_folder_history_tests.rs"]
mod new_folder_history_tests;

#[cfg(test)]
#[path = "ops/tests/restoration_selection_tests.rs"]
mod restoration_selection_tests;
