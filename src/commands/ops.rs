use crate::models::{
    get_file_entry_from_path, CommandError, ConflictEntry, HistoryManager,
    Transaction, TransactionType,
};
use crate::utils::path_security::validate_path;
use log::info;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
#[cfg(target_os = "windows")]
struct RestorationPaths {
    corrected_path: PathBuf,
}

#[cfg(target_os = "windows")]
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

#[cfg(target_os = "windows")]
/// Shares the original identity used by trash listings, restore, and properties.
pub(crate) fn original_trash_path(item: &trash::TrashItem) -> PathBuf {
    get_restoration_paths(item).corrected_path
}

#[cfg(target_os = "windows")]
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

#[cfg(target_os = "windows")]
mod trash_listing;
#[cfg(target_os = "windows")]
pub use trash_listing::list_trash;
#[cfg(target_os = "windows")]
pub(crate) use trash_listing::list_trash_unsorted;
#[cfg(not(target_os = "windows"))]
mod linux_trash;
#[cfg(not(target_os = "windows"))]
pub use linux_trash::{
    empty_trash, get_total_recycle_bin_usage, list_trash, purge_recycle_bin, restore_items,
    restore_items_report, TrashRestoreReport,
};
#[cfg(not(target_os = "windows"))]
pub(crate) use linux_trash::{list_trash_unsorted, trash_properties_metadata};

#[cfg(target_os = "windows")]
/// Empty the Recycle Bin (permanently delete all items)
pub async fn empty_trash() -> Result<(), CommandError> {
    let started = std::time::Instant::now();
    info!(target: "oxyde_lib::trash_purge", "stage=start mode=empty_native");

    crate::platform::platform()
        .empty_trash()
        .map_err(|error| {
            log::warn!(target: "oxyde_lib::trash_purge", "stage=empty_native_failed wall_s={:.6} error={error}", started.elapsed().as_secs_f64());
            CommandError::TrashError(error)
        })?;

    info!(target: "oxyde_lib::trash_purge", "stage=end mode=empty_native wall_s={:.6}", started.elapsed().as_secs_f64());

    Ok(())
}

#[cfg(target_os = "windows")]
/// Permanently delete specific items from the Recycle Bin
pub async fn purge_recycle_bin(paths: Vec<String>) -> Result<(), CommandError> {
    let started = std::time::Instant::now();
    info!(target: "oxyde_lib::trash_purge", "stage=start mode=selected requested={}", paths.len());
    let trash_items = crate::platform::platform()
        .list_trash_items()
        .map_err(|error| {
            log::warn!(target: "oxyde_lib::trash_purge", "stage=enumeration_failed wall_s={:.6} error={error}", started.elapsed().as_secs_f64());
            CommandError::TrashError(error)
        })?;

    info!(target: "oxyde_lib::trash_purge", "stage=enumerated available={} wall_s={:.6}", trash_items.len(), started.elapsed().as_secs_f64());

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
        } else {
            log::warn!(target: "oxyde_lib::trash_purge", "stage=selection_unavailable id={path_str:?}");
        }
    }

    if !items_to_purge.is_empty() {
        info!(target: "oxyde_lib::trash_purge", "stage=shell_delete_start selected={} preparation_s={:.6}", items_to_purge.len(), started.elapsed().as_secs_f64());
        crate::platform::platform()
            .purge_trash_items(items_to_purge)
            .map_err(|error| {
                log::warn!(target: "oxyde_lib::trash_purge", "stage=shell_delete_failed wall_s={:.6} error={error}", started.elapsed().as_secs_f64());
                CommandError::TrashError(error)
            })?;
    }

    info!(target: "oxyde_lib::trash_purge", "stage=end mode=selected wall_s={:.6}", started.elapsed().as_secs_f64());
    Ok(())
}

mod conflicts;
use conflicts::check_conflicts_real;

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

#[derive(Debug, Clone)]
pub struct TrashSystemConfig {
    pub drive_path: String,
    pub max_size_mb: u64,
    pub nuke_on_delete: bool,
}

#[cfg(target_os = "windows")]
pub async fn get_total_recycle_bin_usage() -> Result<(u64, u64), CommandError> {
    crate::platform::platform().recycle_bin_usage().map_err(CommandError::SystemError)
}

#[cfg(target_os = "windows")]
fn normalize_restore_path(path: &std::path::Path) -> String {
    path.to_string_lossy()
        .to_lowercase()
        .replace('/', "\\")
        .trim_start_matches("\\\\?\\")
        .to_string()
}
#[cfg(all(test, target_os = "windows"))]
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
#[cfg(target_os = "windows")]
mod trash_restore;
#[cfg(target_os = "windows")]
pub use trash_restore::{restore_items_report, TrashRestoreReport};

#[cfg(target_os = "windows")]
pub async fn restore_items(paths: Vec<String>) -> Result<Vec<String>, CommandError> {
    let report = restore_items_report(paths).await?;
    if let Some((_, error)) = report.failures.into_iter().next() {
        return Err(CommandError::TrashError(error));
    }
    Ok(report.restored)
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
                    let links = tx.details.created_links.clone().unwrap_or_default();
                    if let Err(error) = crate::systems::history_links::redo(&links) {
                        return Err(rollback_copy_directories(
                            &mut directories,
                            &made_dirs,
                            CommandError::from(error),
                        ));
                    }
                    if !files.is_empty() {
                        match perform_copy_with_progress(files, Arc::new(AtomicBool::new(false))) {
                            Ok(identities) => tx.details.copied_file_identities = Some(identities),
                            Err(error) => {
                                let error = match crate::systems::history_links::remove(&links) {
                                    Ok(()) => error,
                                    Err(rollback) => CommandError::IoError(format!(
                                        "{error}; rollback incomplete: {rollback}"
                                    )),
                                };
                                return Err(rollback_copy_directories(
                                    &mut directories,
                                    &made_dirs,
                                    error,
                                ));
                            }
                        }
                    }
                    crate::systems::history_directory_attributes::DirectoryAttributes::created(&tx.details)
                        .apply(&mut directories, &made_dirs);
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

#[cfg(all(test, target_os = "windows"))]
#[path = "ops/tests/restoration_selection_tests.rs"]
mod restoration_selection_tests;
