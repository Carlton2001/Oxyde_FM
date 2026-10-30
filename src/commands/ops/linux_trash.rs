//! Freedesktop trash commands. Entries are identified by their content path in
//! `$trash/files`, compared exactly; mutations go through the item's `.trashinfo` record.
use crate::models::error::{TrashRestoreFailure, TrashRestoreIssue};
use crate::models::{CommandError, TrashEntry};
use crate::platform::platform;
use crate::utils::path_text::encode;
use log::info;
use std::collections::{HashMap, HashSet};

pub fn list_trash() -> Result<Vec<TrashEntry>, CommandError> {
    let mut entries = list_trash_unsorted()?;
    entries.sort_by_cached_key(|entry| (!entry.is_dir, entry.name.to_lowercase()));
    Ok(entries)
}

/// The tab applies its configured sort; avoid sorting the same listing twice.
pub(crate) fn list_trash_unsorted() -> Result<Vec<TrashEntry>, CommandError> {
    let started = std::time::Instant::now();
    let items = platform().list_trash_items().map_err(CommandError::TrashError)?;
    let entries: Vec<_> = items.iter().filter_map(entry).collect();
    info!(target: "oxyde_lib::trash_listing", "mode=freedesktop items={} records={} total_s={:.6}",
        entries.len(), items.len(), started.elapsed().as_secs_f64());
    Ok(entries)
}

/// A record whose content is gone is a stale leftover, not a restorable item.
fn entry(item: &trash::TrashItem) -> Option<TrashEntry> {
    let content = platform().trash_content_path(item)?;
    let metadata = std::fs::symlink_metadata(&content).ok()?;
    let deleted_time = (item.time_deleted.max(0) as u64) * 1000;
    let is_dir = metadata.is_dir();
    Some(TrashEntry {
        name: item.name.to_string_lossy().into_owned(),
        path: encode(&content),
        original_path: encode(&item.original_path()),
        is_dir,
        size: if is_dir { 0 } else { metadata.len() },
        deleted_time,
        modified: metadata.modified().ok()
            .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
            .map_or(deleted_time, |time| time.as_millis() as u64),
    })
}

/// Original location and deletion time of a listed content path, for Properties.
pub(crate) fn trash_properties_metadata(path: &std::path::Path) -> Option<(String, Option<i64>)> {
    // Listed content always lies in a `files` directory; skip the listing elsewhere.
    if !path.components().any(|component| component.as_os_str() == "files") {
        return None;
    }
    let items = platform().list_trash_items().ok()?;
    let item = items.iter().find(|item| platform().trash_content_path(item).as_deref() == Some(path))?;
    Some((encode(&item.original_path()),
        item.time_deleted.checked_mul(1000).filter(|time| *time >= 0)))
}

/// A request names an item by its content path, or by its original path when exactly
/// one item came from there (Undo of a deletion); lossy display names may collide.
fn select(items: Vec<trash::TrashItem>, paths: Vec<String>) -> (Vec<trash::TrashItem>, Vec<(String, TrashRestoreIssue)>) {
    let keyed = items.into_iter().filter_map(|item| {
        let content = encode(&platform().trash_content_path(&item)?);
        Some((content, item))
    }).collect();
    select_keyed(keyed, paths)
}

fn select_keyed(items: Vec<(String, trash::TrashItem)>, paths: Vec<String>) -> (Vec<trash::TrashItem>, Vec<(String, TrashRestoreIssue)>) {
    let mut by_content: HashMap<&str, Vec<usize>> = HashMap::new();
    let mut by_original: HashMap<String, Vec<usize>> = HashMap::new();
    for (index, (content, item)) in items.iter().enumerate() {
        by_content.entry(content.as_str()).or_default().push(index);
        by_original.entry(encode(&item.original_path())).or_default().push(index);
    }
    let mut chosen = Vec::new();
    let mut failures = Vec::new();
    let mut seen = HashSet::new();
    for path in paths {
        if !seen.insert(path.clone()) {
            continue;
        }
        let matches = by_content.get(path.as_str()).or_else(|| by_original.get(&path));
        match matches.map(Vec::as_slice) {
            // An item requested under both of its paths is still selected once.
            Some([index]) => if !chosen.contains(index) { chosen.push(*index) },
            Some(_) => failures.push((path, TrashRestoreIssue::Ambiguous)),
            None => failures.push((path, TrashRestoreIssue::Unavailable)),
        }
    }
    let mut items: Vec<_> = items.into_iter().map(|(_, item)| Some(item)).collect();
    (chosen.into_iter().filter_map(|index| items[index].take()).collect(), failures)
}

fn selection_failure(path: String, issue: TrashRestoreIssue) -> (String, String) {
    let detail = "Selection does not identify exactly one trash item".into();
    let error = TrashRestoreFailure { issue, path: path.clone(), detail }.to_string();
    (path, error)
}

pub async fn empty_trash() -> Result<(), CommandError> {
    let started = std::time::Instant::now();
    info!(target: "oxyde_lib::trash_purge", "stage=start mode=empty");
    platform().empty_trash().map_err(CommandError::TrashError)?;
    info!(target: "oxyde_lib::trash_purge", "stage=end mode=empty wall_s={:.6}", started.elapsed().as_secs_f64());
    Ok(())
}

/// Permanently delete the selected items; unknown or ambiguous selections are skipped.
pub async fn purge_recycle_bin(paths: Vec<String>) -> Result<(), CommandError> {
    let started = std::time::Instant::now();
    let items = platform().list_trash_items().map_err(CommandError::TrashError)?;
    let (selected, failures) = select(items, paths);
    for (path, issue) in failures {
        log::warn!(target: "oxyde_lib::trash_purge", "stage=selection_unavailable id={path:?} issue={issue:?}");
    }
    if !selected.is_empty() {
        platform().purge_trash_items(selected).map_err(CommandError::TrashError)?;
    }
    info!(target: "oxyde_lib::trash_purge", "stage=end mode=selected wall_s={:.6}", started.elapsed().as_secs_f64());
    Ok(())
}

#[derive(Default)]
pub struct TrashRestoreReport {
    pub restored: Vec<String>,
    pub failures: Vec<(String, String)>,
    /// Content and original folder of the items a rename cannot bring back, for a move.
    pub on_other_volumes: Vec<(String, String)>,
}

/// Whether the item's content and its original folder lie on different file systems.
fn on_other_volume(item: &trash::TrashItem) -> bool {
    use std::os::unix::fs::MetadataExt;
    let device = |path: &std::path::Path| std::fs::symlink_metadata(path).ok().map(|metadata| metadata.dev());
    platform().trash_content_path(item).and_then(|content| device(&content))
        .zip(device(&item.original_parent))
        .is_some_and(|(content, parent)| content != parent)
}

/// Restore each item independently; failures leave the item and its record in the trash.
pub async fn restore_items_report(paths: Vec<String>) -> Result<TrashRestoreReport, CommandError> {
    let started = std::time::Instant::now();
    let items = platform().list_trash_items().map_err(CommandError::TrashError)?;
    let (selected, failures) = select(items, paths);
    let mut report = TrashRestoreReport {
        failures: failures.into_iter().map(|(path, issue)| selection_failure(path, issue)).collect(),
        ..Default::default()
    };
    for item in selected {
        let original = encode(&item.original_path());
        if on_other_volume(&item) {
            if let Some(content) = platform().trash_content_path(&item) {
                report.on_other_volumes.push((encode(&content), encode(&item.original_parent)));
                continue;
            }
        }
        match platform().restore_trash_items(vec![item]) {
            Ok(()) => report.restored.push(original),
            Err(error) => {
                log::warn!(target: "oxyde_lib::trash_restore", "stage=item_failed destination={original:?} error={error}");
                report.failures.push((original, error));
            }
        }
    }
    info!(target: "oxyde_lib::trash_restore", "stage=end restored={} failed={} wall_s={:.6}",
        report.restored.len(), report.failures.len(), started.elapsed().as_secs_f64());
    Ok(report)
}

pub async fn restore_items(paths: Vec<String>) -> Result<Vec<String>, CommandError> {
    let report = restore_items_report(paths).await?;
    if let Some((_, error)) = report.failures.into_iter().next() {
        return Err(CommandError::TrashError(error));
    }
    // Undo has no operation to show a transfer's progress: the trash view offers it.
    if let Some((_, parent)) = report.on_other_volumes.into_iter().next() {
        return Err(CommandError::TrashError(TrashRestoreFailure {
            issue: TrashRestoreIssue::Unavailable, path: parent,
            detail: "The trash and the original folder are on different file systems".into(),
        }.to_string()));
    }
    Ok(report.restored)
}

/// Total size of the trash content and its item count.
pub async fn get_total_recycle_bin_usage() -> Result<(u64, u64), CommandError> {
    platform().recycle_bin_usage().map_err(CommandError::SystemError)
}

#[cfg(test)]
#[path = "tests/linux_selection.rs"]
mod tests;
